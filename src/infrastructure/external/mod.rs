//! Receive requests from outside the application process.
//!
//! Requests arrive from command-line arguments, Apple Events on macOS, a
//! second application instance, and files dropped on the window. The
//! [`startup`] function decides whether this process accepts requests or
//! forwards them to a running instance.

#[cfg(target_os = "macos")]
mod macos;

use crate::domain::external::{Link, Request};
use std::{
    ffi::OsString,
    path::PathBuf,
    sync::{Mutex, OnceLock},
};
use tokio::sync::mpsc;

/// The result of preparing this process to receive external requests.
pub(crate) enum Startup {
    /// A running instance received the requests, and this process exits.
    Forwarded,
    /// This instance accepts requests.
    Primary,
}

static SENDER: OnceLock<mpsc::UnboundedSender<Request>> = OnceLock::new();
static RECEIVER: Mutex<Option<mpsc::UnboundedReceiver<Request>>> = Mutex::new(None);
static LISTENER: Mutex<Option<Listener>> = Mutex::new(None);

#[cfg(unix)]
use unix::Listener;
#[cfg(windows)]
use windows::Listener;

/// Parse the command line, forward requests to a running instance, or claim
/// the receiver and listener for this process.
///
/// A launch without requests still forwards a focus request, so a second
/// launch always raises the running instance instead of competing with it.
pub(crate) fn startup() -> Startup {
    let requests = requests_from_args(std::env::args_os().skip(1));
    let requests = if requests.is_empty() {
        vec![Request::Focus]
    } else {
        requests
    };
    if forward(&requests) {
        return Startup::Forwarded;
    }
    let (sender, receiver) = mpsc::unbounded_channel();
    if let Ok(mut slot) = RECEIVER.lock() {
        *slot = Some(receiver);
    }
    let _ = SENDER.set(sender);
    #[cfg(target_os = "macos")]
    macos::install();
    for request in requests.clone() {
        send(request);
    }
    let listener = bind();
    if listener.is_none() && forward(&requests) {
        // Another process claimed the endpoint between the forward attempt
        // and the bind. It serves the requests instead.
        return Startup::Forwarded;
    }
    if let Ok(mut slot) = LISTENER.lock() {
        *slot = listener;
    }
    Startup::Primary
}

/// Release the local endpoint before the application exits.
pub(crate) fn shutdown() {
    #[cfg(unix)]
    unix::release();
}

/// Take the receiver that turns external requests into application messages.
///
/// The application subscription calls this once. Later calls receive nothing.
pub(crate) fn take_receiver() -> Option<mpsc::UnboundedReceiver<Request>> {
    let mut slot = RECEIVER.lock().ok()?;
    slot.take()
}

/// Deliver a request to the application, buffering it until the subscription
/// consumes it.
pub(crate) fn send(request: Request) {
    if let Some(sender) = SENDER.get() {
        let _ = sender.send(request);
    }
}

/// Start accepting connections from later instances on the current runtime.
///
/// The application subscription calls this once, inside the tokio runtime.
pub(crate) fn serve() {
    let listener = LISTENER.lock().ok().and_then(|mut slot| slot.take());
    tokio::spawn(async move {
        accept(listener).await;
    });
}

/// Classify one command-line argument.
///
/// Deep links parse strictly. Flags and directories stay out of the way so
/// that update flags and the legacy project-directory argument keep working.
fn request_from_arg(arg: &OsString) -> Option<Request> {
    let text = arg.to_string_lossy();
    if let Some(link) = Link::parse(&text) {
        return Some(match link {
            Link::Open(path) => Request::File { path },
            Link::Project(directory) => Request::Project { directory },
        });
    }
    if text.starts_with('-') {
        return None;
    }
    let path = PathBuf::from(arg);
    path.is_file().then_some(Request::File { path })
}

fn requests_from_args(args: impl IntoIterator<Item = OsString>) -> Vec<Request> {
    args.into_iter()
        .filter_map(|arg| request_from_arg(&arg))
        .collect()
}

/// Send the requests to a running instance over the local endpoint.
fn forward(requests: &[Request]) -> bool {
    let Ok(mut payload) = serde_json::to_string(requests) else {
        return false;
    };
    payload.push('\n');
    write_payload(payload.as_bytes())
}

/// Read one line-delimited JSON payload and deliver its requests.
async fn deliver(line: &str) {
    let Ok(requests) = serde_json::from_str::<Vec<Request>>(line) else {
        return;
    };
    for request in requests {
        send(request);
    }
}

/// The directory that holds the local endpoint, shared with saved settings.
fn storage_directory() -> Option<PathBuf> {
    if let Some(directory) = std::env::var_os("IWE_DATA_DIR") {
        return Some(PathBuf::from(directory));
    }
    directories::ProjectDirs::from("", "", "iwe").map(|dirs| dirs.config_dir().to_path_buf())
}

/// Prepare the parent directory for platform endpoint functions.
///
/// The directory holds the forwarding endpoint, so it is restricted to the
/// current user where the platform supports directory modes.
pub(super) fn endpoint_parent() -> Option<PathBuf> {
    let directory = storage_directory()?;
    std::fs::create_dir_all(&directory).ok()?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o700));
    }
    Some(directory)
}

#[cfg(unix)]
mod unix;
#[cfg(unix)]
use unix::{accept, bind, write_payload};

#[cfg(windows)]
mod windows;
#[cfg(windows)]
use windows::{accept, bind, write_payload};

#[cfg(test)]
mod tests;
