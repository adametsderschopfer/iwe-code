//! Named pipe endpoint for request forwarding on Windows.

use super::{Key, serve_connection};
use std::{fs::File, io::Write, time::Duration};
use tokio::net::windows::named_pipe::{
    ClientOptions, NamedPipeClient, NamedPipeServer, ServerOptions,
};

/// The server end of the first named pipe instance.
///
/// Startup creates the first instance to reserve the pipe name, so a later
/// launch cannot take over the forwarding role of this process.
pub(super) struct Listener(NamedPipeServer);

const PIPE_NAME: &str = r"\\.\pipe\iwe-code-external";
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

/// Keep the first pipe instance reserved while serving local requests.
pub(super) async fn accept(Listener(mut server): Listener, key: Option<Key>) {
    loop {
        if server.connect().await.is_err() {
            return;
        }
        serve_connection(&mut server, key.as_ref()).await;
        if server.disconnect().is_err() {
            return;
        }
    }
}

pub(super) async fn connect() -> std::io::Result<NamedPipeClient> {
    for _ in 1..CONNECT_ATTEMPTS {
        match ClientOptions::new().open(PIPE_NAME) {
            Ok(client) => return Ok(client),
            Err(_) => tokio::time::sleep(RETRY_DELAY).await,
        }
    }
    ClientOptions::new().open(PIPE_NAME)
}

/// Create the pipe instance that reserves the forwarding role.
fn first_instance() -> Option<NamedPipeServer> {
    ServerOptions::new()
        .first_pipe_instance(true)
        .reject_remote_clients(true)
        .create(PIPE_NAME)
        .ok()
}
