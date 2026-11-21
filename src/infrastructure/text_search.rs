//! Scan local source files on a blocking worker with cancellation and bounded output.
use crate::domain::text_search::{self, Match, Options};
use std::{
    collections::BTreeMap,
    io::{BufRead, BufReader},
    path::PathBuf,
    time::{Duration, Instant},
};
use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone)]
pub(crate) struct Hit {
    pub path: PathBuf,
    pub found: Match,
}
#[derive(Debug, Clone, Default)]
pub(crate) struct Results {
    pub hits: Vec<Hit>,
    pub limited: bool,
    pub skipped: usize,
}
#[derive(Clone)]
pub(crate) struct Request {
    pub root: PathBuf,
    pub folder: PathBuf,
    pub mask: String,
    pub query: String,
    pub options: Options,
    pub buffers: BTreeMap<PathBuf, std::sync::Arc<String>>,
    pub file: Option<PathBuf>,
    pub cancelled: CancellationToken,
}
pub(crate) async fn search(request: Request) -> Results {
    tokio::task::spawn_blocking(move || scan(request))
        .await
        .unwrap_or_default()
}
fn scan(request: Request) -> Results {
    let mut results = Results::default();
    if request.file.is_none() {
        let Ok(relative) = request.folder.strip_prefix(&request.root) else {
            results.skipped = 1;
            return results;
        };
        let mut path = request.root.clone();
        for component in relative.components() {
            if !matches!(
                component,
                std::path::Component::Normal(_) | std::path::Component::CurDir
            ) {
                results.skipped = 1;
                return results;
            }
            path.push(component);
            if std::fs::symlink_metadata(&path).map_or(true, |meta| meta.file_type().is_symlink()) {
                results.skipped = 1;
                return results;
            }
        }
    }
    let Ok(pattern) = text_search::compile(&request.query, &request.options) else {
        return results;
    };
    if request.query.is_empty() {
        return results;
    }
    let deadline = Instant::now() + Duration::from_secs(45);
    let mut queue = vec![
        request
            .file
            .clone()
            .unwrap_or_else(|| request.folder.clone()),
    ];
    let mask = regex::Regex::new(&format!(
        "^(?:{})$",
        request
            .mask
            .split(',')
            .map(|part| regex::escape(part.trim())
                .replace(r"\*", ".*")
                .replace(r"\?", "."))
            .collect::<Vec<_>>()
            .join("|")
    ))
    .ok();
    let mut visited = 0;
    while let Some(path) = queue.pop() {
        if request.cancelled.is_cancelled() {
            return Results::default();
        }
        if Instant::now() >= deadline || visited >= 100_000 || results.hits.len() >= 10_000 {
            results.limited = true;
            break;
        }
        visited += 1;
        let Ok(meta) = std::fs::symlink_metadata(&path) else {
            results.skipped += 1;
            continue;
        };
        if meta.file_type().is_symlink() {
            continue;
        }
        if meta.is_dir() {
            if !super::project_index::prefetch(&request.root, &path) {
                continue;
            }
            match std::fs::read_dir(&path) {
                Ok(entries) => {
                    let remaining = 100_000 - visited - queue.len();
                    let mut paths = Vec::new();
                    for entry in entries.flatten() {
                        if request.cancelled.is_cancelled() {
                            return Results::default();
                        }
                        if paths.len() == remaining || Instant::now() >= deadline {
                            results.limited = true;
                            break;
                        }
                        if entry.file_name() != ".DS_Store" {
                            paths.push(entry.path());
                        }
                    }
                    paths.sort();
                    queue.extend(paths.into_iter().rev());
                }
                Err(_) => results.skipped += 1,
            }
            continue;
        }
        if !meta.is_file() {
            continue;
        }
        if !request.mask.is_empty()
            && !mask.as_ref().is_some_and(|mask| {
                mask.is_match(&path.file_name().unwrap_or_default().to_string_lossy())
            })
        {
            continue;
        }
        if let Some(text) = request.buffers.get(&path) {
            results.hits.extend(
                text_search::matches(text, &pattern, 10_001 - results.hits.len())
                    .into_iter()
                    .map(|found| Hit {
                        path: path.clone(),
                        found,
                    }),
            );
            continue;
        }
        let Ok(file) = std::fs::File::open(&path) else {
            results.skipped += 1;
            continue;
        };
        let mut reader = BufReader::new(file);
        let mut bytes = Vec::new();
        let mut offset = 0;
        let mut line = 1;
        loop {
            if request.cancelled.is_cancelled() {
                return Results::default();
            }
            if Instant::now() >= deadline || results.hits.len() >= 10_000 {
                results.limited = true;
                break;
            }
            bytes.clear();
            // Limit individual lines to avoid unbounded allocations from binary or generated files.
            let mut bounded = std::io::Read::take(&mut reader, 2 * 1024 * 1024 + 1);
            let Ok(read) = bounded.read_until(b'\n', &mut bytes) else {
                results.skipped += 1;
                break;
            };
            if read == 0 {
                break;
            }
            if bytes.len() > 2 * 1024 * 1024 || bytes.contains(&0) {
                results.skipped += 1;
                break;
            }
            let Ok(text) = std::str::from_utf8(&bytes) else {
                results.skipped += 1;
                break;
            };
            results.hits.extend(
                text_search::matches(
                    text.trim_end_matches(['\r', '\n']),
                    &pattern,
                    10_001 - results.hits.len(),
                )
                .into_iter()
                .map(|mut found| {
                    found.line = line;
                    found.range = found.range.start + offset..found.range.end + offset;
                    Hit {
                        path: path.clone(),
                        found,
                    }
                }),
            );
            offset += read;
            line += 1;
        }
    }
    if results.hits.len() > 10_000 {
        results.hits.truncate(10_000);
        results.limited = true;
    }
    results
}

#[cfg(test)]
mod tests;

/// Read a small context window without retaining the whole file.
pub(crate) async fn preview(
    path: PathBuf,
    line: usize,
    cancelled: CancellationToken,
) -> Option<String> {
    tokio::task::spawn_blocking(move || {
        let file = std::fs::File::open(path).ok()?;
        let mut reader = BufReader::new(file);
        let mut text = String::new();
        let mut bytes = Vec::new();
        for index in 0..line.saturating_add(13) {
            if cancelled.is_cancelled() {
                return None;
            }
            bytes.clear();
            let mut bounded = std::io::Read::take(&mut reader, 2 * 1024 * 1024 + 1);
            let read = bounded.read_until(b'\n', &mut bytes).ok()?;
            if read == 0 {
                break;
            }
            if read > 2 * 1024 * 1024 || bytes.contains(&0) {
                return None;
            }
            let row = std::str::from_utf8(&bytes).ok()?;
            if index + 12 >= line {
                text.push_str(&format!(
                    "{}  {}\n",
                    index + 1,
                    row.trim_end_matches(['\r', '\n'])
                        .chars()
                        .take(2000)
                        .collect::<String>()
                ));
            }
        }
        Some(text)
    })
    .await
    .ok()
    .flatten()
}
