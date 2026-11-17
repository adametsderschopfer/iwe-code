//! Read and write bounded LSP messages without blocking the interface thread.

use serde_json::Value;
use std::io;
use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncReadExt, AsyncWrite, AsyncWriteExt};

const MAX_HEADER_BYTES: usize = 8 * 1024;
const MAX_MESSAGE_BYTES: usize = 16 * 1024 * 1024;

pub(super) async fn read(reader: &mut (impl AsyncBufRead + Unpin)) -> io::Result<Value> {
    let mut length = None;
    let mut header_bytes = 0;
    loop {
        let mut line = Vec::new();
        let remaining = MAX_HEADER_BYTES - header_bytes;
        let count = reader
            .take((remaining + 1) as u64)
            .read_until(b'\n', &mut line)
            .await?;
        header_bytes += count;
        if count == 0 {
            return Err(io::Error::from(io::ErrorKind::UnexpectedEof));
        }
        if header_bytes > MAX_HEADER_BYTES || !line.ends_with(b"\r\n") {
            return Err(invalid_message());
        }
        if line == b"\r\n" {
            break;
        }
        let line = std::str::from_utf8(&line).map_err(|_| invalid_message())?;
        let (name, value) = line.split_once(':').ok_or_else(invalid_message)?;
        if name.eq_ignore_ascii_case("Content-Length") {
            if length.is_some() {
                return Err(invalid_message());
            }
            let size = value
                .trim()
                .parse::<usize>()
                .map_err(|_| invalid_message())?;
            if size == 0 || size > MAX_MESSAGE_BYTES {
                return Err(invalid_message());
            }
            length = Some(size);
        }
    }
    let mut body = vec![0; length.ok_or_else(invalid_message)?];
    reader.read_exact(&mut body).await?;
    serde_json::from_slice(&body).map_err(|_| invalid_message())
}

pub(super) async fn write(
    writer: &mut (impl AsyncWrite + Unpin),
    message: &Value,
) -> io::Result<()> {
    let body = serde_json::to_vec(message).map_err(|_| invalid_message())?;
    if body.len() > MAX_MESSAGE_BYTES {
        return Err(invalid_message());
    }
    let header = format!("Content-Length: {}\r\n\r\n", body.len());
    writer.write_all(header.as_bytes()).await?;
    writer.write_all(&body).await?;
    writer.flush().await
}

fn invalid_message() -> io::Error {
    io::Error::from(io::ErrorKind::InvalidData)
}

#[cfg(test)]
mod tests;
