//! Print command results as human text or machine-readable JSON.

use crate::domain::deployment::{
    Authentication, Connection, Connections, PrivateKey, Profile, Protocol,
};
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

/// Deployment settings and profiles as machine-readable JSON.
#[derive(Serialize)]
pub(crate) struct DeploymentView {
    connections: Vec<DeploymentConnectionView>,
    keys: Vec<DeploymentKeyView>,
    spaces: Vec<DeploymentSpaceView>,
}

/// One connection as machine-readable JSON. Secrets are never included.
#[derive(Serialize)]
pub(crate) struct DeploymentConnectionView {
    id: String,
    name: String,
    protocol: String,
    host: String,
    port: u16,
    username: String,
    authentication: String,
    key: Option<String>,
}

/// One private key as machine-readable JSON.
#[derive(Serialize)]
pub(crate) struct DeploymentKeyView {
    id: String,
    name: String,
    path: String,
}

/// One space with its deployment profiles as machine-readable JSON.
#[derive(Serialize)]
pub(crate) struct DeploymentSpaceView {
    name: String,
    profiles: Vec<DeploymentProfileView>,
}

/// One deployment profile as machine-readable JSON.
#[derive(Serialize)]
pub(crate) struct DeploymentProfileView {
    id: String,
    name: String,
    project: String,
    connection: String,
    connection_name: Option<String>,
    root: String,
    mappings: Vec<DeploymentMappingView>,
    excluded: Vec<String>,
    default: bool,
}

/// One path mapping as machine-readable JSON.
#[derive(Serialize)]
pub(crate) struct DeploymentMappingView {
    local: String,
    remote: String,
}

/// The name of the item removed by one command.
#[derive(Serialize)]
pub(crate) struct RemovedNameView {
    removed: String,
}

/// The outcome of one successful deployment connection check.
#[derive(Serialize)]
pub(crate) struct ConnectedView {
    profile: String,
    connected: bool,
}

/// The counters of one finished transfer.
#[derive(Serialize)]
pub(crate) struct TransferredView {
    profile: String,
    files: u64,
    bytes: u64,
}

/// One remote directory listing as machine-readable JSON.
#[derive(Serialize)]
pub(crate) struct RemoteEntriesView {
    directory: String,
    entries: Vec<RemoteEntryView>,
}

/// One remote entry as machine-readable JSON.
#[derive(Serialize)]
pub(crate) struct RemoteEntryView {
    name: String,
    path: String,
    kind: &'static str,
    size: u64,
}

impl DeploymentView {
    pub(crate) fn from_workspace(workspace: &Workspace) -> Self {
        Self {
            connections: workspace
                .deployment
                .connections
                .iter()
                .map(|connection| {
                    DeploymentConnectionView::from_connection(&workspace.deployment, connection)
                })
                .collect(),
            keys: workspace
                .deployment
                .keys
                .iter()
                .map(DeploymentKeyView::from_key)
                .collect(),
            spaces: workspace
                .spaces
                .iter()
                .map(|space| DeploymentSpaceView {
                    name: space.name.clone(),
                    profiles: space
                        .deployments
                        .iter()
                        .map(|profile| {
                            DeploymentProfileView::from_settings(&workspace.deployment, profile)
                        })
                        .collect(),
                })
                .collect(),
        }
    }
}

impl DeploymentConnectionView {
    pub(crate) fn from_connection(settings: &Connections, connection: &Connection) -> Self {
        Self {
            id: connection.id.clone(),
            name: connection.name.clone(),
            protocol: protocol_name(connection.protocol).to_owned(),
            host: connection.host.clone(),
            port: connection.port,
            username: connection.username.clone(),
            authentication: authentication_name(&connection.authentication).to_owned(),
            key: key_name(settings, &connection.authentication),
        }
    }
}

impl DeploymentKeyView {
    pub(crate) fn from_key(key: &PrivateKey) -> Self {
        Self {
            id: key.id.clone(),
            name: key.name.clone(),
            path: key.path.to_string_lossy().into_owned(),
        }
    }
}

impl DeploymentProfileView {
    pub(crate) fn from_settings(settings: &Connections, profile: &Profile) -> Self {
        Self {
            id: profile.id.clone(),
            name: profile.name.clone(),
            project: profile.project.to_string_lossy().into_owned(),
            connection: profile.connection.clone(),
            connection_name: settings
                .connections
                .iter()
                .find(|connection| connection.id == profile.connection)
                .map(|connection| connection.name.clone()),
            root: profile.root.clone(),
            mappings: profile
                .mappings
                .iter()
                .map(|mapping| DeploymentMappingView {
                    local: mapping.local.to_string_lossy().into_owned(),
                    remote: mapping.remote.clone(),
                })
                .collect(),
            excluded: profile
                .excluded
                .iter()
                .map(|path| path.to_string_lossy().into_owned())
                .collect(),
            default: profile.default,
        }
    }
}

impl RemovedNameView {
    pub(crate) fn from_name(name: &str) -> Self {
        Self {
            removed: name.to_owned(),
        }
    }
}

impl ConnectedView {
    pub(crate) fn from_profile(profile: &Profile) -> Self {
        Self {
            profile: profile.name.clone(),
            connected: true,
        }
    }
}

impl TransferredView {
    pub(crate) fn from_counters(profile: &Profile, files: u64, bytes: u64) -> Self {
        Self {
            profile: profile.name.clone(),
            files,
            bytes,
        }
    }
}

impl RemoteEntriesView {
    pub(crate) fn from_entries(
        directory: &str,
        entries: &[crate::infrastructure::deployment::transport::Entry],
    ) -> Self {
        Self {
            directory: directory.to_owned(),
            entries: entries
                .iter()
                .map(|entry| RemoteEntryView {
                    name: entry.name.clone(),
                    path: entry.path.clone(),
                    kind: if entry.symlink {
                        "symlink"
                    } else if entry.directory {
                        "directory"
                    } else {
                        "file"
                    },
                    size: entry.size,
                })
                .collect(),
        }
    }
}

fn protocol_name(protocol: Protocol) -> &'static str {
    match protocol {
        Protocol::Sftp => "sftp",
        Protocol::Ftp => "ftp",
        Protocol::Ftps => "ftps",
        Protocol::FtpsImplicit => "ftps-implicit",
    }
}

fn authentication_name(authentication: &Authentication) -> &'static str {
    match authentication {
        Authentication::Password => "password",
        Authentication::Agent => "agent",
        Authentication::Key(_) => "key",
    }
}

fn key_name(settings: &Connections, authentication: &Authentication) -> Option<String> {
    match authentication {
        Authentication::Key(id) => settings
            .keys
            .iter()
            .find(|key| &key.id == id)
            .map(|key| key.name.clone()),
        _ => None,
    }
}

/// Print deployments as localized text for people.
pub(crate) fn print_deployments(i18n: &I18n, workspace: &Workspace) -> ExitCode {
    let settings = &workspace.deployment;
    if settings.connections.is_empty()
        && settings.keys.is_empty()
        && workspace
            .spaces
            .iter()
            .all(|space| space.deployments.is_empty())
    {
        println!("{}", i18n.text(Key::CliDeploymentListEmpty));
        return ExitCode::SUCCESS;
    }
    if !settings.connections.is_empty() {
        println!("{}", i18n.text(Key::DeploymentConnections));
        for connection in &settings.connections {
            println!(
                "  {}: {}://{}@{}:{} ({})",
                connection.name,
                protocol_name(connection.protocol),
                connection.username,
                connection.host,
                connection.port,
                authentication_name(&connection.authentication),
            );
        }
    }
    if !settings.keys.is_empty() {
        println!("{}", i18n.text(Key::DeploymentKeys));
        for key in &settings.keys {
            println!("  {}: {}", key.name, key.path.display());
        }
    }
    for space in &workspace.spaces {
        if space.deployments.is_empty() {
            continue;
        }
        let mut arguments = FluentArgs::new();
        arguments.set("name", space.name.clone());
        println!("{}", i18n.format(Key::CliHumanSpace, &arguments));
        for profile in &space.deployments {
            let mut arguments = FluentArgs::new();
            arguments.set("name", profile.name.clone());
            arguments.set("directory", profile.project.to_string_lossy().into_owned());
            println!("  {}", i18n.format(Key::CliHumanProject, &arguments));
            let mut arguments = FluentArgs::new();
            arguments.set("root", profile.root.clone());
            println!(
                "    {}",
                i18n.format(Key::CliDeploymentHumanProfile, &arguments)
            );
            for mapping in &profile.mappings {
                let mut arguments = FluentArgs::new();
                arguments.set("local", mapping.local.display().to_string());
                arguments.set("remote", mapping.remote.clone());
                arguments.set("root", profile.root.clone());
                println!(
                    "      {}",
                    i18n.format(Key::CliDeploymentHumanMapping, &arguments)
                );
            }
            if profile.default {
                println!("    {}", i18n.text(Key::DeploymentDefault));
            }
        }
    }
    ExitCode::SUCCESS
}

/// Print one remote directory listing as data lines for people.
pub(crate) fn print_remote_entries(
    directory: &str,
    entries: &[crate::infrastructure::deployment::transport::Entry],
) {
    println!("{directory}:");
    for entry in entries {
        let marker = if entry.directory { "/" } else { "" };
        println!("  {}{}", entry.name, marker);
    }
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
