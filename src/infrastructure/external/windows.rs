//! Named pipe endpoint for request forwarding on Windows.

use super::deliver;
use std::{fs::File, io::Write, time::Duration};
use tokio::io::AsyncReadExt;
use tokio::net::windows::named_pipe::{NamedPipeServer, ServerOptions};

/// The server end of the first named pipe instance.
///
/// Startup creates the first instance to reserve the pipe name, so a later
/// launch cannot take over the forwarding role of this process.
pub(super) struct Listener(NamedPipeServer);

const PIPE_NAME: &str = r"\\.\pipe\iwe-code-external";
const PAYLOAD_LIMIT: u64 = 1024 * 1024;
const CONNECT_ATTEMPTS: usize = 5;
const RETRY_DELAY: Duration = Duration::from_millis(20);

/// Create the first pipe instance and reserve the forwarding role.
///
/// Fails while another instance owns the pipe name. The default security
/// descriptor of a named pipe is not restricted to the current user, and
/// restricting it needs security-descriptor access outside tokio. Remote
/// clients are rejected and payloads are size-capped.
pub(super) fn bind() -> Option<Listener> {
    first_instance().map(Listener)
}

/// Send the payload to the running instance.
pub(super) fn write_payload(payload: &[u8]) -> bool {
    for _ in 0..CONNECT_ATTEMPTS {
        match File::create(PIPE_NAME) {
            // The handle must close so the running instance sees the payload.
            Ok(mut stream) => return stream.write_all(payload).is_ok(),
            // The running instance may still be recreating its pipe instance.
            Err(_) => std::thread::sleep(RETRY_DELAY),
        }
    }
    false
}

/// Accept forwarded requests until the application exits.
///
/// Startup hands over the reserved first instance. Every later connection
/// gets a freshly created instance, so the next launch can always connect.
pub(super) async fn accept(listener: Option<Listener>) {
    let Some(Listener(server)) = listener else {
        return;
    };
    serve_instance(server).await;
    let mut failures = 0;
    loop {
        match ServerOptions::new()
            .reject_remote_clients(true)
            .create(PIPE_NAME)
        {
            Ok(server) => {
                failures = 0;
                serve_instance(server).await;
            }
            // The previous instance lingers until its client has closed.
            Err(_) => {
                failures += 1;
                if failures >= CONNECT_ATTEMPTS {
                    return;
                }
                tokio::time::sleep(RETRY_DELAY).await;
            }
        }
    }
}

/// Wait for one client, read its single payload, and deliver it.
async fn serve_instance(server: NamedPipeServer) {
    if server.connect().await.is_err() {
        return;
    }
    let mut payload = Vec::new();
    let mut limited = server.take(PAYLOAD_LIMIT);
    let read = limited.read_to_end(&mut payload).await;
    let server = limited.into_inner();
    if read.is_ok() {
        let text = String::from_utf8_lossy(&payload);
        if let Some(line) = text.lines().next() {
            deliver(line).await;
        }
    }
    let _ = server.disconnect();
}

/// Create the pipe instance that reserves the forwarding role.
fn first_instance() -> Option<NamedPipeServer> {
    ServerOptions::new()
        .first_pipe_instance(true)
        .reject_remote_clients(true)
        .create(PIPE_NAME)
        .ok()
}
