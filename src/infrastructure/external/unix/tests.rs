//! Checks for socket binding, permissions, and payload delivery.

use super::{Listener, bind_at, release_at, write_payload_at};
use std::io::{BufRead, BufReader};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

fn socket_path(directory: &Path) -> PathBuf {
    directory.join("external.sock")
}

#[test]
fn bind_restricts_the_socket_to_the_owner() {
    let directory = tempfile::tempdir().expect("tempdir");
    let path = socket_path(directory.path());
    bind_at(&path).expect("bound");
    let mode = std::fs::metadata(&path)
        .expect("socket file")
        .permissions()
        .mode();
    assert_eq!(mode & 0o777, 0o600);
}

#[test]
fn bind_reclaims_a_stale_endpoint() {
    let directory = tempfile::tempdir().expect("tempdir");
    let path = socket_path(directory.path());
    std::fs::write(&path, "left by a crashed instance").expect("write");
    bind_at(&path).expect("bound over the stale file");
}

#[test]
fn release_removes_the_socket_file() {
    let directory = tempfile::tempdir().expect("tempdir");
    let path = socket_path(directory.path());
    bind_at(&path).expect("bound");
    release_at(&path);
    assert!(!path.exists());
}

#[test]
fn payload_reaches_a_live_listener() {
    let directory = tempfile::tempdir().expect("tempdir");
    let path = socket_path(directory.path());
    let Listener(listener) = bind_at(&path).expect("bound");
    let payload = br#"{"type":"focus"}"#;
    assert!(write_payload_at(&path, payload));

    let (stream, _) = listener.accept().expect("queued connection");
    let mut line = String::new();
    BufReader::new(stream)
        .read_line(&mut line)
        .expect("payload line");
    assert_eq!(line.trim_end(), r#"{"type":"focus"}"#);
}

#[test]
fn payload_fails_without_a_listener() {
    let directory = tempfile::tempdir().expect("tempdir");
    let path = socket_path(directory.path());
    assert!(!write_payload_at(&path, b"{}"));
}
