//! Read bounded Docker CLI output and preserve resource identities across refreshes.

use super::model::{Item, Kind, operand};
use crate::infrastructure::process;
use serde_json::Value;
use std::{path::Path, process::Stdio, time::Duration};
use tokio::{
    io::{AsyncRead, AsyncReadExt},
    process::Command,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Error {
    Missing,
    Unavailable,
    Timeout,
    Large,
    Invalid,
    Refused,
    InUse,
    Denied,
    NotFound,
    ComposeMissing,
}

async fn drain(mut stream: impl AsyncRead + Unpin, limit: usize) -> Result<(Vec<u8>, bool), Error> {
    let mut bytes = Vec::new();
    let mut chunk = [0; 8192];
    let mut truncated = false;
    loop {
        let n = stream
            .read(&mut chunk)
            .await
            .map_err(|_| Error::Unavailable)?;
        if n == 0 {
            return Ok((bytes, truncated));
        }
        let keep = n.min(limit - bytes.len());
        bytes.extend_from_slice(&chunk[..keep]);
        truncated |= keep < n;
    }
}

pub(super) async fn run(
    context: Option<&str>,
    args: &[String],
    root: &Path,
    write: bool,
    logs: bool,
) -> Result<String, Error> {
    let mut cmd = Command::new(process::executable("docker"));
    if let Some(name) = context {
        if !operand(name) {
            return Err(Error::Invalid);
        }
        cmd.args(["--context", name]);
    }
    // Named stack operations must not load an unrelated project's Compose file.
    let isolated = if args.first().is_some_and(|arg| arg == "compose")
        && args.get(1).is_some_and(|arg| arg == "--project-name")
    {
        cmd.env_remove("COMPOSE_FILE")
            .env_remove("COMPOSE_ENV_FILES");
        Some(tempfile::tempdir().map_err(|_| Error::Unavailable)?)
    } else {
        None
    };
    cmd.args(args)
        .current_dir(isolated.as_ref().map_or(root, |directory| directory.path()))
        .kill_on_drop(true)
        .stdin(Stdio::null())
        .env("NO_COLOR", "1")
        .env("COMPOSE_ANSI", "never")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    cmd.creation_flags(0x08000000);
    let mut child = cmd.spawn().map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            Error::Missing
        } else {
            Error::Unavailable
        }
    })?;
    let stdout = child.stdout.take().ok_or(Error::Unavailable)?;
    let stderr = child.stderr.take().ok_or(Error::Unavailable)?;
    tokio::time::timeout(Duration::from_secs(if write { 600 } else { 30 }), async {
        let (out, err, status) = tokio::join!(
            drain(stdout, 8 * 1024 * 1024),
            drain(stderr, 512 * 1024),
            child.wait()
        );
        let (mut out, truncated) = out?;
        let (err, err_truncated) = err?;
        if !status.map_err(|_| Error::Unavailable)?.success() {
            let summary = String::from_utf8_lossy(&err).to_lowercase();
            return Err(
                if summary.contains("unknown command")
                    && args.first().is_some_and(|arg| arg == "compose")
                {
                    Error::ComposeMissing
                } else if summary.contains("permission denied")
                    || summary.contains("unauthorized")
                    || summary.contains("access denied")
                {
                    Error::Denied
                } else if summary.contains("in use")
                    || summary.contains("active endpoints")
                    || summary.contains("conflict")
                    || summary.contains("already exists")
                {
                    Error::InUse
                } else if summary.contains("no such") || summary.contains("not found") {
                    if summary.contains("docker.sock") {
                        Error::Unavailable
                    } else {
                        Error::NotFound
                    }
                } else if summary.contains("connect") || summary.contains("daemon is running") {
                    Error::Unavailable
                } else {
                    Error::Refused
                },
            );
        }
        if truncated || logs && err_truncated {
            return Err(Error::Large);
        }
        if logs && !err.is_empty() {
            if !out.is_empty() {
                out.push(b'\n');
            }
            out.extend(err);
        }
        Ok(String::from_utf8_lossy(&out).into_owned())
    })
    .await
    .map_err(|_| Error::Timeout)?
}

pub(super) async fn contexts(root: &Path) -> Result<(Vec<String>, String), Error> {
    let output = run(
        None,
        &["context", "ls", "--format", "{{json .}}"].map(String::from),
        root,
        false,
        false,
    )
    .await?;
    let mut names = Vec::new();
    let mut active = None;
    for line in output.lines() {
        let value: Value = serde_json::from_str(line).map_err(|_| Error::Invalid)?;
        let name = value["Name"]
            .as_str()
            .filter(|name| operand(name))
            .ok_or(Error::Invalid)?;
        if value["Current"] == true || value["Current"] == "true" || value["Current"] == "*" {
            active = Some(name.to_owned());
        }
        names.push(name.to_owned());
    }
    let current = active
        .or_else(|| names.first().cloned())
        .ok_or(Error::Invalid)?;
    Ok((names, current))
}

pub(super) fn parse(kind: Kind, output: &str) -> Result<Vec<Item>, Error> {
    let values = if output.trim_start().starts_with('[') {
        serde_json::from_str::<Vec<Value>>(output).map_err(|_| Error::Invalid)?
    } else {
        output
            .lines()
            .filter(|line| !line.trim().is_empty())
            .map(serde_json::from_str)
            .collect::<Result<Vec<Value>, _>>()
            .map_err(|_| Error::Invalid)?
    };
    let mut items = Vec::with_capacity(values.len());
    for row in values {
        let get = |key: &str| row.get(key).and_then(Value::as_str).unwrap_or_default();
        let (id, name, detail, status) = match kind {
            Kind::Containers => (
                get("ID").into(),
                get("Names").into(),
                get("Image"),
                get("Status"),
            ),
            Kind::Images => {
                let tag = if get("Repository") == "<none>" || get("Tag") == "<none>" {
                    get("ID").into()
                } else {
                    format!("{}:{}", get("Repository"), get("Tag"))
                };
                (tag.clone(), tag, get("ID"), get("Size"))
            }
            Kind::Networks => (
                get("ID").into(),
                get("Name").into(),
                get("Driver"),
                get("Scope"),
            ),
            Kind::Volumes => (
                get("Name").into(),
                get("Name").into(),
                get("Driver"),
                get("Scope"),
            ),
            Kind::Compose => (
                get("Name").into(),
                get("Name").into(),
                get("ConfigFiles"),
                get("Status"),
            ),
        };
        if !operand(&id) || name.is_empty() {
            return Err(Error::Invalid);
        }
        let search = format!("{name} {detail} {status} {id}").to_lowercase();
        items.push(Item {
            id,
            name,
            detail: detail.into(),
            status: status.into(),
            state: get("State").into(),
            search,
        });
    }
    items.sort_unstable_by(|a, b| a.name.cmp(&b.name).then(a.id.cmp(&b.id)));
    Ok(items)
}
