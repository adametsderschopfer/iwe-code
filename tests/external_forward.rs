//! Forwarding between two application instances over the local endpoint.
//!
//! The companion instance exits before any window opens, so the test runs
//! without a graphical session on the Unix continuous-integration runners.
//! The Unix socket transport keeps this target Unix-only; Windows forwarding
//! is exercised through the release CI build instead.
#![cfg(unix)]

use std::io::{ErrorKind, Read};
use std::os::unix::net::UnixListener;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// Run the application once against a listening endpoint and return the
/// request payload it forwarded.
fn forwarded_payload(arguments: &[PathBuf]) -> serde_json::Value {
    let directory = tempfile::tempdir().expect("isolated data directory");
    let endpoint = directory.path().join("external.sock");
    let listener = UnixListener::bind(&endpoint).expect("bind the endpoint");
    listener
        .set_nonblocking(true)
        .expect("nonblocking endpoint");

    let mut child = Command::new(env!("CARGO_BIN_EXE_iwe"))
        .args(arguments)
        .env("IWE_DATA_DIR", directory.path())
        .env("IWE_LANG", "en")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("run the application");

    let deadline = Instant::now() + Duration::from_secs(30);
    let (stream, _) = loop {
        match listener.accept() {
            Ok(pair) => break pair,
            Err(error) if error.kind() == ErrorKind::WouldBlock => {
                assert!(
                    Instant::now() < deadline,
                    "the application never reached the endpoint"
                );
                std::thread::sleep(Duration::from_millis(20));
            }
            Err(error) => panic!("accept failed: {error}"),
        }
    };
    let _ = stream.set_nonblocking(true);

    let read_deadline = Instant::now() + Duration::from_secs(10);
    let mut bytes = Vec::new();
    let mut buffer = [0; 512];
    loop {
        match (&stream).read(&mut buffer) {
            Ok(0) => break,
            Ok(count) => {
                bytes.extend_from_slice(&buffer[..count]);
                if bytes.contains(&b'\n') {
                    break;
                }
            }
            Err(error) if error.kind() == ErrorKind::WouldBlock => {
                assert!(
                    Instant::now() < read_deadline,
                    "the application never forwarded a payload"
                );
                std::thread::sleep(Duration::from_millis(20));
            }
            Err(error) => panic!("read failed: {error}"),
        }
    }

    let status = child.wait().expect("wait for the application");
    assert!(
        status.success(),
        "forwarding exits successfully, got {status}"
    );

    let line = String::from_utf8_lossy(&bytes);
    let requests: Vec<serde_json::Value> =
        serde_json::from_str(line.trim_end()).expect("forwarded requests");
    assert_eq!(requests.len(), 1, "one request per payload");
    requests.into_iter().next().expect("the request")
}

#[test]
fn a_file_argument_reaches_the_running_instance() {
    let directory = tempfile::tempdir().expect("isolated data directory");
    let notes = directory.path().join("notes.md");
    std::fs::write(&notes, "content").expect("write the file");

    let request = forwarded_payload(std::slice::from_ref(&notes));
    assert_eq!(request["type"], "file");
    let path = request["path"].as_str().expect("file path");
    assert_eq!(Path::new(path), notes.as_path());
}

#[test]
fn a_launch_without_arguments_focuses_the_running_instance() {
    let request = forwarded_payload(&[]);
    assert_eq!(request["type"], "focus");
}

#[cfg(target_os = "linux")]
#[test]
fn first_launch_reaches_graphics_initialization_without_a_running_instance() {
    let directory = tempfile::tempdir().expect("isolated data directory");
    let output = Command::new(env!("CARGO_BIN_EXE_iwe"))
        .env("IWE_DATA_DIR", directory.path())
        .env_remove("IWE_GRAPHICS_CHILD")
        .env_remove("DISPLAY")
        .env_remove("WAYLAND_DISPLAY")
        .env_remove("WAYLAND_SOCKET")
        .env_remove("XDG_RUNTIME_DIR")
        .output()
        .expect("run without a graphical session");

    assert!(
        !output.status.success(),
        "a first launch without a display must report a startup failure"
    );
}
