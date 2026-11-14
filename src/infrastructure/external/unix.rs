//! Local socket endpoint for request forwarding on Unix platforms.

use super::deliver;
use crate::infrastructure::ipc::{Line, read_line};
use std::{
    io::Write,
    os::unix::fs::PermissionsExt,
    os::unix::net::{UnixListener as StdListener, UnixStream},
    path::{Path, PathBuf},
};
use tokio::io::BufReader;

/// The bound local socket that later instances forward requests to.
pub(super) struct Listener(StdListener);

const ENDPOINT_NAME: &str = "external.sock";

/// Cap one forwarded payload, matching the other local transports.
const PAYLOAD_LIMIT: usize = 1024 * 1024;

fn endpoint_path() -> Option<PathBuf> {
    endpoint_parent().map(|directory| directory.join(ENDPOINT_NAME))
}

/// The directory that holds the local endpoint, shared with saved settings.
fn storage_directory() -> Option<PathBuf> {
    if let Some(directory) = std::env::var_os("IWE_DATA_DIR") {
        return Some(PathBuf::from(directory));
    }
    directories::ProjectDirs::from("", "", "iwe").map(|dirs| dirs.config_dir().to_path_buf())
}

/// Prepare the endpoint parent directory, restricted to the current user.
fn endpoint_parent() -> Option<PathBuf> {
    let directory = storage_directory()?;
    std::fs::create_dir_all(&directory).ok()?;
    let _ = std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o700));
    Some(directory)
}

/// Bind the local socket so later instances can forward requests.
///
/// Forwarding has already failed, so any leftover socket file belongs to a
/// crashed instance and is reclaimed here. The socket is restricted to the
/// current user: connecting requires write permission on the socket file.
pub(super) fn bind() -> Option<Listener> {
    let path = endpoint_path()?;
    bind_at(&path)
}

fn bind_at(path: &Path) -> Option<Listener> {
    let _ = std::fs::remove_file(path);
    let listener = StdListener::bind(path).ok()?;
    let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
    Some(Listener(listener))
}

/// Remove the socket file so no stale endpoint survives this process.
pub(super) fn release() {
    if let Some(path) = endpoint_path() {
        release_at(&path);
    }
}

fn release_at(path: &Path) {
    let _ = std::fs::remove_file(path);
}

/// Send the payload to the running instance.
pub(super) fn write_payload(payload: &[u8]) -> bool {
    match endpoint_path() {
        Some(path) => write_payload_at(&path, payload),
        None => false,
    }
}

fn write_payload_at(path: &Path, payload: &[u8]) -> bool {
    let Ok(mut stream) = UnixStream::connect(path) else {
        return false;
    };
    stream.write_all(payload).is_ok()
}

/// Accept forwarded requests until the application exits.
pub(super) async fn accept(listener: Option<Listener>) {
    let Some(Listener(listener)) = listener else {
        return;
    };
    // Tokio refuses to register a blocking socket.
    if listener.set_nonblocking(true).is_err() {
        return;
    }
    let Ok(listener) = tokio::net::UnixListener::from_std(listener) else {
        return;
    };
    loop {
        let Ok((stream, _)) = listener.accept().await else {
            return;
        };
        let mut reader = BufReader::new(stream);
        if let Ok(Some(Line::Text(line))) = read_line(&mut reader, PAYLOAD_LIMIT).await {
            deliver(&line).await;
        }
    }
}

#[cfg(test)]
mod tests;
