//! Implement the MCP tools over workspace metadata, project files, and Git.

use crate::domain::{
    Failure,
    changes::FileStatus,
    deployment::{Authentication, Connections, Error as DeploymentError, Profile, Protocol},
    editor::LIMIT,
    search::{self, FileCandidate},
    workspace::{Project, Workspace},
};
use crate::infrastructure::deployment::transfer::{self, Operation, Output, Request};
use crate::infrastructure::deployment::transport::Progress;
use crate::infrastructure::editor::{Error as SaveError, save as save_file};
use crate::infrastructure::{project_index, repository, workspace};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::VecDeque,
    io::Read,
    path::{Path, PathBuf},
};

const DEFAULT_LOG_LIMIT: usize = 50;

/// The response budget for remote file content. It matches the local `read_file` limit and keeps
/// one response under the transport line limit.
const REMOTE_LIMIT: usize = 512 * 1024;

const CHANGED: &str = "The file changed after it was read. Read it again, then apply your change.";
const WRITE_UNAVAILABLE: &str =
    "IWE Code could not read the file before writing. Check that the file exists.";

/// The tool catalog served through tools/list.
pub(crate) fn list() -> Value {
    json!({
        "tools": [
            tool(
                "list_projects",
                "List the spaces and projects of the IWE Code workspace, including the selected project.",
                json!({ "type": "object", "properties": {} }),
            ),
            tool(
                "list_files",
                "List files and directories at one directory level of a workspace project.",
                json!({
                    "type": "object",
                    "properties": {
                        "project": project_parameter(),
                        "directory": {
                            "type": "string",
                            "description": "Directory path relative to the project root. Defaults to the root."
                        }
                    }
                }),
            ),
            tool(
                "read_file",
                "Read a UTF-8 text file from a workspace project. Files above 512 KiB or 100,000 lines are rejected. The response carries a revision token that write_file requires.",
                json!({
                    "type": "object",
                    "properties": {
                        "project": project_parameter(),
                        "path": {
                            "type": "string",
                            "description": "File path relative to the project root."
                        }
                    },
                    "required": ["path"]
                }),
            ),
            tool(
                "write_file",
                "Replace the content of an existing text file. Requires the revision token from the latest read_file. The write fails when the file changed after that read.",
                json!({
                    "type": "object",
                    "properties": {
                        "project": project_parameter(),
                        "path": {
                            "type": "string",
                            "description": "File path relative to the project root."
                        },
                        "content": {
                            "type": "string",
                            "description": "The complete new file content."
                        },
                        "revision": {
                            "type": "string",
                            "description": "The revision token returned by read_file for this file."
                        }
                    },
                    "required": ["path", "content", "revision"]
                }),
            ),
            tool(
                "search_files",
                "Rank the file names and paths of a workspace project against a fuzzy query.",
                json!({
                    "type": "object",
                    "properties": {
                        "query": {
                            "type": "string",
                            "description": "Fuzzy subsequence to search for."
                        },
                        "project": project_parameter(),
                        "limit": {
                            "type": "number",
                            "description": "Maximum results, up to 50. Defaults to 50."
                        }
                    },
                    "required": ["query"]
                }),
            ),
            tool(
                "git_status",
                "Show the current branch and the changed files of the Git repositories inside a workspace project.",
                json!({
                    "type": "object",
                    "properties": { "project": project_parameter() }
                }),
            ),
            tool(
                "git_log",
                "Show recent commits with authors, dates, and refs for the Git repositories inside a workspace project.",
                json!({
                    "type": "object",
                    "properties": {
                        "project": project_parameter(),
                        "limit": {
                            "type": "number",
                            "description": "Maximum commits, up to 250. Defaults to 50."
                        }
                    }
                }),
            ),
            tool(
                "list_deployments",
                "List the deployment connections, SSH keys, and profiles of the workspace. Profiles map project folders to remote server folders. The response never carries secrets.",
                json!({ "type": "object", "properties": {} }),
            ),
            tool(
                "resolve_deployment_path",
                "Translate one path across a deployment profile mapping. Provide a project-relative local path to get its remote path, or an absolute remote path to get its project-relative local path. Planning only: run transfers with the iwe command line.",
                json!({
                    "type": "object",
                    "properties": {
                        "profile": {
                            "type": "string",
                            "description": "Deployment profile name."
                        },
                        "local": {
                            "type": "string",
                            "description": "Project-relative local path to translate to a remote path."
                        },
                        "remote": {
                            "type": "string",
                            "description": "Absolute remote path to translate to a project-relative local path."
                        }
                    },
                    "required": ["profile"]
                }),
            ),
            tool(
                "deployment_read_remote",
                "Read one remote UTF-8 text file through a deployment profile connection. Files above 512 KiB are rejected. The response never carries credentials and the server is never modified.",
                json!({
                    "type": "object",
                    "properties": {
                        "profile": {
                            "type": "string",
                            "description": "Deployment profile name."
                        },
                        "path": {
                            "type": "string",
                            "description": "Absolute remote file path."
                        }
                    },
                    "required": ["profile", "path"]
                }),
            ),
            tool(
                "deployment_compare",
                "Compare one local project file with its remote counterpart through a deployment profile. Returns both texts and an identical flag. Files above 512 KiB are rejected. The server is never modified.",
                json!({
                    "type": "object",
                    "properties": {
                        "profile": {
                            "type": "string",
                            "description": "Deployment profile name."
                        },
                        "path": {
                            "type": "string",
                            "description": "Project-relative local file path."
                        }
                    },
                    "required": ["profile", "path"]
                }),
            ),
        ]
    })
}

fn tool(name: &str, description: &str, schema: Value) -> Value {
    json!({ "name": name, "description": description, "inputSchema": schema })
}

fn project_parameter() -> Value {
    json!({
        "type": "string",
        "description": "Project name. Defaults to the selected project."
    })
}

/// Run one named tool with its arguments.
pub(crate) async fn dispatch(name: &str, arguments: &Value) -> Result<Value, String> {
    match name {
        "list_projects" => list_projects().await,
        "list_files" => list_files(arguments).await,
        "read_file" => read_file(arguments).await,
        "write_file" => write_file(arguments).await,
        "search_files" => search_files(arguments).await,
        "git_status" => git_status(arguments).await,
        "git_log" => git_log(arguments).await,
        "list_deployments" => list_deployments().await,
        "resolve_deployment_path" => resolve_deployment_path(arguments).await,
        "deployment_read_remote" => deployment_read_remote(arguments).await,
        "deployment_compare" => deployment_compare(arguments).await,
        _ => Err(format!(
            "Unknown tool '{name}'. Call tools/list for the available tools."
        )),
    }
}

async fn list_projects() -> Result<Value, String> {
    let workspace = workspace_metadata().await?;
    let spaces = workspace
        .spaces
        .iter()
        .enumerate()
        .map(|(space_index, space)| {
            json!({
                "name": space.name,
                "selected": workspace.selected_space == Some(space_index),
                "projects": space
                    .projects
                    .iter()
                    .enumerate()
                    .map(|(project_index, project)| {
                        json!({
                            "name": project.name,
                            "directory": project.directory.display().to_string(),
                            "pinned": project.pinned,
                            "selected": workspace.selected_space == Some(space_index)
                                && workspace.selected_project == Some(project_index),
                        })
                    })
                    .collect::<Vec<_>>(),
            })
        })
        .collect::<Vec<_>>();
    Ok(json!({ "spaces": spaces }))
}

async fn list_files(arguments: &Value) -> Result<Value, String> {
    let name = optional_string(arguments, "project")?;
    let project = resolve_project(name.as_deref()).await?;
    let directory = match optional_string(arguments, "directory")? {
        Some(text) => Some(relative_path(&text, "directory")?),
        None => None,
    };
    let root = project.directory.clone();
    let target = directory
        .as_ref()
        .map(|directory| root.join(directory))
        .unwrap_or_else(|| root.clone());
    let base = project.directory.clone();
    let entries = tokio::task::spawn_blocking(move || workspace::entries_at(&root, &target))
        .await
        .map_err(|_| "The file listing failed.".to_owned())?
        .map_err(failure_message)?;
    let listed = entries
        .iter()
        .map(|entry| {
            let path = entry.path.strip_prefix(&base).unwrap_or(&entry.path);
            json!({
                "name": entry.name,
                "path": forward_slashes(path),
                "type": if entry.directory { "directory" } else { "file" },
            })
        })
        .collect::<Vec<_>>();
    Ok(json!({
        "directory": directory.as_deref().map(forward_slashes).unwrap_or_default(),
        "entries": listed,
    }))
}

async fn read_file(arguments: &Value) -> Result<Value, String> {
    let name = optional_string(arguments, "project")?;
    let project = resolve_project(name.as_deref()).await?;
    let relative = relative_path(&required_string(arguments, "path")?, "path")?;
    let root = project.directory;
    let target = root.join(&relative);
    let content = workspace::read(root, target)
        .await
        .map_err(failure_message)?;
    Ok(json!({
        "path": forward_slashes(&relative),
        "revision": revision_of(&content),
        "content": content,
    }))
}

async fn write_file(arguments: &Value) -> Result<Value, String> {
    let name = optional_string(arguments, "project")?;
    let project = resolve_project(name.as_deref()).await?;
    let relative = relative_path(&required_string(arguments, "path")?, "path")?;
    let content = required_string(arguments, "content")?;
    let revision = required_string(arguments, "revision")?;
    if content.len() > LIMIT {
        return Err("The new content exceeds the file size limit.".to_owned());
    }
    if !valid_revision(&revision) {
        return Err("The 'revision' argument must be the token returned by read_file.".to_owned());
    }
    let root = project
        .directory
        .canonicalize()
        .map_err(|_| "The project directory is missing.".to_owned())?;
    let resolved = root.join(&relative).canonicalize().map_err(|_| {
        "The file does not exist. write_file changes existing files only.".to_owned()
    })?;
    if !resolved.starts_with(&root) {
        return Err("The file is outside the project.".to_owned());
    }
    let (path, next) = tokio::task::spawn_blocking(move || -> Result<(String, String), String> {
        let file = std::fs::File::open(&resolved).map_err(|_| WRITE_UNAVAILABLE.to_owned())?;
        let mut bytes = Vec::new();
        file.take(LIMIT as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| WRITE_UNAVAILABLE.to_owned())?;
        if bytes.len() > LIMIT {
            return Err("This file is too large to edit through IWE Code.".to_owned());
        }
        let current = String::from_utf8(bytes)
            .map_err(|_| "This file is not a UTF-8 text file.".to_owned())?;
        if revision_of(&current) != revision.to_ascii_lowercase() {
            return Err(CHANGED.to_owned());
        }
        save_file(&resolved, &current, &content).map_err(save_message)?;
        let relative = resolved.strip_prefix(&root).unwrap_or(&resolved);
        Ok((forward_slashes(relative), revision_of(&content)))
    })
    .await
    .map_err(|_| "The file operation failed.".to_owned())??;
    Ok(json!({ "path": path, "revision": next }))
}

async fn search_files(arguments: &Value) -> Result<Value, String> {
    let name = optional_string(arguments, "project")?;
    let project = resolve_project(name.as_deref()).await?;
    let query = required_string(arguments, "query")?;
    let limit = optional_count(arguments, "limit")?
        .unwrap_or(search::MAX_RESULTS)
        .clamp(1, search::MAX_RESULTS);
    let root = project.directory.clone();
    let files = tokio::task::spawn_blocking(move || collect_candidates(&root))
        .await
        .map_err(|_| "The search failed.".to_owned())?;
    let results = search::rank(&query, &files)
        .into_iter()
        .take(limit)
        .map(|index| {
            let file = &files[index];
            let path = file
                .path
                .strip_prefix(&project.directory)
                .unwrap_or(&file.path);
            json!({ "name": file.name, "path": forward_slashes(path) })
        })
        .collect::<Vec<_>>();
    Ok(json!({ "query": query, "results": results }))
}

async fn git_status(arguments: &Value) -> Result<Value, String> {
    let name = optional_string(arguments, "project")?;
    let project = resolve_project(name.as_deref()).await?;
    let repository = repository::load(project.directory.clone(), None)
        .await
        .map_err(failure_message)?;
    let worktree = repository::working_status(project.directory.clone())
        .await
        .map_err(failure_message)?;
    let branches = repository
        .branches
        .iter()
        .map(|branch| json!({ "name": branch.name, "current": branch.current }))
        .collect::<Vec<_>>();
    let files = worktree
        .files
        .iter()
        .map(|(path, change)| {
            let relative = path.strip_prefix(&project.directory).unwrap_or(path);
            json!({ "path": forward_slashes(relative), "status": status_name(change.status) })
        })
        .collect::<Vec<_>>();
    Ok(json!({
        "head": repository.head,
        "branches": branches,
        "changedFiles": files,
    }))
}

async fn git_log(arguments: &Value) -> Result<Value, String> {
    let name = optional_string(arguments, "project")?;
    let project = resolve_project(name.as_deref()).await?;
    let limit = optional_count(arguments, "limit")?
        .unwrap_or(DEFAULT_LOG_LIMIT)
        .clamp(1, repository::HISTORY_PAGE_SIZE);
    let repository = repository::load(project.directory.clone(), None)
        .await
        .map_err(failure_message)?;
    let commits = repository
        .commits
        .iter()
        .take(limit)
        .map(|commit| {
            json!({
                "id": commit.id,
                "subject": commit.subject,
                "author": commit.author,
                "date": commit.date,
                "refs": commit.refs,
            })
        })
        .collect::<Vec<_>>();
    Ok(json!({ "head": repository.head, "commits": commits }))
}

/// List deployment connections, keys, and profiles without secrets.
async fn list_deployments() -> Result<Value, String> {
    let workspace = workspace_metadata().await?;
    let settings = &workspace.deployment;
    let connections = settings
        .connections
        .iter()
        .map(|connection| {
            json!({
                "name": connection.name,
                "protocol": protocol_name(connection.protocol),
                "host": connection.host,
                "port": connection.port,
                "username": connection.username,
                "authentication": authentication_name(&connection.authentication),
                "key": key_name(settings, &connection.authentication),
            })
        })
        .collect::<Vec<_>>();
    let keys = settings
        .keys
        .iter()
        .map(|key| {
            json!({
                "name": key.name,
                "path": key.path.display().to_string(),
            })
        })
        .collect::<Vec<_>>();
    let spaces = workspace
        .spaces
        .iter()
        .map(|space| {
            json!({
                "name": space.name,
                "profiles": space
                    .deployments
                    .iter()
                    .map(|profile| profile_json(settings, profile))
                    .collect::<Vec<_>>(),
            })
        })
        .collect::<Vec<_>>();
    Ok(json!({
        "connections": connections,
        "keys": keys,
        "spaces": spaces,
    }))
}

/// Resolve one deployment profile by name across the workspace spaces.
fn deployment_profile<'a>(workspace: &'a Workspace, name: &str) -> Result<&'a Profile, String> {
    let matches: Vec<&Profile> = workspace
        .spaces
        .iter()
        .flat_map(|space| space.deployments.iter())
        .filter(|profile| profile.name == name || profile.id == name)
        .collect();
    match matches.as_slice() {
        [profile] => Ok(profile),
        [] => Err(format!(
            "No deployment profile matches '{name}'. Call list_deployments for the profiles."
        )),
        _ => Err(format!(
            "Several profiles are named '{name}'. Profiles must have unique names to resolve paths."
        )),
    }
}

/// Run one deployment transfer for a profile and map failures to tool messages.
async fn deployment_transfer(
    workspace: &Workspace,
    profile: &Profile,
    operation: Operation,
) -> Result<Output, String> {
    deployment_transfer_with_secret(workspace, profile, operation, None).await
}

/// Run one deployment transfer with an explicit secret. Production reads the stored credentials,
/// so the secret stays empty there.
async fn deployment_transfer_with_secret(
    workspace: &Workspace,
    profile: &Profile,
    operation: Operation,
    secret: Option<crate::infrastructure::deployment::transport::Secret>,
) -> Result<Output, String> {
    let connection = workspace
        .deployment
        .connections
        .iter()
        .find(|connection| connection.id == profile.connection)
        .cloned()
        .ok_or_else(|| "The profile references a missing connection.".to_owned())?;
    let request = Request {
        connection,
        settings: workspace.deployment.clone(),
        profile: profile.clone(),
        operation,
        secret,
    };
    tokio::task::spawn_blocking(move || transfer::execute(request, &Progress::default()))
        .await
        .map_err(|_| "The deployment operation failed.".to_owned())?
        .map_err(deployment_transfer_message)
}

/// Read one remote UTF-8 text file through a profile connection.
async fn deployment_read_remote(arguments: &Value) -> Result<Value, String> {
    let workspace = workspace_metadata().await?;
    let name = required_string(arguments, "profile")?;
    let profile = deployment_profile(&workspace, &name)?;
    let path = required_string(arguments, "path")?;
    let output = deployment_transfer(&workspace, profile, Operation::Preview(path.clone())).await?;
    let Output::Preview { text, .. } = output else {
        return Err("The deployment operation failed.".to_owned());
    };
    if text.len() > REMOTE_LIMIT {
        return Err(too_large());
    }
    Ok(json!({
        "profile": profile.name,
        "path": path,
        "content": *text,
    }))
}

/// Compare one local project file with its remote counterpart.
async fn deployment_compare(arguments: &Value) -> Result<Value, String> {
    let workspace = workspace_metadata().await?;
    let name = required_string(arguments, "profile")?;
    let profile = deployment_profile(&workspace, &name)?;
    let local = relative_path(&required_string(arguments, "path")?, "path")?;
    let output = deployment_transfer(
        &workspace,
        profile,
        Operation::Compare(profile.project.join(&local)),
    )
    .await?;
    let Output::Preview {
        text,
        previous,
        remote,
        ..
    } = output
    else {
        return Err("The deployment operation failed.".to_owned());
    };
    let Some(previous) = previous else {
        return Err("The deployment operation failed.".to_owned());
    };
    if text.len() > REMOTE_LIMIT || previous.len() > REMOTE_LIMIT {
        return Err(too_large());
    }
    Ok(json!({
        "profile": profile.name,
        "local": forward_slashes(&local),
        "remote": remote,
        "identical": *text == *previous,
        "localText": *text,
        "remoteText": *previous,
    }))
}

fn too_large() -> String {
    "This file is too large for the deployment tools. Use the iwe deployment command line for bigger transfers.".to_owned()
}

/// Translate one path across one profile mapping.
async fn resolve_deployment_path(arguments: &Value) -> Result<Value, String> {
    let workspace = workspace_metadata().await?;
    let name = required_string(arguments, "profile")?;
    let profile = deployment_profile(&workspace, &name)?;
    let local = optional_string(arguments, "local")?;
    let remote = optional_string(arguments, "remote")?;
    match (local, remote) {
        (Some(local), None) => {
            let relative = relative_path(&local, "local")?;
            let absolute = profile.project.join(relative);
            let remote = profile.to_remote(&absolute).map_err(deployment_message)?;
            Ok(json!({
                "profile": profile.name,
                "local": forward_slashes(absolute.strip_prefix(&profile.project).unwrap_or(&absolute)),
                "remote": remote,
            }))
        }
        (None, Some(remote)) => {
            let local = profile.to_local(&remote).map_err(deployment_message)?;
            Ok(json!({
                "profile": profile.name,
                "remote": remote,
                "local": forward_slashes(local.strip_prefix(&profile.project).unwrap_or(&local)),
            }))
        }
        (Some(_), Some(_)) => {
            Err("Provide either the 'local' or the 'remote' argument, not both.".to_owned())
        }
        (None, None) => {
            Err("Provide the 'local' or the 'remote' argument to translate.".to_owned())
        }
    }
}

fn profile_json(settings: &Connections, profile: &Profile) -> Value {
    json!({
        "name": profile.name,
        "project": profile.project.display().to_string(),
        "connection": settings
            .connections
            .iter()
            .find(|connection| connection.id == profile.connection)
            .map(|connection| connection.name.clone())
            .unwrap_or_else(|| profile.connection.clone()),
        "root": profile.root,
        "mappings": profile
            .mappings
            .iter()
            .map(|mapping| {
                json!({
                    "local": mapping.local.display().to_string(),
                    "remote": mapping.remote,
                })
            })
            .collect::<Vec<_>>(),
        "excluded": profile
            .excluded
            .iter()
            .map(|path| path.display().to_string())
            .collect::<Vec<_>>(),
        "default": profile.default,
    })
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

fn key_name(
    settings: &crate::domain::deployment::Connections,
    authentication: &Authentication,
) -> Option<String> {
    match authentication {
        Authentication::Key(id) => settings
            .keys
            .iter()
            .find(|key| &key.id == id)
            .map(|key| key.name.clone()),
        _ => None,
    }
}

/// Map deployment domain errors to tool messages.
fn deployment_message(error: DeploymentError) -> String {
    match error {
        DeploymentError::Mapping => "The path is outside the profile mappings.".to_owned(),
        DeploymentError::Excluded => "The path is excluded by the profile.".to_owned(),
        _ => "The profile settings are not valid.".to_owned(),
    }
}

/// Map one deployment transfer error to a generic tool message.
fn deployment_transfer_message(error: DeploymentError) -> String {
    match error {
        DeploymentError::Connection => "IWE Code could not connect to the deployment server. Check that the server is reachable and the connection settings are current.".to_owned(),
        DeploymentError::Credentials => "The deployment server rejected the stored credentials. Update the connection or key.".to_owned(),
        DeploymentError::HostUnknown(fingerprint) => format!(
            "The server key fingerprint {fingerprint} is not trusted. Verify the key, then run: iwe deployment test <profile> --trust-host-key {fingerprint}"
        ),
        DeploymentError::HostChanged => "The server key changed since the last connection. Verify the server before connecting again.".to_owned(),
        DeploymentError::TooLarge => too_large(),
        DeploymentError::Binary => "This file is not a UTF-8 text file.".to_owned(),
        DeploymentError::Mapping => "The path is outside the profile mappings.".to_owned(),
        DeploymentError::Excluded => "The path is excluded by the profile.".to_owned(),
        DeploymentError::Access => "IWE Code could not read this file or folder on the server.".to_owned(),
        DeploymentError::Configuration => "The deployment profile settings are not valid.".to_owned(),
        _ => "The deployment operation failed.".to_owned(),
    }
}

/// Load workspace metadata from disk without applying launch arguments.
async fn workspace_metadata() -> Result<Workspace, String> {
    workspace::load_metadata()
        .await
        .map_err(|_| "IWE Code could not read the workspace metadata.".to_owned())
}

/// Resolve the named project, or fall back to the selected one.
async fn resolve_project(name: Option<&str>) -> Result<Project, String> {
    let workspace = workspace_metadata().await?;
    match name {
        Some(name) => workspace
            .spaces
            .iter()
            .find_map(|space| space.projects.iter().find(|project| project.name == name))
            .cloned()
            .ok_or_else(|| unknown_project(name, &workspace)),
        None => workspace
            .project()
            .cloned()
            .ok_or_else(|| "No project is selected. List the projects first.".to_owned()),
    }
}

fn unknown_project(name: &str, workspace: &Workspace) -> String {
    let mut names: Vec<String> = workspace
        .spaces
        .iter()
        .flat_map(|space| space.projects.iter().map(|project| project.name.clone()))
        .collect();
    names.sort();
    names.dedup();
    if names.is_empty() {
        return format!("Unknown project '{name}'. The workspace has no projects.");
    }
    names.truncate(20);
    format!(
        "Unknown project '{name}'. Available projects: {}.",
        names.join(", ")
    )
}

/// Validate a project-relative path argument.
///
/// The path must be relative, nonempty, and free of root or parent components.
fn relative_path(text: &str, name: &str) -> Result<PathBuf, String> {
    if text.is_empty() {
        return Err(format!("The '{name}' argument must not be empty."));
    }
    let path = Path::new(text);
    if path.components().any(|component| {
        matches!(
            component,
            std::path::Component::Prefix(_) | std::path::Component::RootDir
        )
    }) {
        return Err(format!(
            "The '{name}' argument must be relative to the project root."
        ));
    }
    if path
        .components()
        .any(|component| matches!(component, std::path::Component::ParentDir))
    {
        return Err(format!("The '{name}' argument must not contain '..'."));
    }
    Ok(path.to_path_buf())
}

fn optional_string(arguments: &Value, name: &str) -> Result<Option<String>, String> {
    match arguments.get(name) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(text)) => Ok(Some(text.clone())),
        Some(_) => Err(format!("The '{name}' argument must be a string.")),
    }
}

fn required_string(arguments: &Value, name: &str) -> Result<String, String> {
    optional_string(arguments, name)?.ok_or_else(|| format!("The '{name}' argument is required."))
}

fn optional_count(arguments: &Value, name: &str) -> Result<Option<usize>, String> {
    match arguments.get(name) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Number(number)) => {
            let value = number
                .as_u64()
                .ok_or_else(|| format!("The '{name}' argument must be a positive number."))?;
            usize::try_from(value)
                .map(Some)
                .map_err(|_| format!("The '{name}' argument is too large."))
        }
        Some(_) => Err(format!("The '{name}' argument must be a number.")),
    }
}

/// Render a project-relative path with forward slashes.
fn forward_slashes(path: &Path) -> String {
    path.components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

/// Hash text into the opaque revision token shared with write_file.
fn revision_of(text: &str) -> String {
    let digest = Sha256::digest(text.as_bytes());
    format!("{digest:x}")
}

fn valid_revision(revision: &str) -> bool {
    revision.len() == 64
        && revision
            .chars()
            .all(|character| character.is_ascii_hexdigit())
}

fn save_message(error: SaveError) -> String {
    match error {
        SaveError::Changed => CHANGED.to_owned(),
        SaveError::TooLarge => "The new content exceeds the file size limit.".to_owned(),
        SaveError::Symlink => "The file must be a regular file.".to_owned(),
        SaveError::Access => {
            "IWE Code could not write the file. Check that it is writable.".to_owned()
        }
    }
}

fn failure_message(error: Failure) -> String {
    match error {
        Failure::MissingTool => "Git is not available on this computer.".to_owned(),
        Failure::NotRepository => "This project contains no Git repository.".to_owned(),
        Failure::TooLarge => "This file is too large to read through the MCP server.".to_owned(),
        Failure::Binary => "This file is not a UTF-8 text file.".to_owned(),
        _ => "IWE Code could not read this data.".to_owned(),
    }
}

fn status_name(status: FileStatus) -> &'static str {
    match status {
        FileStatus::Untracked => "untracked",
        FileStatus::Added => "added",
        FileStatus::Deleted => "deleted",
        FileStatus::Renamed => "renamed",
        FileStatus::Modified => "modified",
        FileStatus::Conflict => "conflict",
    }
}

/// Collect bounded file candidates with the same limits as the background tree.
fn collect_candidates(root: &Path) -> Vec<FileCandidate> {
    let mut files = Vec::new();
    let mut queue = VecDeque::from([root.to_path_buf()]);
    let mut visited = 0usize;
    while let Some(directory) = queue.pop_front() {
        visited += 1;
        if visited > project_index::MAX_DIRECTORIES || files.len() >= project_index::MAX_ENTRIES {
            break;
        }
        let Ok(entries) = workspace::entries_at(root, &directory) else {
            continue;
        };
        for entry in entries {
            if entry.directory {
                if queue.len() < project_index::MAX_DIRECTORIES
                    && project_index::prefetch(root, &entry.path)
                {
                    queue.push_back(entry.path);
                }
            } else if files.len() < project_index::MAX_ENTRIES {
                let relative = entry.path.strip_prefix(root).unwrap_or(&entry.path);
                files.push(FileCandidate {
                    directory: forward_slashes(relative.parent().unwrap_or(relative)),
                    name: entry.name,
                    path: entry.path,
                });
            }
        }
    }
    files
}

#[cfg(test)]
mod tests;
