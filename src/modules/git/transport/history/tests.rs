use super::*;
use crate::domain::repository::project::{BranchKey, CommitKey};

fn commit(root: &Path, message: &str, content: &str) -> String {
    use super::super::tests::command;
    std::fs::write(root.join("example.txt"), content).unwrap();
    command(root, &["add", "--", "example.txt"]);
    command(root, &["commit", "-m", message]);
    command(root, &["rev-parse", "HEAD"])
}

fn key(root: &Path, id: String) -> Target {
    Target::Commit(CommitKey {
        root: root.to_owned(),
        id,
    })
}

fn branch(root: &Path, name: &str) -> Target {
    Target::Branch(BranchKey {
        root: root.to_owned(),
        reference: format!("refs/heads/{name}"),
    })
}

#[tokio::test]
async fn snapshot_reads_unmodified_content() {
    use super::super::tests::repository;
    let dir = repository();
    let root = dir.path();
    let revision = commit(root, "Add file", "old\n");
    std::fs::write(root.join("example.txt"), "new\n").unwrap();
    let target = key(root, revision);
    let context = inspect(&target).await.unwrap();
    let snapshot = execute(&target, Action::Snapshot, &context, &Input::default())
        .await
        .unwrap();
    assert!(matches!(snapshot, Output::Snapshot(paths) if paths == ["example.txt"]));
    let contents = crate::infrastructure::repository::blob(
        root.to_owned(),
        context.id.clone(),
        "example.txt".into(),
    )
    .await
    .unwrap();
    assert_eq!(contents, "old\n");
    assert_eq!(
        std::fs::read_to_string(root.join("example.txt")).unwrap(),
        "new\n"
    );
}

#[tokio::test]
async fn branch_moves_make_a_confirmation_stale() {
    use super::super::tests::{command, repository};
    let dir = repository();
    let root = dir.path();
    command(root, &["branch", "topic"]);
    let target = branch(root, "topic");
    let context = inspect(&target).await.unwrap();
    let revision = commit(root, "Advance", "new\n");
    command(root, &["branch", "-f", "topic", &revision]);
    assert!(matches!(
        execute(&target, Action::Merge, &context, &Input::default()).await,
        Err(Error::Stale)
    ));
}

#[tokio::test]
async fn conflict_blocks_other_writes_and_can_be_aborted() {
    use super::super::tests::{command, repository};
    let dir = repository();
    let root = dir.path();
    commit(root, "Base", "base\n");
    command(root, &["switch", "-c", "topic"]);
    commit(root, "Topic", "topic\n");
    command(root, &["switch", "main"]);
    let before = commit(root, "Main", "main\n");
    let target = branch(root, "topic");
    let context = inspect(&target).await.unwrap();
    assert!(
        execute(&target, Action::Merge, &context, &Input::default())
            .await
            .is_err()
    );
    let interrupted = inspect(&target).await.unwrap();
    assert_eq!(interrupted.unfinished.as_deref(), Some("merge"));
    assert!(matches!(
        super::super::execute(
            root.to_owned(),
            crate::domain::git::Operation::Delete("refs/heads/topic".into())
        )
        .await,
        Err(Error::Unfinished)
    ));
    execute(&target, Action::Abort, &interrupted, &Input::default())
        .await
        .unwrap();
    assert_eq!(command(root, &["rev-parse", "HEAD"]), before);
    assert_eq!(command(root, &["status", "--porcelain"]), "");
}

#[tokio::test]
async fn reset_modes_preserve_the_promised_state() {
    use super::super::tests::{command, repository};
    for (mode, status) in [
        (Reset::Soft, "M  example.txt"),
        (Reset::Mixed, " M example.txt"),
        (Reset::Hard, ""),
        (Reset::Keep, ""),
    ] {
        let dir = repository();
        let root = dir.path();
        let initial = commit(root, "Initial file", "initial\n");
        commit(root, "Later file", "later\n");
        let target = key(root, initial.clone());
        let context = inspect(&target).await.unwrap();
        let input = Input {
            reset: mode,
            name: "main".into(),
            ..Default::default()
        };
        execute(&target, Action::Reset, &context, &input)
            .await
            .unwrap();
        assert_eq!(command(root, &["rev-parse", "HEAD"]), initial);
        // The helper trims leading whitespace. Use Git's index directly to distinguish mixed and soft.
        assert_eq!(command(root, &["status", "--porcelain"]), status.trim());
        if mode == Reset::Mixed {
            assert!(command(root, &["diff", "--cached", "--name-only"]).is_empty());
        }
    }
}

#[tokio::test]
async fn undo_requires_unpublished_tip_and_preserves_staging() {
    use super::super::tests::{command, remote, repository};
    let dir = repository();
    let root = dir.path();
    let _remote = remote(root);
    let id = commit(root, "New", "content\n");
    let target = key(root, id);
    assert!(inspect(&target).await.unwrap().unpublished);
    command(root, &["push"]);
    let published = inspect(&target).await.unwrap();
    assert!(!published.unpublished);
    assert!(
        execute(&target, Action::Undo, &published, &Input::default())
            .await
            .is_err()
    );
    let id = commit(root, "Local", "replacement\n");
    let target = key(root, id);
    let context = inspect(&target).await.unwrap();
    execute(&target, Action::Undo, &context, &Input::default())
        .await
        .unwrap();
    assert_eq!(
        command(root, &["diff", "--cached", "--name-only"]),
        "example.txt"
    );
}

#[tokio::test]
async fn detach_requires_clean_files_and_leaves_branch_tip_unchanged() {
    use super::super::tests::{command, repository};
    let dir = repository();
    let root = dir.path();
    let first = command(root, &["rev-parse", "HEAD"]);
    let latest = commit(root, "New", "content\n");
    let target = key(root, first.clone());
    let context = inspect(&target).await.unwrap();
    std::fs::write(root.join("example.txt"), "changed\n").unwrap();
    assert!(matches!(
        execute(&target, Action::Detach, &context, &Input::default()).await,
        Err(Error::Dirty)
    ));
    command(root, &["restore", "example.txt"]);
    execute(&target, Action::Detach, &context, &Input::default())
        .await
        .unwrap();
    assert_eq!(command(root, &["rev-parse", "HEAD"]), first);
    assert_eq!(command(root, &["rev-parse", "refs/heads/main"]), latest);
}

#[tokio::test]
async fn pull_uses_updated_remote_and_push_tag_sends_only_one_tag() {
    use super::super::tests::{command, remote, repository};
    let dir = repository();
    let root = dir.path();
    let bare = remote(root);
    let upstream = tempfile::tempdir().unwrap();
    command(
        upstream.path(),
        &["clone", "--", bare.path().to_str().unwrap(), "."],
    );
    command(upstream.path(), &["config", "user.name", "Test"]);
    command(
        upstream.path(),
        &["config", "user.email", "test@example.invalid"],
    );
    let new = commit(upstream.path(), "Remote update", "from remote\n");
    command(upstream.path(), &["push"]);

    let target = Target::Branch(BranchKey {
        root: root.to_owned(),
        reference: "refs/remotes/origin/main".into(),
    });
    let context = inspect(&target).await.unwrap();
    execute(&target, Action::PullMerge, &context, &Input::default())
        .await
        .unwrap();
    assert_eq!(command(root, &["rev-parse", "HEAD"]), new);
    let target = key(root, new);
    let context = inspect(&target).await.unwrap();
    let tag = Input {
        name: "preview".into(),
        message: "Description".into(),
        ..Default::default()
    };
    execute(&target, Action::Tag, &context, &tag).await.unwrap();
    command(root, &["tag", "unrelated"]);
    let target = Target::Branch(BranchKey {
        root: root.to_owned(),
        reference: "refs/tags/preview".into(),
    });
    let context = inspect(&target).await.unwrap();
    let push = Input {
        name: "origin".into(),
        ..Default::default()
    };
    execute(&target, Action::PushTag, &context, &push)
        .await
        .unwrap();
    assert_eq!(command(bare.path(), &["tag", "--list"]), "preview");
    execute(&target, Action::DeleteTag, &context, &Input::default())
        .await
        .unwrap();
    assert_eq!(command(root, &["tag", "--list"]), "unrelated");
    assert_eq!(command(bare.path(), &["tag", "--list"]), "preview");
}

#[tokio::test]
async fn patch_exports_a_commit_without_overwriting_an_existing_file() {
    use super::super::tests::repository;
    let dir = repository();
    let root = dir.path();
    let revision = commit(root, "Export example", "patch contents\n");
    let target = key(root, revision);
    let context = inspect(&target).await.unwrap();
    let output = tempfile::tempdir().unwrap();
    let path = output.path().join("change.patch");
    let input = Input {
        path: Some(path.clone()),
        ..Default::default()
    };
    execute(&target, Action::Patch, &context, &input)
        .await
        .unwrap();
    let patch = std::fs::read_to_string(&path).unwrap();
    assert!(patch.contains("Subject: [PATCH] Export example"));
    assert!(patch.contains("+patch contents"));
    assert!(
        execute(&target, Action::Patch, &context, &input)
            .await
            .is_err()
    );
    assert_eq!(std::fs::read_to_string(path).unwrap(), patch);
}

#[tokio::test]
async fn edit_message_preserves_commit_contents_and_staged_changes() {
    use super::super::tests::{command, repository};
    let dir = repository();
    let root = dir.path();
    let id = commit(root, "Original", "committed\n");
    let tree = command(root, &["rev-parse", "HEAD^{tree}"]);
    std::fs::write(root.join("example.txt"), "staged\n").unwrap();
    command(root, &["add", "example.txt"]);

    let target = key(root, id);
    let context = inspect(&target).await.unwrap();
    assert_eq!(context.message.as_deref(), Some("Original"));
    let input = Input {
        message: "Replacement\n\nCommit body".into(),
        ..Default::default()
    };
    execute(&target, Action::Message, &context, &input)
        .await
        .unwrap();
    assert_eq!(command(root, &["log", "-1", "--format=%B"]), input.message);
    assert_eq!(command(root, &["rev-parse", "HEAD^{tree}"]), tree);
    assert_eq!(
        command(root, &["diff", "--cached", "--name-only"]),
        "example.txt"
    );
}
