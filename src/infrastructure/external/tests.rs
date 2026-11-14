use super::request_from_arg;
use crate::domain::external::Request;
use std::{ffi::OsString, path::PathBuf};

fn arg(value: &str) -> OsString {
    OsString::from(value)
}

#[test]
fn classify_existing_file_argument() {
    let directory = tempfile::tempdir().expect("tempdir");
    let file = directory.path().join("notes.md");
    std::fs::write(&file, "content").expect("write");
    let request = request_from_arg(&arg(file.to_str().expect("utf8"))).expect("file request");
    assert_eq!(
        request,
        Request::File {
            path: PathBuf::from(&file)
        }
    );
}

#[test]
fn ignore_directory_arguments() {
    let directory = tempfile::tempdir().expect("tempdir");
    assert_eq!(
        request_from_arg(&arg(directory.path().to_str().expect("utf8"))),
        None
    );
}

#[test]
fn ignore_missing_paths_and_flags() {
    assert_eq!(request_from_arg(&arg("/missing/notes.md")), None);
    assert_eq!(request_from_arg(&arg("--iwe-install-update")), None);
    assert_eq!(request_from_arg(&arg("-v")), None);
}

#[test]
fn classify_open_link_argument() {
    let request = request_from_arg(&arg("iwe://open?path=/notes/ideas.md")).expect("file request");
    assert_eq!(
        request,
        Request::File {
            path: PathBuf::from("/notes/ideas.md")
        }
    );
}

#[test]
fn classify_project_link_argument() {
    let request = request_from_arg(&arg("iwe://project?path=/work/site")).expect("project request");
    assert_eq!(
        request,
        Request::Project {
            directory: PathBuf::from("/work/site")
        }
    );
}

#[test]
fn ignore_broken_links_and_other_schemes() {
    assert_eq!(request_from_arg(&arg("iwe://open")), None);
    assert_eq!(
        request_from_arg(&arg("https://open?path=/notes/a.txt")),
        None
    );
}

#[test]
fn update_flag_paths_stay_out_of_requests() {
    // The update flow passes directory paths with its own flags.
    assert_eq!(request_from_arg(&arg("/tmp/.iwe-update-stage")), None);
}
