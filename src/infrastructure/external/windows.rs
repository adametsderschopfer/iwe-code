//! Named pipe endpoint for request forwarding on Windows.

use super::deliver;
use std::{fs::File, io::Write, time::Duration};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::windows::named_pipe::ServerOptions;

/// The named pipe state handed to the accept loop.
///
/// Windows creates the first pipe instance inside the tokio runtime, so this
/// type carries no bound resource and the loop creates instances itself.
pub(super) struct Listener;

const PIPE_NAME: &str = r"\\.\pipe\iwe-code-external";
const PAYLOAD_LIMIT: u64 = 1024 * 1024;
const CONNECT_ATTEMPTS: usize = 5;

/// Reserve the forwarding role for this instance.
///
/// The first pipe instance is created when the accept loop starts. This keeps
/// the startup path free of runtime requirements.
pub(super) fn bind() -> Option<Listener> {
    Some(Listener)
}

/// Send the payload to the running instance.
pub(super) fn write_payload(payload: &[u8]) -> bool {
    for _ in 0..CONNECT_ATTEMPTS {
        match File::create(PIPE_NAME) {
            // The handle must close so the running instance sees the payload.
            Ok(mut stream) => return stream.write_all(payload).is_ok(),
            // The running instance may still be recreating its pipe instance.
            Err(_) => std::thread::sleep(Duration::from_millis(20)),
        }
    }
    false
}

/// Accept forwarded requests until the application exits.
///
/// Each accepted connection serves exactly one payload. The loop recreates
/// the server instance so the next launch can always connect.
pub(super) async fn accept(listener: Option<Listener>) {
    if listener.is_none() {
        return;
    }
    loop {
        let Ok(server) = ServerOptions::new().create(PIPE_NAME) else {
            return;
        };
        if server.connect().await.is_err() {
            return;
        }
        let mut payload = Vec::new();
        let mut limited = server.take(PAYLOAD_LIMIT);
        let read = limited.read_to_end(&mut payload).await;
        let mut server = limited.into_inner();
        if read.is_ok() {
            let text = String::from_utf8_lossy(&payload);
            if let Some(line) = text.lines().next() {
                deliver(line).await;
            }
        }
        let _ = server.disconnect();
    }
}
