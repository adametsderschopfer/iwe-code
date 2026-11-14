//! Print command results as human text or machine-readable JSON.

use crate::domain::workspace::{Project, Workspace};
use crate::i18n::{FluentArgs, I18n, Key};
use serde::Serialize;
use std::process::ExitCode;

/// The workspace as machine-readable JSON.
#[derive(Serialize)]
pub(crate) struct WorkspaceView {
    spaces: Vec<SpaceView>,
}

/// One space as machine-readable JSON.
#[derive(Serialize)]
pub(crate) struct SpaceView {
    name: String,
    projects: Vec<ProjectView>,
}

/// One project as machine-readable JSON.
#[derive(Serialize)]
pub(crate) struct ProjectView {
    name: String,
    directory: String,
    pinned: bool,
}

/// The projects removed by one command.
#[derive(Serialize)]
pub(crate) struct RemovedView {
    removed: Vec<ProjectView>,
}

impl RemovedView {
    pub(crate) fn from_projects(projects: &[Project]) -> Self {
        Self {
            removed: projects.iter().map(ProjectView::from_project).collect(),
        }
    }
}

impl WorkspaceView {
    pub(crate) fn from_workspace(workspace: &Workspace) -> Self {
        Self {
            spaces: workspace
                .spaces
                .iter()
                .map(|space| SpaceView {
                    name: space.name.clone(),
                    projects: space
                        .projects
                        .iter()
                        .map(ProjectView::from_project)
                        .collect(),
                })
                .collect(),
        }
    }
}

impl ProjectView {
    pub(crate) fn from_project(project: &Project) -> Self {
        Self {
            name: project.name.clone(),
            directory: project.directory.to_string_lossy().into_owned(),
            pinned: project.pinned,
        }
    }
}

/// Print one JSON value. `Err` means serialization failed.
pub(crate) fn print_json(value: &impl Serialize) -> Result<(), ()> {
    serde_json::to_string_pretty(value)
        .map(|text| println!("{text}"))
        .map_err(|_| ())
}

/// Print the workspace as localized text for people.
pub(crate) fn print_workspace(i18n: &I18n, workspace: &Workspace) -> ExitCode {
    if workspace.spaces.is_empty() {
        println!("{}", i18n.text(Key::NoProjects));
        return ExitCode::SUCCESS;
    }
    for space in &workspace.spaces {
        let mut arguments = FluentArgs::new();
        arguments.set("name", space.name.clone());
        println!("{}", i18n.format(Key::CliHumanSpace, &arguments));
        for project in &space.projects {
            let mut arguments = FluentArgs::new();
            arguments.set("name", project.name.clone());
            arguments.set(
                "directory",
                project.directory.to_string_lossy().into_owned(),
            );
            println!("  {}", i18n.format(Key::CliHumanProject, &arguments));
        }
    }
    ExitCode::SUCCESS
}
