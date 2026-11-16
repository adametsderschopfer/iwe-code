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
    /// A path prefix that stays absolute on every platform.
    #[cfg(windows)]
    const ROOT: &str = "C:/notes-work";
    #[cfg(not(windows))]
    const ROOT: &str = "/notes-work";

    let path = format!("{ROOT}/ideas.md");
    let request = request_from_arg(&arg(&format!("iwe://open?path={path}"))).expect("file request");
    assert_eq!(
        request,
        Request::File {
            path: PathBuf::from(path)
        }
    );
}

#[test]
fn classify_project_link_argument() {
    /// A path prefix that stays absolute on every platform.
    #[cfg(windows)]
    const ROOT: &str = "C:/notes-work";
    #[cfg(not(windows))]
    const ROOT: &str = "/notes-work";

    let path = format!("{ROOT}/work/site");
    let request =
        request_from_arg(&arg(&format!("iwe://project?path={path}"))).expect("project request");
    assert_eq!(
        request,
        Request::Project {
            directory: PathBuf::from(path)
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

#[tokio::test]
async fn inspection_requires_the_key_and_answers_only_the_requester() {
    use super::{Event, Inspection, handle_connection};
    use tokio::io::{AsyncReadExt, AsyncWriteExt, duplex};

    let key = [42; 32];
    let (sender, mut events) = tokio::sync::mpsc::unbounded_channel();
    let (host, mut client) = duplex(4096);
    let payload = serde_json::to_string(&Inspection {
        window_key: [0; 32],
        include_text: true,
    })
    .unwrap();
    client
        .write_all(format!("{payload}\n").as_bytes())
        .await
        .unwrap();
    assert!(handle_connection(host, Some(&key), &sender).await.is_none());
    assert!(events.try_recv().is_err());

    let (host, mut client) = duplex(4096);
    let payload = serde_json::to_string(&Inspection {
        window_key: key,
        include_text: true,
    })
    .unwrap();
    client
        .write_all(format!("{payload}\n").as_bytes())
        .await
        .unwrap();
    let server = handle_connection(host, Some(&key), &sender);
    let request = async {
        let Some(Event::Inspect(true, reply)) = events.recv().await else {
            panic!("inspection request");
        };
        reply
            .send(serde_json::json!({"state": "ready"}))
            .await
            .unwrap();
        let mut reply = [0; 18];
        client.read_exact(&mut reply).await.unwrap();
        assert_eq!(&reply, b"{\"state\":\"ready\"}\n");
        client.write_all(b"\n").await.unwrap();
    };
    let (result, ()) = tokio::time::timeout(std::time::Duration::from_secs(2), async {
        tokio::join!(server, request)
    })
    .await
    .expect("window query completed");
    assert!(result.is_some());
}
