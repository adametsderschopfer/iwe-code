//! Format unsaved documents with the project's configured formatter.

mod discovery;

#[cfg(test)]
mod tests;

use crate::domain::editor::LIMIT;
use std::{io, path::Path, process::Stdio, time::Duration};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

pub(crate) async fn format(root: &Path, path: &Path, text: &str) -> io::Result<Option<String>> {
    let root = root.to_owned();
    let path = path.to_owned();
    let plan = tokio::task::spawn_blocking(move || discovery::discover(&root, &path))
        .await
        .map_err(io::Error::other)??;
    let Some(plan) = plan else {
        return Ok(None);
    };
    run(&plan, text).await.map(Some)
}

async fn run(plan: &discovery::Plan, text: &str) -> io::Result<String> {
    if text.len() > LIMIT {
        return Err(io::ErrorKind::InvalidInput.into());
    }
    let mut command = tokio::process::Command::new(&plan.program);
    command
        .args(&plan.arguments)
        .current_dir(&plan.directory)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env("NO_COLOR", "1")
        .kill_on_drop(true);
    #[cfg(windows)]
    command.creation_flags(0x08000000);
    let operation = async {
        let mut child = command.spawn()?;
        let mut input = child.stdin.take().ok_or(io::ErrorKind::BrokenPipe)?;
        let output = child.stdout.take().ok_or(io::ErrorKind::BrokenPipe)?;
        let errors = child.stderr.take().ok_or(io::ErrorKind::BrokenPipe)?;
        let mut bytes = Vec::new();
        let mut error_bytes = Vec::new();
        let (written, read, drained) = tokio::join!(
            async {
                input.write_all(text.as_bytes()).await?;
                drop(input);
                Ok::<_, io::Error>(())
            },
            async { output.take(LIMIT as u64 + 1).read_to_end(&mut bytes).await },
            async {
                errors
                    .take(64 * 1024 + 1)
                    .read_to_end(&mut error_bytes)
                    .await
            },
        );
        written?;
        read?;
        drained?;
        if bytes.len() > LIMIT || error_bytes.len() > 64 * 1024 {
            return Err(io::ErrorKind::InvalidData.into());
        }
        if !child.wait().await?.success() {
            return Err(io::ErrorKind::InvalidData.into());
        }
        let formatted = String::from_utf8(bytes).map_err(|_| io::ErrorKind::InvalidData)?;
        if formatted.is_empty() && !text.trim().is_empty() {
            return Err(io::ErrorKind::InvalidData.into());
        }
        Ok(formatted)
    };
    tokio::time::timeout(Duration::from_secs(25), operation)
        .await
        .map_err(|_| io::ErrorKind::TimedOut)?
}
