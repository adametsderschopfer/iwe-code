//! Local socket endpoint for request forwarding on Unix platforms.

use super::deliver;
use std::{
    io::Write,
    os::unix::net::{UnixListener as StdListener, UnixStream},
    path::PathBuf,
};

/// The bound local socket that later instances forward requests to.
pub(super) struct Listener(StdListener);

const ENDPOINT_NAME: &str = "external.sock";

fn endpoint_path() -> Option<PathBuf> {
    super::endpoint_parent().map(|directory| directory.join(ENDPOINT_NAME))
}

/// Bind the local socket so later instances can forward requests.
pub(super) fn bind() -> Option<Listener> {
    let path = endpoint_path()?;
    // A crashed instance can leave the socket file behind. Reclaim it.
    let _ = std::fs::remove_file(&path);
    let listener = StdListener::bind(&path).ok()?;
    Some(Listener(listener))
}

/// Send the payload to the running instance.
pub(super) fn write_payload(payload: &[u8]) -> bool {
    let Some(path) = endpoint_path() else {
        return false;
    };
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
        if let Ok(line) = read_line(stream).await {
            deliver(&line).await;
        }
    }
}

async fn read_line(stream: tokio::net::UnixStream) -> std::io::Result<String> {
    use tokio::io::{AsyncBufReadExt, BufReader};
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    reader.read_line(&mut line).await?;
    Ok(line)
}
