//! Run deployment commands against workspace settings and remote servers.

use super::{args, output};
use crate::domain::deployment::{
    Authentication, Connection, Connections, Error, Mapping, PrivateKey, Profile, Protocol,
    remote_path, safe_text,
};
use crate::domain::workspace::Workspace;
use crate::i18n::{FluentArgs, I18n, Key};
use crate::infrastructure::deployment::{
    identifier,
    transfer::{self, Operation, Output, Request},
    transport::{self, Progress, Secret},
};
use crate::infrastructure::secret_files;
use std::{path::PathBuf, process::ExitCode};
use zeroize::Zeroizing;

/// One failed deployment command.
#[derive(Debug)]
pub(crate) enum Failure {
    /// Invalid option values or settings. Exit code 2.
    Invalid(Key, Option<String>),
    /// A missing connection, key, profile, or space. Exit code 1.
    Missing(Key, String),
    /// A missing local path. Exit code 1.
    Path(Key, String),
    /// A deployment transfer error. Exit code 1.
    Transfer(Error),
    /// A plain failure without values. Exit code 1.
    Plain(Key),
}

/// The authentication choice behind one connection.
enum AuthChoice {
    Password,
    Agent,
    Key,
}

/// Interpret one deployment subcommand.
pub(crate) fn dispatch(i18n: &I18n, command: args::Deployment) -> ExitCode {
    match command {
        args::Deployment::List { json } => list(i18n, json),
        args::Deployment::ConnectionAdd(draft) => connection_add(i18n, draft),
        args::Deployment::ConnectionSet { name, changes } => connection_set(i18n, &name, changes),
        args::Deployment::ConnectionRemove { name, json } => connection_remove(i18n, &name, json),
        args::Deployment::KeyAdd { name, path, json } => key_add(i18n, &name, path, json),
        args::Deployment::KeyRemove { name, json } => key_remove(i18n, &name, json),
        args::Deployment::ProfileAdd(draft) => profile_add(i18n, draft),
        args::Deployment::ProfileSet { name, changes } => profile_set(i18n, &name, changes),
        args::Deployment::ProfileRemove { name, space, json } => {
            profile_remove(i18n, &name, space.as_deref(), json)
        }
        args::Deployment::Test {
            profile,
            space,
            trust,
            json,
        } => run(
            i18n,
            |_| Ok(Operation::Test),
            &profile,
            space.as_deref(),
            trust,
            json,
        ),
        args::Deployment::Upload {
            profile,
            path,
            space,
            trust,
            json,
        } => upload(i18n, &profile, path, space.as_deref(), trust, json),
        args::Deployment::Download {
            profile,
            remote,
            overwrite,
            space,
            trust,
            json,
        } => run(
            i18n,
            |_| {
                remote_path(&remote)
                    .map(|remote| Operation::Download(remote, overwrite))
                    .map_err(|_| Failure::Invalid(Key::CliErrorInvalidPath, Some(remote.clone())))
            },
            &profile,
            space.as_deref(),
            trust,
            json,
        ),
        args::Deployment::ListRemote {
            profile,
            directory,
            space,
            trust,
            json,
        } => run(
            i18n,
            |profile: &Profile| match directory.as_deref() {
                Some(directory) => remote_path(directory).map(Operation::List).map_err(|_| {
                    Failure::Invalid(Key::CliErrorInvalidPath, Some(directory.to_owned()))
                }),
                None => Ok(Operation::List(profile.root.clone())),
            },
            &profile,
            space.as_deref(),
            trust,
            json,
        ),
    }
}

/// Print connections, keys, and profiles of the workspace.
fn list(i18n: &I18n, json: bool) -> ExitCode {
    let workspace = match super::load(i18n) {
        Ok(workspace) => workspace,
        Err(code) => return code,
    };
    if json {
        return super::emit_json(i18n, &output::DeploymentView::from_workspace(&workspace));
    }
    output::print_deployments(i18n, &workspace)
}

/// Add one connection with an optional secret read from standard input.
fn connection_add(i18n: &I18n, draft: args::ConnectionDraft) -> ExitCode {
    let workspace = match super::load(i18n) {
        Ok(workspace) => workspace,
        Err(code) => return code,
    };
    let connection = match ensured(i18n, connection_of(&workspace.deployment, &draft)) {
        Ok(connection) => connection,
        Err(code) => return code,
    };
    if draft.password_stdin
        && let Err(error) = store_connection_secret(&connection)
    {
        return exit(i18n, Failure::Transfer(error));
    }
    let view =
        output::DeploymentConnectionView::from_connection(&workspace.deployment, &connection);
    let name = connection.name.clone();
    let mut workspace = workspace;
    workspace.deployment.connections.push(connection);
    match super::save(i18n, workspace) {
        Ok(()) => {}
        Err(code) => return code,
    }
    if draft.json {
        return super::emit_json(i18n, &view);
    }
    super::message(i18n, Key::CliConnectionAdded, name_arguments(&name))
}

/// Change one connection.
fn connection_set(i18n: &I18n, name: &str, changes: args::ConnectionChanges) -> ExitCode {
    let mut workspace = match super::load(i18n) {
        Ok(workspace) => workspace,
        Err(code) => return code,
    };
    let index = match ensured(i18n, connection_index(&workspace.deployment, name)) {
        Ok(index) => index,
        Err(code) => return code,
    };
    if changes.password_stdin && changes.auth.as_deref() == Some("agent") {
        return exit(i18n, Failure::Plain(Key::CliErrorPasswordStdinAgent));
    }
    // Resolve every option against the saved settings before mutating them.
    let protocol = match ensured(i18n, protocol_of(changes.protocol.as_deref())) {
        Ok(protocol) => protocol,
        Err(code) => return code,
    };
    let port = match ensured(i18n, port_of(changes.port.as_deref(), protocol)) {
        Ok(port) => port,
        Err(code) => return code,
    };
    let authentication = match ensured(
        i18n,
        authentication_of(
            changes.auth.as_deref(),
            changes.key.as_deref(),
            &workspace.deployment,
        ),
    ) {
        Ok(authentication) => authentication,
        Err(code) => return code,
    };
    {
        let connection = &mut workspace.deployment.connections[index];
        let previous = connection.clone();
        if let Some(host) = non_empty(changes.host.as_deref().unwrap_or_default()) {
            connection.host = host;
        }
        if let Some(username) = non_empty(changes.username.as_deref().unwrap_or_default()) {
            connection.username = username;
        }
        if changes.protocol.is_some() {
            connection.protocol = protocol;
            if protocol != Protocol::Sftp {
                connection.parse_ssh_config = false;
            }
            if changes.port.is_none() {
                connection.port = port;
            }
        }
        if changes.port.is_some() {
            connection.port = port;
        }
        if changes.auth.is_some() {
            connection.authentication = authentication;
        }
        if previous.host != connection.host
            || previous.port != connection.port
            || previous.protocol != connection.protocol
            || previous.uses_ssh_config() != connection.uses_ssh_config()
            || (connection.uses_ssh_config() && previous.username != connection.username)
        {
            connection.trusted_host_key = None;
        }
        if connection.validate().is_err() {
            return exit(
                i18n,
                Failure::Invalid(deployment_error_key(&Error::Configuration), None),
            );
        }
    }
    let view = output::DeploymentConnectionView::from_connection(
        &workspace.deployment,
        &workspace.deployment.connections[index],
    );
    let name = workspace.deployment.connections[index].name.clone();
    if changes.password_stdin
        && let Err(error) = store_connection_secret(&workspace.deployment.connections[index])
    {
        return exit(i18n, Failure::Transfer(error));
    }
    match super::save(i18n, workspace) {
        Ok(()) => {}
        Err(code) => return code,
    }
    if changes.json {
        return super::emit_json(i18n, &view);
    }
    super::message(i18n, Key::CliConnectionUpdated, name_arguments(&name))
}

/// Remove one unused connection and its stored secret.
fn connection_remove(i18n: &I18n, name: &str, json: bool) -> ExitCode {
    let mut workspace = match super::load(i18n) {
        Ok(workspace) => workspace,
        Err(code) => return code,
    };
    let index = match ensured(i18n, connection_index(&workspace.deployment, name)) {
        Ok(index) => index,
        Err(code) => return code,
    };
    let connection = workspace.deployment.connections[index].clone();
    if workspace.spaces.iter().any(|space| {
        space
            .deployments
            .iter()
            .any(|p| p.connection == connection.id)
    }) {
        return exit(
            i18n,
            Failure::Missing(Key::CliErrorConnectionInUse, connection.name.clone()),
        );
    }
    workspace.deployment.connections.remove(index);
    if connection.authentication == Authentication::Password
        && let Ok(path) = secret_files::path("deployment", &connection.id)
    {
        let _ = secret_files::delete(&path);
    }
    match super::save(i18n, workspace) {
        Ok(()) => {}
        Err(code) => return code,
    }
    if json {
        return super::emit_json(i18n, &output::RemovedNameView::from_name(&connection.name));
    }
    super::message(
        i18n,
        Key::CliConnectionRemoved,
        name_arguments(&connection.name),
    )
}

/// Register one SSH key file.
fn key_add(i18n: &I18n, name: &str, path: PathBuf, json: bool) -> ExitCode {
    let mut workspace = match super::load(i18n) {
        Ok(workspace) => workspace,
        Err(code) => return code,
    };
    let path = match std::fs::canonicalize(&path) {
        Ok(path) if path.is_file() => path,
        _ => {
            return exit(
                i18n,
                Failure::Path(Key::CliErrorPathNotFound, path.display().to_string()),
            );
        }
    };
    if name.trim().is_empty() || !safe_text(name) {
        return exit(
            i18n,
            Failure::Invalid(deployment_error_key(&Error::Configuration), None),
        );
    }
    let id = match identifier() {
        Ok(id) => id,
        Err(_) => return exit(i18n, Failure::Plain(Key::CliErrorUnexpected)),
    };
    let key = PrivateKey {
        id,
        name: name.to_owned(),
        path,
    };
    let view = output::DeploymentKeyView::from_key(&key);
    workspace.deployment.keys.push(key);
    match super::save(i18n, workspace) {
        Ok(()) => {}
        Err(code) => return code,
    }
    if json {
        return super::emit_json(i18n, &view);
    }
    super::message(i18n, Key::CliKeyAdded, name_arguments(name))
}

/// Remove one unused key and its stored passphrase.
fn key_remove(i18n: &I18n, name: &str, json: bool) -> ExitCode {
    let mut workspace = match super::load(i18n) {
        Ok(workspace) => workspace,
        Err(code) => return code,
    };
    let index = match ensured(i18n, key_index(&workspace.deployment, name)) {
        Ok(index) => index,
        Err(code) => return code,
    };
    let key = workspace.deployment.keys[index].clone();
    if workspace
        .deployment
        .connections
        .iter()
        .any(|connection| connection.authentication == Authentication::Key(key.id.clone()))
    {
        return exit(
            i18n,
            Failure::Missing(Key::CliErrorKeyInUse, key.name.clone()),
        );
    }
    workspace.deployment.keys.remove(index);
    if let Ok(path) = secret_files::path("deployment", &key.id) {
        let _ = secret_files::delete(&path);
    }
    match super::save(i18n, workspace) {
        Ok(()) => {}
        Err(code) => return code,
    }
    if json {
        return super::emit_json(i18n, &output::RemovedNameView::from_name(&key.name));
    }
    super::message(i18n, Key::CliKeyRemoved, name_arguments(&key.name))
}

/// Add one profile to one space.
fn profile_add(i18n: &I18n, draft: args::ProfileDraft) -> ExitCode {
    let mut workspace = match super::load(i18n) {
        Ok(workspace) => workspace,
        Err(code) => return code,
    };
    let space_index = match ensured(i18n, space_of(&workspace, draft.space.as_deref())) {
        Ok(index) => index,
        Err(code) => return code,
    };
    let connection = match ensured(
        i18n,
        connection_id(&workspace.deployment, &draft.connection),
    ) {
        Ok(id) => id,
        Err(code) => return code,
    };
    let profile = match ensured(i18n, profile_of(&draft, connection)) {
        Ok(profile) => profile,
        Err(code) => return code,
    };
    let view = output::DeploymentProfileView::from_settings(&workspace.deployment, &profile);
    if profile.default {
        for other in &mut workspace.spaces[space_index].deployments {
            other.default = false;
        }
    }
    workspace.spaces[space_index].deployments.push(profile);
    match super::save(i18n, workspace) {
        Ok(()) => {}
        Err(code) => return code,
    }
    if draft.json {
        return super::emit_json(i18n, &view);
    }
    super::message(i18n, Key::CliProfileAdded, name_arguments(&draft.name))
}

/// Change one profile.
fn profile_set(i18n: &I18n, name: &str, changes: args::ProfileChanges) -> ExitCode {
    let mut workspace = match super::load(i18n) {
        Ok(workspace) => workspace,
        Err(code) => return code,
    };
    let (space_index, profile_index) = match ensured(
        i18n,
        profile_index(&workspace, name, changes.space.as_deref()),
    ) {
        Ok(indices) => indices,
        Err(code) => return code,
    };
    let project = match ensured(
        i18n,
        changes.project.map(existing_directory).unwrap_or_else(|| {
            Ok(workspace.spaces[space_index].deployments[profile_index]
                .project
                .clone())
        }),
    ) {
        Ok(project) => project,
        Err(code) => return code,
    };
    let connection = match ensured(
        i18n,
        match changes.connection.as_deref() {
            Some(connection) => connection_id(&workspace.deployment, connection),
            None => Ok(workspace.spaces[space_index].deployments[profile_index]
                .connection
                .clone()),
        },
    ) {
        Ok(connection) => connection,
        Err(code) => return code,
    };
    let root = match ensured(
        i18n,
        match changes.root.as_deref() {
            Some(root) => remote_path(root)
                .map_err(|_| Failure::Invalid(Key::CliErrorInvalidPath, Some(root.to_owned()))),
            None => Ok(workspace.spaces[space_index].deployments[profile_index]
                .root
                .clone()),
        },
    ) {
        Ok(root) => root,
        Err(code) => return code,
    };
    let mappings = match ensured(
        i18n,
        match changes.mappings.as_ref() {
            Some(values) => mappings_of(values),
            None => Ok(workspace.spaces[space_index].deployments[profile_index]
                .mappings
                .clone()),
        },
    ) {
        Ok(mappings) => mappings,
        Err(code) => return code,
    };
    let excluded = if changes.clear_excluded {
        Vec::new()
    } else {
        match ensured(
            i18n,
            match changes.excluded.as_ref() {
                Some(values) => Ok(values.iter().map(PathBuf::from).collect::<Vec<_>>()),
                None => Ok(workspace.spaces[space_index].deployments[profile_index]
                    .excluded
                    .clone()),
            },
        ) {
            Ok(excluded) => excluded,
            Err(code) => return code,
        }
    };
    {
        let profile = &mut workspace.spaces[space_index].deployments[profile_index];
        profile.project = project;
        profile.connection = connection;
        profile.root = root;
        profile.mappings = mappings;
        profile.excluded = excluded;
        if let Some(default) = changes.default {
            profile.default = default;
        }
        if profile.validate().is_err() {
            return exit(
                i18n,
                Failure::Invalid(deployment_error_key(&Error::Configuration), None),
            );
        }
    }
    if changes.default == Some(true) {
        let id = workspace.spaces[space_index].deployments[profile_index]
            .id
            .clone();
        for other in &mut workspace.spaces[space_index].deployments {
            other.default = other.id == id;
        }
    }
    let updated = workspace.spaces[space_index].deployments[profile_index].clone();
    let view = output::DeploymentProfileView::from_settings(&workspace.deployment, &updated);
    match super::save(i18n, workspace) {
        Ok(()) => {}
        Err(code) => return code,
    }
    if changes.json {
        return super::emit_json(i18n, &view);
    }
    super::message(i18n, Key::CliProfileUpdated, name_arguments(&updated.name))
}

/// Remove one profile.
fn profile_remove(i18n: &I18n, name: &str, space: Option<&str>, json: bool) -> ExitCode {
    let mut workspace = match super::load(i18n) {
        Ok(workspace) => workspace,
        Err(code) => return code,
    };
    let (space_index, profile_index) = match ensured(i18n, profile_index(&workspace, name, space)) {
        Ok(indices) => indices,
        Err(code) => return code,
    };
    let profile = workspace.spaces[space_index]
        .deployments
        .remove(profile_index);
    match super::save(i18n, workspace) {
        Ok(()) => {}
        Err(code) => return code,
    }
    if json {
        return super::emit_json(i18n, &output::RemovedNameView::from_name(&profile.name));
    }
    super::message(i18n, Key::CliProfileRemoved, name_arguments(&profile.name))
}

/// Run one transfer operation against the profile connection.
fn run(
    i18n: &I18n,
    build: impl FnOnce(&Profile) -> Result<Operation, Failure>,
    target: &str,
    space: Option<&str>,
    trust: Option<String>,
    json: bool,
) -> ExitCode {
    let workspace = match super::load(i18n) {
        Ok(workspace) => workspace,
        Err(code) => return code,
    };
    let (space_index, profile_index) = match ensured(i18n, profile_index(&workspace, target, space))
    {
        Ok(indices) => indices,
        Err(code) => return code,
    };
    let profile = workspace.spaces[space_index].deployments[profile_index].clone();
    let operation = match ensured(i18n, build(&profile)) {
        Ok(operation) => operation,
        Err(code) => return code,
    };
    let Some(connection_index) = workspace
        .deployment
        .connections
        .iter()
        .position(|connection| connection.id == profile.connection)
    else {
        return exit(
            i18n,
            Failure::Invalid(deployment_error_key(&Error::Configuration), None),
        );
    };
    let connection = workspace.deployment.connections[connection_index].clone();
    let settings = workspace.deployment.clone();
    if let Some(fingerprint) = trust {
        let mut workspace = workspace;
        workspace.deployment.connections[connection_index].trusted_host_key = Some(fingerprint);
        match super::save(i18n, workspace) {
            Ok(()) => {}
            Err(code) => return code,
        }
    }
    let request = Request {
        connection,
        settings,
        profile: profile.clone(),
        operation,
        secret: None,
    };
    let progress = Progress::default();
    match transfer::execute(request, &progress) {
        Ok(Output::Connected) => {
            if json {
                super::emit_json(i18n, &output::ConnectedView::from_profile(&profile))
            } else {
                super::message(
                    i18n,
                    Key::CliDeploymentConnected,
                    name_arguments(&profile.name),
                )
            }
        }
        Ok(Output::Transferred) => {
            let files = progress.files.load(std::sync::atomic::Ordering::Relaxed);
            let bytes = progress.bytes.load(std::sync::atomic::Ordering::Relaxed);
            if json {
                super::emit_json(
                    i18n,
                    &output::TransferredView::from_counters(&profile, files, bytes),
                )
            } else {
                let mut arguments = FluentArgs::new();
                arguments.set("count", files as i64);
                arguments.set("bytes", bytes as i64);
                super::message(i18n, Key::CliDeploymentTransferred, arguments)
            }
        }
        Ok(Output::Listed(directory, entries)) => {
            if json {
                super::emit_json(
                    i18n,
                    &output::RemoteEntriesView::from_entries(&directory, &entries),
                )
            } else {
                output::print_remote_entries(&directory, &entries);
                ExitCode::SUCCESS
            }
        }
        Ok(_) => ExitCode::SUCCESS,
        Err(error) => exit(i18n, Failure::Transfer(error)),
    }
}

/// Upload one local file or folder.
fn upload(
    i18n: &I18n,
    target: &str,
    path: PathBuf,
    space: Option<&str>,
    trust: Option<String>,
    json: bool,
) -> ExitCode {
    match std::fs::canonicalize(&path) {
        Ok(path) => run(
            i18n,
            |_| Ok(Operation::Upload(path)),
            target,
            space,
            trust,
            json,
        ),
        Err(_) => exit(
            i18n,
            Failure::Path(Key::CliErrorPathNotFound, path.display().to_string()),
        ),
    }
}

/// Build one validated connection draft against the saved settings.
fn connection_of(
    settings: &Connections,
    draft: &args::ConnectionDraft,
) -> Result<Connection, Failure> {
    if draft.password_stdin && draft.auth.as_deref() == Some("agent") {
        return Err(Failure::Plain(Key::CliErrorPasswordStdinAgent));
    }
    let protocol = protocol_of(draft.protocol.as_deref())?;
    let port = port_of(draft.port.as_deref(), protocol)?;
    let authentication = authentication_of(draft.auth.as_deref(), draft.key.as_deref(), settings)?;
    let id = identifier().map_err(|_| Failure::Plain(Key::CliErrorUnexpected))?;
    let connection = Connection {
        id,
        name: draft.name.clone(),
        protocol,
        host: draft.host.clone(),
        port,
        username: draft.username.clone(),
        authentication,
        parse_ssh_config: false,
        trusted_host_key: None,
    };
    connection
        .validate()
        .map(|()| connection)
        .map_err(|error| Failure::Invalid(deployment_error_key(&error), None))
}

/// Build one validated profile draft.
fn profile_of(draft: &args::ProfileDraft, connection: String) -> Result<Profile, Failure> {
    let project = existing_directory(draft.project.clone())?;
    let root = remote_path(&draft.root)
        .map_err(|_| Failure::Invalid(Key::CliErrorInvalidPath, Some(draft.root.clone())))?;
    let mappings = mappings_of(&draft.mappings)?;
    let id = identifier().map_err(|_| Failure::Plain(Key::CliErrorUnexpected))?;
    let profile = Profile {
        id,
        name: draft.name.clone(),
        project,
        connection,
        root,
        mappings,
        excluded: draft.excluded.iter().map(PathBuf::from).collect(),
        default: draft.default.unwrap_or(false),
    };
    profile
        .validate()
        .map(|()| profile)
        .map_err(|error| Failure::Invalid(deployment_error_key(&error), None))
}

/// Resolve the space of a command: the named space, the selected space, or the first space.
fn space_of(workspace: &Workspace, name: Option<&str>) -> Result<usize, Failure> {
    match name {
        Some(name) => workspace
            .spaces
            .iter()
            .position(|space| space.name == name)
            .ok_or_else(|| Failure::Missing(Key::CliErrorSpaceNotFound, name.to_owned())),
        None => workspace
            .selected_space
            .filter(|index| *index < workspace.spaces.len())
            .or(if workspace.spaces.is_empty() {
                None
            } else {
                Some(0)
            })
            .ok_or(Failure::Plain(Key::NoProjects)),
    }
}

/// List the spaces to search, the selected space first.
fn space_order(workspace: &Workspace, space: Option<&str>) -> Result<Vec<usize>, Failure> {
    match space {
        Some(name) => Ok(vec![space_of(workspace, Some(name))?]),
        None => {
            let mut order = Vec::with_capacity(workspace.spaces.len());
            if let Some(selected) = workspace
                .selected_space
                .filter(|index| *index < workspace.spaces.len())
            {
                order.push(selected);
            }
            for index in 0..workspace.spaces.len() {
                if !order.contains(&index) {
                    order.push(index);
                }
            }
            Ok(order)
        }
    }
}

/// Resolve one profile to its space and position, by identifier or name.
pub(crate) fn profile_index(
    workspace: &Workspace,
    target: &str,
    space: Option<&str>,
) -> Result<(usize, usize), Failure> {
    let mut found: Option<(usize, usize)> = None;
    for space_index in space_order(workspace, space)? {
        let Some(profile_index) = workspace.spaces[space_index]
            .deployments
            .iter()
            .position(|profile| profile.id == target || profile.name == target)
        else {
            continue;
        };
        if space.is_none() && workspace.selected_space == Some(space_index) {
            return Ok((space_index, profile_index));
        }
        if found.is_some() {
            return Err(Failure::Missing(
                Key::CliErrorAmbiguousProfile,
                target.to_owned(),
            ));
        }
        found = Some((space_index, profile_index));
    }
    found.ok_or_else(|| Failure::Missing(Key::CliErrorProfileNotFound, target.to_owned()))
}

/// Resolve one connection to its position, by identifier or name.
pub(crate) fn connection_index(settings: &Connections, target: &str) -> Result<usize, Failure> {
    if let Some(index) = settings
        .connections
        .iter()
        .position(|connection| connection.id == target)
    {
        return Ok(index);
    }
    let matches: Vec<usize> = settings
        .connections
        .iter()
        .enumerate()
        .filter(|(_, connection)| connection.name == target)
        .map(|(index, _)| index)
        .collect();
    match matches.as_slice() {
        [index] => Ok(*index),
        [] => Err(Failure::Missing(
            Key::CliErrorConnectionNotFound,
            target.to_owned(),
        )),
        _ => Err(Failure::Missing(
            Key::CliErrorAmbiguousConnection,
            target.to_owned(),
        )),
    }
}

/// Resolve one key to its position, by identifier or name.
pub(crate) fn key_index(settings: &Connections, target: &str) -> Result<usize, Failure> {
    if let Some(index) = settings.keys.iter().position(|key| key.id == target) {
        return Ok(index);
    }
    let matches: Vec<usize> = settings
        .keys
        .iter()
        .enumerate()
        .filter(|(_, key)| key.name == target)
        .map(|(index, _)| index)
        .collect();
    match matches.as_slice() {
        [index] => Ok(*index),
        [] => Err(Failure::Missing(
            Key::CliErrorKeyNotFound,
            target.to_owned(),
        )),
        _ => Err(Failure::Missing(
            Key::CliErrorAmbiguousKeyName,
            target.to_owned(),
        )),
    }
}

/// Resolve one connection identifier, by identifier or name.
fn connection_id(settings: &Connections, target: &str) -> Result<String, Failure> {
    connection_index(settings, target).map(|index| settings.connections[index].id.clone())
}

/// Parse one protocol value.
fn protocol_of(value: Option<&str>) -> Result<Protocol, Failure> {
    match value.unwrap_or("sftp") {
        "sftp" => Ok(Protocol::Sftp),
        "ftp" => Ok(Protocol::Ftp),
        "ftps" => Ok(Protocol::Ftps),
        "ftps-implicit" => Ok(Protocol::FtpsImplicit),
        other => Err(Failure::Invalid(
            Key::CliErrorInvalidProtocol,
            Some(other.to_owned()),
        )),
    }
}

/// Parse one port value with the protocol default.
fn port_of(value: Option<&str>, protocol: Protocol) -> Result<u16, Failure> {
    match value {
        None => Ok(protocol.port()),
        Some(value) => value
            .parse::<u16>()
            .ok()
            .filter(|port| *port > 0)
            .ok_or_else(|| Failure::Invalid(Key::CliErrorInvalidPort, Some(value.to_owned()))),
    }
}

/// Parse one authentication choice with its key requirement.
fn auth_choice_of(value: Option<&str>, key: Option<&str>) -> Result<AuthChoice, Failure> {
    match value.unwrap_or("password") {
        "password" => {
            if key.is_some() {
                return Err(Failure::Plain(Key::CliErrorKeyRequiresAuth));
            }
            Ok(AuthChoice::Password)
        }
        "agent" => Ok(AuthChoice::Agent),
        "key" => key
            .map(|_| AuthChoice::Key)
            .ok_or(Failure::Plain(Key::CliErrorKeyRequiresAuth)),
        other => Err(Failure::Invalid(
            Key::CliErrorInvalidAuth,
            Some(other.to_owned()),
        )),
    }
}

/// Build the authentication value of one connection.
fn authentication_of(
    value: Option<&str>,
    key: Option<&str>,
    settings: &Connections,
) -> Result<Authentication, Failure> {
    match auth_choice_of(value, key)? {
        AuthChoice::Password => Ok(Authentication::Password),
        AuthChoice::Agent => Ok(Authentication::Agent),
        AuthChoice::Key => {
            let index = key_index(settings, key.unwrap_or_default())?;
            Ok(Authentication::Key(settings.keys[index].id.clone()))
        }
    }
}

/// Parse `local=remote` mapping values.
pub(crate) fn mappings_of(values: &[String]) -> Result<Vec<Mapping>, Failure> {
    values
        .iter()
        .map(|raw| {
            let Some((local, remote)) = raw.split_once('=') else {
                return Err(Failure::Invalid(
                    Key::CliErrorInvalidMapping,
                    Some(raw.clone()),
                ));
            };
            if local.is_empty() || remote.is_empty() {
                return Err(Failure::Invalid(
                    Key::CliErrorInvalidMapping,
                    Some(raw.clone()),
                ));
            }
            Ok(Mapping {
                local: PathBuf::from(local),
                remote: remote.trim_start_matches('/').to_owned(),
            })
        })
        .collect()
}

/// Canonicalize one existing directory.
fn existing_directory(path: PathBuf) -> Result<PathBuf, Failure> {
    std::fs::canonicalize(&path)
        .ok()
        .filter(|path| path.is_dir())
        .ok_or_else(|| Failure::Path(Key::CliErrorPathNotFound, path.display().to_string()))
}

/// Read one secret line from standard input.
fn secret_from_stdin() -> Option<Secret> {
    let mut line = String::new();
    std::io::stdin().read_line(&mut line).ok().map(|_| {
        Secret(Zeroizing::new(
            line.trim_end_matches(['\n', '\r']).to_owned(),
        ))
    })
}

/// Store one secret for one connection draft from standard input.
fn store_connection_secret(connection: &Connection) -> Result<(), Error> {
    let Some(secret) = secret_from_stdin() else {
        return Err(Error::Credentials);
    };
    transport::store_secret(transport::secret_id(connection), &secret)
}

/// Drop nonempty trimmed values.
fn non_empty(value: &str) -> Option<String> {
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_owned())
}

/// Map one failure to its message arguments and exit code.
fn exit(i18n: &I18n, failure: Failure) -> ExitCode {
    let mut arguments = FluentArgs::new();
    let code = match &failure {
        Failure::Invalid(key, Some(value)) => {
            arguments.set("value", value.clone());
            (*key, 2)
        }
        Failure::Invalid(key, None) => (*key, 2),
        Failure::Missing(key, name) => {
            arguments.set("name", name.clone());
            (*key, 1)
        }
        Failure::Path(key, path) => {
            arguments.set("path", path.clone());
            (*key, 1)
        }
        Failure::Transfer(Error::HostUnknown(fingerprint)) => {
            arguments.set("fingerprint", fingerprint.clone());
            (Key::CliDeploymentUnknownHost, 1)
        }
        Failure::Transfer(error) => (deployment_error_key(error), 1),
        Failure::Plain(key) => (*key, 1),
    };
    eprintln!("{}", i18n.format(code.0, &arguments));
    ExitCode::from(code.1)
}

/// Turn one fallible step into an exit code.
fn ensured<T>(i18n: &I18n, result: Result<T, Failure>) -> Result<T, ExitCode> {
    result.map_err(|failure| exit(i18n, failure))
}

/// Build `{name}` message arguments.
fn name_arguments(name: &str) -> FluentArgs<'static> {
    let mut arguments = FluentArgs::new();
    arguments.set("name", name.to_owned());
    arguments
}

/// Mirror `modules::deployment::view::error_key` for console failures.
fn deployment_error_key(error: &Error) -> Key {
    match error {
        Error::Configuration => Key::DeploymentConfigurationError,
        Error::Mapping => Key::DeploymentMappingError,
        Error::Excluded => Key::DeploymentExcludedError,
        Error::Access => Key::DeploymentAccessError,
        Error::Connection => Key::DeploymentConnectionError,
        Error::Credentials => Key::DeploymentCredentialsError,
        Error::HostUnknown(_) => Key::DeploymentUnknownHost,
        Error::HostChanged => Key::DeploymentHostChanged,
        Error::Cancelled => Key::DeploymentCancelled,
        Error::TooLarge => Key::DeploymentTooLarge,
        Error::Binary => Key::DeploymentBinary,
        Error::Overwrite => Key::DeploymentOverwrite,
        Error::Changed => Key::DeploymentChanged,
        Error::Unsupported => Key::DeploymentUnsupported,
    }
}
