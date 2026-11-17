use super::*;
use serde_json::json;

#[tokio::test]
async fn reads_consecutive_messages_with_unicode_payloads() {
    let messages = [
        json!({"text": "привет 🦀"}),
        json!({"id": 2, "result": null}),
    ];
    let mut bytes = Vec::new();
    for message in &messages {
        write(&mut bytes, message).await.unwrap();
    }
    let mut input = bytes.as_slice();
    for message in messages {
        assert_eq!(read(&mut input).await.unwrap(), message);
    }
    assert!(input.is_empty());
}

#[tokio::test]
async fn rejects_invalid_lengths_and_truncated_messages() {
    for bytes in [
        b"Content-Length: 2\r\nContent-Length: 2\r\n\r\n{}".as_slice(),
        b"Content-Length: 16777217\r\n\r\n",
        b"Content-Length: -1\r\n\r\n",
        b"Content-Type: application/json\r\n\r\n{}",
        b"Content-Length: 4\r\n\r\n{}",
    ] {
        assert!(read(&mut &bytes[..]).await.is_err());
    }
}

#[tokio::test]
async fn accepts_extra_headers_and_case_insensitive_length() {
    let mut input =
        b"content-length: 2\r\nContent-Type: application/vscode-jsonrpc; charset=utf-8\r\n\r\n{}"
            .as_slice();
    assert_eq!(read(&mut input).await.unwrap(), json!({}));
}

#[tokio::test]
async fn limits_unterminated_headers() {
    let bytes = vec![b'a'; MAX_HEADER_BYTES + 100];
    let mut input = bytes.as_slice();
    assert_eq!(
        read(&mut input).await.unwrap_err().kind(),
        io::ErrorKind::InvalidData
    );
    assert!(!input.is_empty());
}
