use super::*;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[tokio::test]
async fn closed_catalog_allows_exact_repository_and_paged_tags() {
    let server = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = server.local_addr().unwrap();
    let reg = Registry::new("test", &format!("http://{addr}"), false).unwrap();
    let worker = tokio::spawn(async move {
        for (path, status, link, body) in [
            ("/v2/_catalog?n=40", "403 Forbidden", "", "{}"),
            (
                "/v2/team/app/tags/list?n=40",
                "200 OK",
                "Link: </v2/team/app/tags/list?n=40&last=a>; rel=\"next\"\r\n",
                r#"{"tags":["a"]}"#,
            ),
            (
                "/v2/team/app/tags/list?n=40&last=a",
                "200 OK",
                "",
                r#"{"tags":["b"]}"#,
            ),
        ] {
            let (mut socket, _) = server.accept().await.unwrap();
            let mut buf = [0; 4096];
            let n = socket.read(&mut buf).await.unwrap();
            assert!(String::from_utf8_lossy(&buf[..n]).starts_with(&format!("GET {path} ")));
            socket.write_all(format!("HTTP/1.1 {status}\r\n{link}Content-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).as_bytes()).await.unwrap();
        }
    });
    assert_eq!(
        api::list(&reg, None, None).await.unwrap_err(),
        Error::Denied
    );
    let first = api::list(&reg, Some("team/app"), None).await.unwrap();
    assert_eq!(first.names, ["a"]);
    let second = api::list(&reg, Some("team/app"), first.next.as_deref())
        .await
        .unwrap();
    assert_eq!(second.names, ["b"]);
    assert!(second.next.is_none());
    worker.await.unwrap();
    assert_eq!(
        api::list(&reg, None, Some("https://elsewhere.example/v2/_catalog"))
            .await
            .unwrap_err(),
        Error::Invalid
    );
}
#[test]
fn stale_registry_result_cannot_replace_the_new_selection() {
    let first = Registry::default();
    let second = Registry::new("private", "https://registry.example", false).unwrap();
    let mut profiles = vec![first.clone(), second.clone()];
    let mut s = State::default();
    let root = Path::new(".");
    let _ = s.update(Event::Select(first), &mut profiles, None, root);
    let old = s.revision;
    let _ = s.update(Event::Select(second.clone()), &mut profiles, None, root);
    let _ = s.update(
        Event::Listed(
            old,
            Ok(Page {
                names: vec!["stale".into()],
                next: None,
            }),
        ),
        &mut profiles,
        None,
        root,
    );
    assert!(s.page.names.is_empty());
    assert_eq!(s.selected, Some(second));
}
#[tokio::test]
async fn bearer_exchange_accepts_both_token_fields_returned_by_docker_hub() {
    let server = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = server.local_addr().unwrap();
    let reg = Registry::new("fixture", &format!("http://{addr}"), false).unwrap();
    let worker = tokio::spawn(async move {
        for turn in 0..3 {
            let (mut socket, _) = server.accept().await.unwrap();
            let mut request = Vec::new();
            while !request.windows(4).any(|w| w == b"\r\n\r\n") {
                let mut chunk = [0; 1024];
                let n = socket.read(&mut chunk).await.unwrap();
                assert_ne!(n, 0);
                request.extend_from_slice(&chunk[..n]);
            }
            let request = String::from_utf8(request).unwrap().to_lowercase();
            let (status, headers, body) = match turn {
                0 => (
                    "401 Unauthorized",
                    format!(
                        "WWW-Authenticate: Bearer realm=\"http://{addr}/token\",service=\"fixture\"\r\n"
                    ),
                    "",
                ),
                1 => {
                    assert!(request.starts_with("get /token?"));
                    (
                        "200 OK",
                        String::new(),
                        r#"{"token":"fixture","access_token":"fixture"}"#,
                    )
                }
                _ => {
                    assert!(request.contains("authorization: bearer fixture"));
                    ("200 OK", String::new(), r#"{"tags":["stable"]}"#)
                }
            };
            socket.write_all(format!("HTTP/1.1 {status}\r\n{headers}Connection: close\r\nContent-Length: {}\r\n\r\n{body}",body.len()).as_bytes()).await.unwrap();
        }
    });
    assert_eq!(
        api::list(&reg, Some("team/web"), None).await.unwrap().names,
        ["stable"]
    );
    worker.await.unwrap();
}
