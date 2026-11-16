//! Open an installed assistant without interpreting project data as a command.

use std::{path::PathBuf, process::Stdio, time::Duration};
use tokio::process::Command;

pub(crate) async fn launch(
    application: Option<PathBuf>,
    directory: Option<PathBuf>,
    prompt: String,
) -> bool {
    let launcher =
        tokio::task::spawn_blocking(move || launch_command(application, directory, &prompt)).await;
    let Ok(Some((program, arguments))) = launcher else {
        return false;
    };
    let mut command = Command::new(program);
    command
        .args(arguments)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(target_os = "windows")]
    command.creation_flags(0x08000000);
    let Ok(mut process) = command.spawn() else {
        return false;
    };
    #[cfg(target_os = "macos")]
    let timeout = Duration::from_secs(10);
    #[cfg(not(target_os = "macos"))]
    let timeout = Duration::from_millis(300);
    match tokio::time::timeout(timeout, process.wait()).await {
        Ok(result) => result.is_ok_and(|status| status.success()),
        Err(_) => true,
    }
}

type Launcher = (PathBuf, Vec<std::ffi::OsString>);

#[cfg(target_os = "macos")]
fn launch_command(
    application: Option<PathBuf>,
    directory: Option<PathBuf>,
    prompt: &str,
) -> Option<Launcher> {
    let mut arguments = match application {
        Some(path) if path.is_absolute() && path.is_dir() => {
            vec!["-a".into(), path.into_os_string()]
        }
        Some(_) => return None,
        None => vec!["-b".into(), "com.openai.codex".into()],
    };
    let mut link = url::Url::parse("codex://new").ok()?;
    link.query_pairs_mut().append_pair("prompt", prompt);
    if let Some(directory) = directory.filter(|path| path.is_absolute() && path.is_dir()) {
        link.query_pairs_mut()
            .append_pair("path", directory.to_str()?);
    }
    arguments.push(link.as_str().into());
    Some((PathBuf::from("/usr/bin/open"), arguments))
}

#[cfg(not(target_os = "macos"))]
fn launch_command(
    application: Option<PathBuf>,
    _directory: Option<PathBuf>,
    _prompt: &str,
) -> Option<Launcher> {
    let path = match application {
        Some(path) if path.is_absolute() && path.is_file() => path,
        Some(_) => return None,
        None => find_application()?,
    };
    Some((path, Vec::new()))
}

#[cfg(not(target_os = "macos"))]
fn find_application() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    let name = "Codex.exe";
    #[cfg(not(target_os = "windows"))]
    let name = "codex-desktop";
    let path = std::env::var_os("PATH").unwrap_or_default();
    let directories = std::env::split_paths(&path);
    #[cfg(target_os = "windows")]
    let directories = directories.chain(
        ["LOCALAPPDATA", "ProgramFiles", "ProgramW6432"]
            .into_iter()
            .filter_map(std::env::var_os)
            .map(PathBuf::from)
            .flat_map(|root| {
                let name = name.trim_end_matches(".exe");
                [root.join(name), root.join("Programs").join(name)]
            }),
    );
    directories
        .filter(|directory| directory.is_absolute())
        .map(|directory| directory.join(name))
        .find(|path| path.is_file())
}

#[cfg(test)]
mod tests;
