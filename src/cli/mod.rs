//! Run command-line commands before the interface starts.
//!
//! The gate yields directories, files, deep links, and internal update flags
//! to the interface. Everything else is a command-line invocation.

mod args;
mod deployment;
mod output;

#[cfg(test)]
mod tests;

use crate::domain::external::Link;
use crate::domain::workspace::{Project, Workspace};
use crate::i18n::{FluentArgs, I18n, Key};
use crate::infrastructure;
use std::{
    ffi::OsStr,
    future::Future,
    path::{Path, PathBuf},
    process::ExitCode,
};

/// Interpret the command line. `None` continues into the interface.
pub(crate) fn run() -> Option<ExitCode> {
    let mut arguments = std::env::args_os();
    arguments.next();
    let first = arguments.next()?;
    if belongs_to_interface(&first) {
        return None;
    }
    Some(execute())
}

/// Decide whether the first argument goes to the interface or an external
/// request path instead of the command line.
fn belongs_to_interface(first: &OsStr) -> bool {
    let path = Path::new(first);
    if path.is_dir() || path.is_file() {
        return true;
    }
    let text = first.to_string_lossy();
    text.starts_with("--iwe-") || Link::parse(&text).is_some()
}

fn execute() -> ExitCode {
    let i18n = I18n::new(infrastructure::locale::detect());
    match args::parse(&i18n) {
        Ok(invocation) => dispatch(&i18n, invocation),
        Err(error) => args::report(&i18n, &error),
    }
}

fn dispatch(i18n: &I18n, invocation: args::Invocation) -> ExitCode {
    match invocation {
        args::Invocation::Open(directory) => open(i18n, directory),
        args::Invocation::List { json } => list(i18n, json),
        args::Invocation::AddProject {
            directory,
            space,
            json,
        } => add_project(i18n, &directory, space.as_deref(), json),
        args::Invocation::RemoveProject { target, json } => remove_project(i18n, &target, json),
        args::Invocation::Deployment(command) => deployment::dispatch(i18n, command),
    }
}

/// Start the interface, optionally with a project directory.
fn open(i18n: &I18n, directory: Option<PathBuf>) -> ExitCode {
    if let Some(directory) = &directory
        && !directory.is_dir()
    {
        let mut arguments = FluentArgs::new();
        arguments.set("path", directory.to_string_lossy().into_owned());
        return failure(
            i18n,
            Key::CliErrorDirectoryNotFound,
            arguments,
            ExitCode::from(2),
        );
    }
    launch(i18n, directory.as_deref())
}

/// Start a fresh interface process with the optional directory argument.
///
/// The command line owns the current arguments, so the interface starts in a
/// new process. It then reads the directory from its own first argument.
fn launch(i18n: &I18n, directory: Option<&Path>) -> ExitCode {
    let Ok(executable) = std::env::current_exe() else {
        return failure(
            i18n,
            Key::CliErrorLaunchFailed,
            FluentArgs::new(),
            ExitCode::FAILURE,
        );
    };
    let mut command = std::process::Command::new(executable);
    if let Some(directory) = directory {
        command.arg(directory);
    }
    match command.spawn() {
        Ok(_) => ExitCode::SUCCESS,
        Err(_) => failure(
            i18n,
            Key::CliErrorLaunchFailed,
            FluentArgs::new(),
            ExitCode::FAILURE,
        ),
    }
}

/// Print the workspace spaces and projects.
fn list(i18n: &I18n, json: bool) -> ExitCode {
    let mut workspace = match load(i18n) {
        Ok(workspace) => workspace,
        Err(code) => return code,
    };
    workspace.normalize();
    if json {
        return emit_json(i18n, &output::WorkspaceView::from_workspace(&workspace));
    }
    output::print_workspace(i18n, &workspace)
}

/// Add a project directory to a space and save the workspace.
fn add_project(i18n: &I18n, directory: &Path, space: Option<&str>, json: bool) -> ExitCode {
    let directory = match std::fs::canonicalize(directory) {
        Ok(directory) if directory.is_dir() => directory,
        _ => {
            let mut arguments = FluentArgs::new();
            arguments.set("path", directory.to_string_lossy().into_owned());
            return failure(
                i18n,
                Key::CliErrorDirectoryNotFound,
                arguments,
                ExitCode::FAILURE,
            );
        }
    };
    let mut workspace = match load(i18n) {
        Ok(workspace) => workspace,
        Err(code) => return code,
    };
    if workspace.spaces.iter().any(|space| {
        space
            .projects
            .iter()
            .any(|project| project.directory == directory)
    }) {
        let project = Project::from_directory(directory);
        return present(i18n, json, &project, Key::CliProjectAlreadyPresent);
    }
    let space_index = match space {
        Some(name) => match workspace.spaces.iter().position(|space| space.name == name) {
            Some(index) => index,
            None => {
                let mut arguments = FluentArgs::new();
                arguments.set("name", name.to_owned());
                return failure(
                    i18n,
                    Key::CliErrorSpaceNotFound,
                    arguments,
                    ExitCode::FAILURE,
                );
            }
        },
        None if workspace.spaces.is_empty() => {
            let name = Project::from_directory(directory.clone()).name;
            let name = if name.is_empty() {
                directory.to_string_lossy().into_owned()
            } else {
                name
            };
            match workspace.create_space(name, Vec::new()) {
                Some(index) => index,
                None => {
                    return failure(
                        i18n,
                        Key::CliErrorUnexpected,
                        FluentArgs::new(),
                        ExitCode::FAILURE,
                    );
                }
            }
        }
        None => 0,
    };
    workspace.add_project(space_index, directory.clone());
    if let Err(code) = save(i18n, workspace) {
        return code;
    }
    present(
        i18n,
        json,
        &Project::from_directory(directory),
        Key::CliProjectAdded,
    )
}

/// Remove a project by directory or name and save the workspace.
fn remove_project(i18n: &I18n, target: &str, json: bool) -> ExitCode {
    let mut workspace = match load(i18n) {
        Ok(workspace) => workspace,
        Err(code) => return code,
    };
    let directories = match std::fs::canonicalize(target) {
        Ok(directory) if directory.is_dir() => vec![directory],
        _ => {
            let matches: Vec<PathBuf> = workspace
                .spaces
                .iter()
                .flat_map(|space| space.projects.iter())
                .filter(|project| project.name == target || project.directory == Path::new(target))
                .map(|project| project.directory.clone())
                .collect();
            let mut unique = matches.clone();
            unique.sort();
            unique.dedup();
            if unique.len() > 1 {
                let mut arguments = FluentArgs::new();
                arguments.set("name", target.to_owned());
                return failure(
                    i18n,
                    Key::CliErrorAmbiguousName,
                    arguments,
                    ExitCode::FAILURE,
                );
            }
            unique
        }
    };
    let mut removed = Vec::new();
    for directory in directories {
        removed.extend(workspace.remove_projects(&directory));
    }
    if removed.is_empty() {
        let mut arguments = FluentArgs::new();
        arguments.set("target", target.to_owned());
        return failure(
            i18n,
            Key::CliErrorProjectNotFound,
            arguments,
            ExitCode::FAILURE,
        );
    }
    if let Err(code) = save(i18n, workspace) {
        return code;
    }
    if json {
        return emit_json(i18n, &output::RemovedView::from_projects(&removed));
    }
    if removed.len() == 1 {
        let mut arguments = FluentArgs::new();
        arguments.set("name", removed[0].name.clone());
        return message(i18n, Key::CliProjectRemoved, arguments);
    }
    let mut arguments = FluentArgs::new();
    arguments.set("count", removed.len() as i64);
    message(i18n, Key::CliProjectsRemoved, arguments)
}

/// Report an added or removed project as JSON or localized text.
fn present(i18n: &I18n, json: bool, project: &Project, key: Key) -> ExitCode {
    if json {
        return emit_json(i18n, &output::ProjectView::from_project(project));
    }
    let mut arguments = FluentArgs::new();
    arguments.set("name", project.name.clone());
    message(i18n, key, arguments)
}

/// Print one JSON value and succeed, or fail with a localized message.
pub(crate) fn emit_json(i18n: &I18n, value: &impl serde::Serialize) -> ExitCode {
    match output::print_json(value) {
        Ok(()) => ExitCode::SUCCESS,
        Err(()) => failure(
            i18n,
            Key::CliErrorUnexpected,
            FluentArgs::new(),
            ExitCode::FAILURE,
        ),
    }
}

/// Load the saved workspace.
pub(crate) fn load(i18n: &I18n) -> Result<Workspace, ExitCode> {
    match perform(infrastructure::workspace::load()) {
        Some(Ok((workspace, _))) => Ok(workspace),
        _ => Err(failure(
            i18n,
            Key::CliErrorLoadFailed,
            FluentArgs::new(),
            ExitCode::FAILURE,
        )),
    }
}

/// Save the workspace.
pub(crate) fn save(i18n: &I18n, workspace: Workspace) -> Result<(), ExitCode> {
    match perform(infrastructure::workspace::save(workspace)) {
        Some(Ok(())) => Ok(()),
        _ => Err(failure(
            i18n,
            Key::CliErrorSaveFailed,
            FluentArgs::new(),
            ExitCode::FAILURE,
        )),
    }
}

/// Run one asynchronous storage operation on a local runtime.
fn perform<T>(operation: impl Future<Output = T>) -> Option<T> {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .ok()
        .map(|runtime| runtime.block_on(operation))
}

/// Print one localized message to stderr and return the exit code.
fn failure(i18n: &I18n, key: Key, arguments: FluentArgs<'_>, code: ExitCode) -> ExitCode {
    eprintln!("{}", i18n.format(key, &arguments));
    code
}

/// Print one localized message to stdout and succeed.
pub(crate) fn message(i18n: &I18n, key: Key, arguments: FluentArgs<'_>) -> ExitCode {
    println!("{}", i18n.format(key, &arguments));
    ExitCode::SUCCESS
}
