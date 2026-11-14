//! Shared helpers for line-oriented local transports.

use tokio::io::{AsyncBufRead, AsyncBufReadExt};

/// One line as read from a bounded transport.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Line {
    Text(String),
    /// The line exceeded the size cap. Its remainder was discarded.
    TooLong,
}

/// Read one newline-terminated line, buffering at most `limit` bytes.
///
/// Returns `None` only when the stream ends before any bytes arrive. A final
/// line without a newline is returned like a complete line. The remainder of
/// an over-long line is drained so the framing of later lines stays intact.
pub(crate) async fn read_line<R: AsyncBufRead + Unpin>(
    input: &mut R,
    limit: usize,
) -> std::io::Result<Option<Line>> {
    let mut bytes = Vec::new();
    let mut consumed_any = false;
    let mut too_long = false;
    loop {
        let available = input.fill_buf().await?;
        if available.is_empty() {
            break;
        }
        let complete = available
            .iter()
            .position(|byte| *byte == b'\n')
            .unwrap_or(available.len());
        let newline_found = complete < available.len();
        if !too_long {
            if bytes.len() + complete > limit {
                too_long = true;
                bytes = Vec::new();
            } else {
                bytes.extend_from_slice(&available[..complete]);
            }
        }
        let consumed = complete + usize::from(newline_found);
        input.consume(consumed);
        consumed_any |= consumed > 0;
        if newline_found {
            break;
        }
    }
    let line = if too_long {
        Line::TooLong
    } else {
        Line::Text(String::from_utf8_lossy(&bytes).into_owned())
    };
    Ok(consumed_any.then_some(line))
}

#[cfg(test)]
mod tests;
