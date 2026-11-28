//! Install validated theme files in the local configuration directory.

use crate::domain::theme::{self, Definition};
use std::{
    fs::{self, File},
    io::{ErrorKind, Read, Write},
    path::{Path, PathBuf},
};

const MAX_BYTES: u64 = 256 * 1024;
const MAX_THEMES: usize = 24;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Error {
    Read,
    Invalid,
    TooMany,
    Save,
}

fn directory() -> Result<PathBuf, Error> {
    super::workspace::config_path()
        .map_err(|_| Error::Read)?
        .parent()
        .map(|parent| parent.join("themes"))
        .ok_or(Error::Read)
}

pub(crate) async fn load() -> Vec<Definition> {
    tokio::task::spawn_blocking(|| directory().map_or_else(|_| Vec::new(), |path| load_from(&path)))
        .await
        .unwrap_or_default()
}

fn load_from(directory: &Path) -> Vec<Definition> {
    if !directory.symlink_metadata().is_ok_and(|data| data.is_dir()) {
        return Vec::new();
    }
    let Ok(entries) = directory.read_dir() else {
        return Vec::new();
    };
    entries
        .take(MAX_THEMES * 4)
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let path = entry.path();
            if path.extension()?.to_str()? != "json" {
                return None;
            }
            let key = path.file_stem()?.to_str()?;
            if !theme::valid_id(key) || theme::is_bundled(key) {
                return None;
            }
            let (definition, _) = read(&path).ok()?;
            (definition.id == key).then_some(definition)
        })
        .take(MAX_THEMES)
        .collect()
}

pub(crate) async fn pick_and_install(
    title: String,
    filter: String,
) -> Result<Option<Definition>, Error> {
    let Some(file) = rfd::AsyncFileDialog::new()
        .set_title(title)
        .add_filter(filter, &["json"])
        .pick_file()
        .await
    else {
        return Ok(None);
    };
    let source = file.path().to_owned();
    tokio::task::spawn_blocking(move || install(&source, &directory()?).map(Some))
        .await
        .map_err(|_| Error::Save)?
}

fn read(path: &Path) -> Result<(Definition, Vec<u8>), Error> {
    if !path.symlink_metadata().is_ok_and(|data| data.is_file()) {
        return Err(Error::Read);
    }
    let file = File::open(path).map_err(|_| Error::Read)?;
    let mut bytes = Vec::new();
    file.take(MAX_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| Error::Read)?;
    if bytes.len() as u64 > MAX_BYTES {
        return Err(Error::Invalid);
    }
    let definition: Definition = serde_json::from_slice(&bytes).map_err(|_| Error::Invalid)?;
    if !definition.is_valid() || theme::is_bundled(&definition.id) {
        return Err(Error::Invalid);
    }
    Ok((definition, bytes))
}

fn install(source: &Path, directory: &Path) -> Result<Definition, Error> {
    let (definition, bytes) = read(source)?;
    fs::create_dir_all(directory).map_err(|_| Error::Save)?;
    if !directory.symlink_metadata().is_ok_and(|data| data.is_dir()) {
        return Err(Error::Save);
    }
    let destination = directory.join(format!("{}.json", definition.id));
    match destination.symlink_metadata() {
        Ok(data) if !data.is_file() => return Err(Error::Save),
        Err(error) if error.kind() != ErrorKind::NotFound => return Err(Error::Save),
        Err(_) if load_from(directory).len() >= MAX_THEMES => return Err(Error::TooMany),
        _ => {}
    }
    let mut temporary = tempfile::NamedTempFile::new_in(directory).map_err(|_| Error::Save)?;
    temporary.write_all(&bytes).map_err(|_| Error::Save)?;
    temporary.as_file().sync_all().map_err(|_| Error::Save)?;
    temporary.persist(destination).map_err(|_| Error::Save)?;
    Ok(definition)
}

#[cfg(test)]
mod tests;
