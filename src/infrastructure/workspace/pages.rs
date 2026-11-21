//! Read bounded UTF-8 portions without retaining the complete file.

use crate::domain::{
    Failure,
    changes::{TextCursor, TextPage},
};
use std::{
    fs::{File, Metadata},
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
};

pub(crate) const PAGE_BYTES: usize = super::PREVIEW_LIMIT as usize;
pub(crate) const READ_BYTES: usize = PAGE_BYTES + 4;
const PAGE_LINES: usize = 4096;

pub(crate) fn decode(
    bytes: Vec<u8>,
    cursor: TextCursor,
    total: u64,
    version: Option<(u64, u128)>,
) -> Result<(String, TextPage), Failure> {
    let limit = bytes.len().min(PAGE_BYTES);
    let mut end = limit;
    let mut lines = 0;
    let mut newline = None;
    for (index, byte) in bytes[..limit].iter().enumerate() {
        if *byte == b'\n' {
            newline = Some(index + 1);
            lines += 1;
            if lines == PAGE_LINES {
                break;
            }
        }
    }
    if let Some(boundary) = newline {
        end = boundary;
    } else if let Err(error) = std::str::from_utf8(&bytes[..end]) {
        if error.error_len().is_some() {
            return Err(Failure::Binary);
        }
        end = error.valid_up_to();
    }
    // Keep CRLF together, including exceptionally long lines split at the byte limit.
    if end > 0 && bytes[end - 1] == b'\r' && bytes.get(end) == Some(&b'\n') {
        end -= 1;
    }
    if end == 0 && cursor.offset < total {
        return Err(Failure::Read);
    }
    let selected = &bytes[..end];
    if selected.contains(&0) {
        return Err(Failure::Binary);
    }
    let text = std::str::from_utf8(selected)
        .map_err(|_| Failure::Binary)?
        .to_owned();
    let next_offset = cursor.offset + end as u64;
    let next = (next_offset < total).then(|| TextCursor {
        offset: next_offset,
        line: cursor.line + selected.iter().filter(|byte| **byte == b'\n').count(),
    });
    Ok((
        text,
        TextPage {
            cursor,
            next,
            version,
        },
    ))
}

fn version(metadata: &Metadata) -> Result<(u64, u128), Failure> {
    let modified = metadata.modified().map_err(|_| Failure::Read)?;
    let stamp = modified
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| Failure::Read)?;
    Ok((metadata.len(), stamp.as_nanos()))
}

fn read_bytes(file: &File, size: u64) -> Result<Vec<u8>, Failure> {
    let mut bytes = Vec::with_capacity(size.min(READ_BYTES as u64) as usize);
    file.take(READ_BYTES as u64)
        .read_to_end(&mut bytes)
        .map_err(|_| Failure::Read)?;
    Ok(bytes)
}

fn finish_page(
    file: &File,
    snapshot: (u64, u128),
    bytes: Vec<u8>,
    cursor: TextCursor,
) -> Result<(String, TextPage), Failure> {
    if version(&file.metadata().map_err(|_| Failure::Read)?)? != snapshot {
        return Err(Failure::Read);
    }
    decode(bytes, cursor, snapshot.0, Some(snapshot))
}

pub(crate) fn first(root: &Path, path: &Path) -> Result<(String, Option<TextPage>), Failure> {
    let (file, metadata) = super::open_text_file(root, path)?;
    let bytes = read_bytes(&file, metadata.len())?;
    let bytes = if metadata.len() <= super::PREVIEW_LIMIT && bytes.len() <= PAGE_BYTES {
        let text = super::utf8_text(bytes)?;
        if !super::exceeds_line_limit(&text) {
            return Ok((text, None));
        }
        text.into_bytes()
    } else {
        bytes
    };
    let (text, page) = finish_page(&file, version(&metadata)?, bytes, TextCursor::default())?;
    Ok((text, Some(page)))
}

pub(crate) async fn read(
    root: PathBuf,
    path: PathBuf,
    cursor: TextCursor,
    expected: Option<(u64, u128)>,
) -> Result<(String, TextPage), Failure> {
    tokio::task::spawn_blocking(move || {
        let (mut file, metadata) = super::open_text_file(&root, &path)?;
        let snapshot = version(&metadata)?;
        let cursor = if expected.is_some_and(|expected| expected != snapshot) {
            TextCursor::default()
        } else {
            cursor
        };
        if cursor.offset > metadata.len() {
            return Err(Failure::Read);
        }
        file.seek(SeekFrom::Start(cursor.offset))
            .map_err(|_| Failure::Read)?;
        let bytes = read_bytes(&file, metadata.len() - cursor.offset)?;
        finish_page(&file, snapshot, bytes, cursor)
    })
    .await
    .map_err(|_| Failure::Read)?
}
