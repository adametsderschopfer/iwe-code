//! Apply explicit edits to local project files without replacing existing entries.

use std::{
    fs,
    io::{self, ErrorKind},
    path::{Component, Path, PathBuf},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Operation {
    CreateFile,
    CreateFolder,
    Copy,
    Move,
    Trash,
    Reveal,
}

#[derive(Debug, Clone)]
pub(crate) struct Request {
    pub operation: Operation,
    pub source: Option<PathBuf>,
    pub destination: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Error {
    InvalidName,
    Protected,
    Missing,
    Conflict,
    CrossVolume,
    Unsupported,
    Access,
    Trash,
}

impl From<io::Error> for Error {
    fn from(error: io::Error) -> Self {
        match error.kind() {
            ErrorKind::NotFound => Self::Missing,
            ErrorKind::AlreadyExists => Self::Conflict,
            ErrorKind::CrossesDevices => Self::CrossVolume,
            ErrorKind::Unsupported => Self::Unsupported,
            _ => Self::Access,
        }
    }
}

pub(crate) fn validate_name(name: &str) -> Result<(), Error> {
    if name.trim().is_empty()
        || name.ends_with(['.', ' '])
        || name
            .chars()
            .any(|character| character.is_control() || "<>:\"/\\|?*".contains(character))
    {
        return Err(Error::InvalidName);
    }
    let base = name
        .split('.')
        .next()
        .unwrap_or_default()
        .trim_end()
        .to_ascii_uppercase();
    if matches!(
        base.as_str(),
        "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$"
    ) || ["COM", "LPT"].iter().any(|prefix| {
        base.strip_prefix(prefix).is_some_and(|suffix| {
            matches!(
                suffix,
                "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³"
            )
        })
    }) {
        return Err(Error::InvalidName);
    }
    if name.eq_ignore_ascii_case(".git") {
        return Err(Error::Protected);
    }
    Ok(())
}

pub(crate) fn duplicate_name(root: &Path, source: &Path) -> Result<String, Error> {
    let metadata = checked_entry(root, source)?;
    let name = source
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or(Error::InvalidName)?;
    let (stem, extension) = if metadata.is_file() {
        name.rsplit_once('.')
            .filter(|(stem, _)| !stem.is_empty())
            .unwrap_or((name, ""))
    } else {
        (name, "")
    };
    let suffix = if extension.is_empty() {
        String::new()
    } else {
        format!(".{extension}")
    };
    for number in 1..=10_000 {
        let name = format!("{stem} ({number}){suffix}");
        match fs::symlink_metadata(source.with_file_name(&name)) {
            Err(error) if error.kind() == ErrorKind::NotFound => return Ok(name),
            Err(error) => return Err(error.into()),
            Ok(_) => {}
        }
    }
    Err(Error::Conflict)
}

pub(crate) fn run(root: &Path, repositories: &[PathBuf], request: &Request) -> Result<(), Error> {
    use Operation::*;
    let source = request.source.as_deref();
    let destination = request.destination.as_deref();
    if request.operation == Reveal {
        let source = source.ok_or(Error::Protected)?;
        let metadata = checked_entry(root, source)?;
        return reveal(source, metadata.is_dir());
    }

    if let Some(source) = source {
        if source == root
            || repositories
                .iter()
                .any(|repository| repository.starts_with(source))
        {
            return Err(Error::Protected);
        }
        scan_source(root, source)?;
    }
    if let Some(destination) = destination {
        let parent = destination.parent().ok_or(Error::Protected)?;
        if !checked_entry(root, parent)?.is_dir() {
            return Err(Error::Protected);
        }
        let name = destination
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or(Error::InvalidName)?;
        validate_name(name)?;
        if source.is_some_and(|source| destination.starts_with(source)) {
            return Err(if source == Some(destination) {
                Error::Conflict
            } else {
                Error::Protected
            });
        }
        match fs::symlink_metadata(destination) {
            Ok(_) => return Err(Error::Conflict),
            Err(error) if error.kind() == ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }
    match (request.operation, source, destination) {
        (CreateFile, None, Some(destination)) => {
            fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(destination)?;
        }
        (CreateFolder, None, Some(destination)) => fs::create_dir(destination)?,
        (Copy, Some(source), Some(destination)) => copy(root, source, destination)?,
        (Move, Some(source), Some(destination)) => {
            renamore::rename_exclusive(source, destination)?;
        }
        (Trash, Some(source), None) => move_to_trash(source)?,
        _ => return Err(Error::Protected),
    }
    Ok(())
}

fn checked_entry(root: &Path, path: &Path) -> Result<fs::Metadata, Error> {
    let relative = path.strip_prefix(root).map_err(|_| Error::Protected)?;
    let mut current = root.to_path_buf();
    let mut metadata = fs::symlink_metadata(root)?;
    if !metadata.is_dir() || is_link(&metadata) {
        return Err(Error::Protected);
    }
    for component in relative.components() {
        let Component::Normal(name) = component else {
            return Err(Error::Protected);
        };
        if name.eq_ignore_ascii_case(".git") {
            return Err(Error::Protected);
        }
        current.push(name);
        metadata = fs::symlink_metadata(&current)?;
        if is_link(&metadata) || !(metadata.is_dir() || metadata.is_file()) {
            return Err(Error::Protected);
        }
    }
    Ok(metadata)
}

fn is_link(metadata: &fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
        metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
    }
    #[cfg(not(windows))]
    metadata.is_symlink()
}

fn scan_source(root: &Path, source: &Path) -> Result<(), Error> {
    let mut pending = vec![source.to_path_buf()];
    while let Some(path) = pending.pop() {
        if checked_entry(root, &path)?.is_dir() {
            for entry in fs::read_dir(path)? {
                pending.push(entry?.path());
            }
        }
    }
    Ok(())
}

fn copy(root: &Path, source: &Path, destination: &Path) -> Result<(), Error> {
    let parent = destination.parent().ok_or(Error::Protected)?;
    let staging = tempfile::Builder::new()
        .prefix(".iwe-copy-")
        .tempdir_in(parent)?;
    let staged = staging.path().join("entry");
    let result = copy_entry(root, source, &staged, 0).and_then(|()| {
        checked_entry(root, parent)?;
        renamore::rename_exclusive(&staged, destination)?;
        Ok(())
    });
    #[cfg(unix)]
    if result.is_err() {
        make_staged_directories_removable(&staged);
    }
    result
}

#[cfg(unix)]
fn make_staged_directories_removable(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    let Ok(metadata) = fs::symlink_metadata(path) else {
        return;
    };
    if !metadata.is_dir() {
        return;
    }
    // Staged directories can inherit read-only permissions before publication fails.
    let mode = metadata.permissions().mode();
    if mode & 0o700 != 0o700 {
        let _ = fs::set_permissions(path, fs::Permissions::from_mode(mode | 0o700));
    }
    if let Ok(entries) = fs::read_dir(path) {
        for entry in entries.flatten() {
            make_staged_directories_removable(&entry.path());
        }
    }
}

fn copy_entry(root: &Path, source: &Path, destination: &Path, depth: usize) -> Result<(), Error> {
    if depth > 256 {
        return Err(Error::Unsupported);
    }
    let metadata = checked_entry(root, source)?;
    if metadata.is_file() {
        fs::copy(source, destination)?;
        return Ok(());
    }
    fs::create_dir(destination)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        copy_entry(
            root,
            &entry.path(),
            &destination.join(entry.file_name()),
            depth + 1,
        )?;
    }
    fs::set_permissions(destination, metadata.permissions())?;
    Ok(())
}

fn move_to_trash(path: &Path) -> Result<(), Error> {
    #[cfg(target_os = "macos")]
    let result = {
        use trash::macos::{DeleteMethod, TrashContextExtMacos};
        let mut context = trash::TrashContext::new();
        context.set_delete_method(DeleteMethod::NsFileManager);
        context.delete(path)
    };
    #[cfg(not(target_os = "macos"))]
    let result = trash::delete(path);
    result.map_err(|_| Error::Trash)
}

fn reveal(path: &Path, directory: bool) -> Result<(), Error> {
    if directory {
        return open::that(path).map_err(|_| Error::Access);
    }
    #[cfg(target_os = "macos")]
    {
        if std::process::Command::new("/usr/bin/open")
            .arg("-R")
            .arg(path)
            .status()
            .is_ok_and(|status| status.success())
        {
            return Ok(());
        }
    }
    #[cfg(target_os = "windows")]
    {
        let mut argument = std::ffi::OsString::from("/select,");
        argument.push(path);
        if std::process::Command::new("explorer.exe")
            .arg(argument)
            .spawn()
            .is_ok()
        {
            return Ok(());
        }
    }
    open::that(path.parent().ok_or(Error::Protected)?).map_err(|_| Error::Access)
}

#[cfg(test)]
mod tests;
