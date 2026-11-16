//! Inspect and execute actions against a pinned repository revision.

use super::{clean, name, reference, remote, remote_branch, run, run_bytes};
use crate::{
    domain::git::Error,
    modules::git::actions::history::{Action, Context, Input, Output, Reset, Target},
};
use std::path::{Path, PathBuf};

pub(super) async fn unfinished(root: &Path) -> Result<Option<String>, Error> {
    let paths = run(
        root,
        &[
            "rev-parse",
            "--git-path",
            "rebase-merge",
            "--git-path",
            "rebase-apply",
            "--git-path",
            "MERGE_HEAD",
            "--git-path",
            "CHERRY_PICK_HEAD",
            "--git-path",
            "REVERT_HEAD",
            "--git-path",
            "sequencer",
            "--git-path",
            "BISECT_START",
        ],
    )
    .await?;
    for (path, action) in
        paths
            .lines()
            .zip(["rebase", "rebase", "merge", "cherry-pick", "revert", "", ""])
    {
        if tokio::fs::try_exists(root.join(path))
            .await
            .map_err(|_| Error::Failed)?
        {
            return Ok(Some(action.into()));
        }
    }
    Ok(None)
}

fn oid(value: &str) -> bool {
    matches!(value.len(), 40 | 64) && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

pub(crate) async fn inspect(target: &Target) -> Result<Context, Error> {
    let root = target.root();
    let revision = target.revision();
    match target {
        Target::Branch(_) => reference(root, revision).await?,
        Target::Commit(_) if !oid(revision) => return Err(Error::InvalidName),
        Target::Commit(_) => {}
    }
    let id = resolve(root, revision).await?;
    let head = resolve(root, "HEAD").await?;
    let branch = run(root, &["symbolic-ref", "--quiet", "HEAD"])
        .await
        .unwrap_or_default();
    let branch = branch.trim_end().to_owned();
    let parents = run(root, &["show", "-s", "--format=%P", &id])
        .await?
        .split_whitespace()
        .map(str::to_owned)
        .collect::<Vec<_>>();
    let unpublished = matches!(target, Target::Commit(_))
        && id == head
        && !branch.is_empty()
        && parents.len() == 1
        && unpublished(root, &id).await?;
    let refs = if matches!(target, Target::Commit(_)) {
        run(
            root,
            &[
                "for-each-ref",
                "--format=%(refname)",
                "--points-at",
                &id,
                "refs/heads",
                "refs/remotes",
            ],
        )
        .await?
        .lines()
        .map(str::to_owned)
        .collect()
    } else {
        Vec::new()
    };
    let message = if unpublished {
        Some(
            run(root, &["show", "-s", "--format=%B", &id])
                .await?
                .trim_end_matches('\n')
                .to_owned(),
        )
    } else {
        None
    };
    Ok(Context {
        id,
        head,
        branch,
        parents,
        refs,
        unfinished: unfinished(root).await?,
        unpublished,
        message,
    })
}

async fn resolve(root: &Path, revision: &str) -> Result<String, Error> {
    Ok(run(
        root,
        &[
            "rev-parse",
            "--verify",
            "--end-of-options",
            &format!("{revision}^{{commit}}"),
        ],
    )
    .await?
    .trim_end()
    .to_owned())
}

async fn unpublished(root: &Path, id: &str) -> Result<bool, Error> {
    if !run(root, &["remote"]).await?.is_empty()
        && run(root, &["rev-parse", "--verify", "@{upstream}"])
            .await
            .is_err()
    {
        return Ok(false);
    }
    Ok(run(
        root,
        &[
            "for-each-ref",
            "--format=%(refname)",
            "--contains",
            id,
            "refs/remotes",
            "refs/tags",
        ],
    )
    .await?
    .is_empty())
}

pub(crate) async fn execute(
    target: &Target,
    action: Action,
    expected: &Context,
    input: &Input,
) -> Result<Output, Error> {
    let root = target.root();
    let revision = target.revision();
    let id = &expected.id;
    if action.reads() {
        return read(root, action, expected).await;
    }
    let _guard = super::WRITE_LOCK.lock().await;
    let current = inspect(target).await.map_err(|_| Error::Stale)?;
    if &current != expected {
        return Err(Error::Stale);
    }
    if matches!(action, Action::Continue | Action::Abort) {
        let operation = current
            .unfinished
            .filter(|value| !value.is_empty())
            .ok_or(Error::Stale)?;
        let option = if action == Action::Continue {
            "--continue"
        } else {
            "--abort"
        };
        run(root, &[&operation, option]).await?;
        return Ok(Output::Done);
    }
    if current.unfinished.is_some() {
        return Err(Error::Unfinished);
    }

    if matches!(
        action,
        Action::Merge
            | Action::Rebase
            | Action::SwitchRebase
            | Action::PullMerge
            | Action::PullRebase
            | Action::Detach
            | Action::Pick
            | Action::Revert
    ) || action == Action::Create && input.checkout
    {
        clean(root).await?;
    }
    if current.branch.is_empty()
        && matches!(
            action,
            Action::Merge
                | Action::Rebase
                | Action::SwitchRebase
                | Action::PullMerge
                | Action::PullRebase
                | Action::Reset
                | Action::Undo
                | Action::Message
        )
    {
        return Err(Error::CurrentBranch);
    }

    if matches!(action, Action::Rebase | Action::SwitchRebase) {
        protect_untracked(root, id).await?;
        protect_untracked(root, &expected.head).await?;
    }
    match action {
        Action::Create => {
            name(root, &input.name).await?;
            if input.checkout {
                run(
                    root,
                    &[
                        "switch",
                        "--no-overwrite-ignore",
                        "--create",
                        &input.name,
                        id,
                    ],
                )
                .await?;
            } else {
                run(root, &["branch", "--", &input.name, id]).await?;
            }
        }
        Action::Detach => {
            run(
                root,
                &["switch", "--no-overwrite-ignore", "--detach", "--", id],
            )
            .await?;
        }
        Action::Merge => {
            run(
                root,
                &["merge", "--no-edit", "--no-overwrite-ignore", "--", id],
            )
            .await?;
        }
        Action::Rebase => {
            run(root, &["rebase", "--no-autostash", "--", id]).await?;
        }
        Action::SwitchRebase => {
            let branch = revision
                .strip_prefix("refs/heads/")
                .ok_or(Error::InvalidName)?;
            run(root, &["switch", "--no-overwrite-ignore", "--", branch]).await?;
            run(root, &["rebase", "--no-autostash", "--", &expected.head]).await?;
        }
        Action::PullMerge | Action::PullRebase => {
            let (remote, branch) = remote_branch(root, revision).await?;
            let source = format!("refs/heads/{branch}");
            run(root, &["fetch", "--no-tags", "--", &remote, &source]).await?;
            let after_fetch = inspect(target).await?;
            if after_fetch.branch != current.branch
                || after_fetch.head != current.head
                || after_fetch.unfinished.is_some()
            {
                return Err(Error::Stale);
            }
            clean(root).await?;
            let fetched = resolve(root, "FETCH_HEAD").await?;
            if action == Action::PullMerge {
                run(
                    root,
                    &[
                        "merge",
                        "--no-edit",
                        "--no-overwrite-ignore",
                        "--",
                        &fetched,
                    ],
                )
                .await?;
            } else {
                protect_untracked(root, &fetched).await?;
                run(root, &["rebase", "--no-autostash", "--", &fetched]).await?;
            }
        }
        Action::Tag => {
            if input.name.starts_with('-') || input.name.is_empty() {
                return Err(Error::InvalidName);
            }
            let mut args = vec!["-c", "tag.gpgSign=false", "tag"];
            if !input.message.trim().is_empty() {
                args.extend(["-a", "-m", &input.message]);
            }
            args.extend(["--", &input.name, id]);
            run(root, &args).await?;
        }
        Action::PushTag => {
            if !revision.starts_with("refs/tags/") {
                return Err(Error::InvalidName);
            }
            remote(root, &input.name).await?;
            run(
                root,
                &[
                    "-c",
                    &format!("remote.{}.mirror=false", input.name),
                    "push",
                    "--no-mirror",
                    "--no-follow-tags",
                    "--",
                    &input.name,
                    &format!("{revision}:{revision}"),
                ],
            )
            .await?;
        }
        Action::DeleteTag => {
            let tag = revision
                .strip_prefix("refs/tags/")
                .ok_or(Error::InvalidName)?;
            run(root, &["tag", "--delete", "--", tag]).await?;
        }
        Action::Patch => {
            if current.parents.len() > 1 {
                return Err(Error::InvalidName);
            }
            let path = input
                .path
                .as_ref()
                .filter(|path| path.is_absolute())
                .ok_or(Error::InvalidName)?;
            let bytes =
                run_bytes(root, &["format-patch", "--stdout", "--binary", "-1", id]).await?;
            use tokio::io::AsyncWriteExt;
            let mut file = tokio::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(path)
                .await
                .map_err(|_| Error::Failed)?;
            if file.write_all(&bytes).await.is_err() {
                drop(file);
                let _ = tokio::fs::remove_file(path).await;
                return Err(Error::Failed);
            }
        }
        Action::Pick | Action::Revert => {
            let command = if action == Action::Pick {
                "cherry-pick"
            } else {
                "revert"
            };
            let mut args = vec![command, "--no-edit"];
            let mainline;
            if expected.parents.len() > 1 {
                let parent = input
                    .parent
                    .filter(|index| (1..=expected.parents.len()).contains(index))
                    .ok_or(Error::InvalidName)?;
                mainline = parent.to_string();
                args.extend(["--mainline", &mainline]);
            }
            args.extend(["--", id]);
            run(root, &args).await?;
        }
        Action::Reset => {
            let mode = match input.reset {
                Reset::Soft => "--soft",
                Reset::Mixed => "--mixed",
                Reset::Hard => {
                    if current.branch.strip_prefix("refs/heads/") != Some(input.name.as_str()) {
                        return Err(Error::InvalidName);
                    }
                    protect_untracked(root, id).await?;
                    "--hard"
                }
                Reset::Keep => "--keep",
            };
            run(root, &["reset", mode, id, "--"]).await?;
        }
        Action::Undo | Action::Message if current.unpublished => {
            if action == Action::Undo {
                run(root, &["reset", "--soft", &current.parents[0], "--"]).await?;
            } else {
                if input.message.trim().is_empty() {
                    return Err(Error::InvalidName);
                }
                run(root, &["commit", "--amend", "--only", "-m", &input.message]).await?;
            }
        }
        _ => return Err(Error::InvalidName),
    }
    Ok(Output::Done)
}

async fn protect_untracked(root: &Path, id: &str) -> Result<(), Error> {
    let untracked = run_bytes(root, &["ls-files", "-z", "--others"]).await?;
    if untracked.is_empty() {
        return Ok(());
    }
    let target = run_bytes(root, &["ls-tree", "-r", "-z", "--name-only", id]).await?;
    let files: std::collections::BTreeSet<_> = target.split(|byte| *byte == 0).collect();
    for path in untracked
        .split(|byte| *byte == 0)
        .filter(|path| !path.is_empty())
    {
        if files.contains(path)
            || path
                .iter()
                .enumerate()
                .any(|(index, byte)| *byte == b'/' && files.contains(&path[..index]))
        {
            return Err(Error::Dirty);
        }
        let prefix = [path, b"/"].concat();
        if files
            .range(prefix.as_slice()..)
            .next()
            .is_some_and(|file| file.starts_with(&prefix))
        {
            return Err(Error::Dirty);
        }
    }
    Ok(())
}

async fn read(root: &Path, action: Action, context: &Context) -> Result<Output, Error> {
    let id = &context.id;
    Ok(match action {
        Action::Tip => Output::Navigate(vec![id.clone()]),
        Action::Parents => Output::Navigate(context.parents.clone()),
        Action::Children => {
            let lines = run(root, &["rev-list", "--all", "--parents", "HEAD", "--"]).await?;
            Output::Navigate(
                lines
                    .lines()
                    .filter_map(|line| {
                        let mut parts = line.split_whitespace();
                        let child = parts.next()?;
                        parts.any(|parent| parent == id).then(|| child.to_owned())
                    })
                    .collect(),
            )
        }
        Action::Snapshot => Output::Snapshot(
            run(root, &["ls-tree", "-r", "-z", "--name-only", id])
                .await?
                .split_terminator('\0')
                .map(str::to_owned)
                .collect(),
        ),
        _ => return Err(Error::InvalidName),
    })
}

pub(crate) async fn load_commit(
    root: PathBuf,
    id: String,
) -> Result<crate::domain::repository::Commit, Error> {
    if !oid(&id) {
        return Err(Error::InvalidName);
    }
    let record = run(
        &root,
        &[
            "show",
            "-s",
            "--format=%P%x00%s%x00%an%x00%as%x00%ct%x00%D",
            &id,
        ],
    )
    .await?;
    let fields: Vec<_> = record.trim_end_matches('\n').split('\0').collect();
    let [parents, subject, author, date, timestamp, refs] = fields.as_slice() else {
        return Err(Error::Failed);
    };
    Ok(crate::domain::repository::Commit {
        id,
        parents: parents.split_whitespace().map(str::to_owned).collect(),
        subject: (*subject).into(),
        author: (*author).into(),
        date: (*date).into(),
        timestamp: timestamp.parse().map_err(|_| Error::Failed)?,
        refs: (*refs).into(),
        graph: Default::default(),
    })
}

#[cfg(test)]
mod tests;
