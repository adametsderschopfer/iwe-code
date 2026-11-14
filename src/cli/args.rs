//! Build the localized command tree and turn parse failures into messages.

use crate::i18n::{FluentArgs, I18n, Key};
use clap::{
    Arg, ArgAction, ArgMatches, Command, Error,
    error::{ContextKind, ContextValue, ErrorKind},
};
use std::{ffi::OsString, path::PathBuf, process::ExitCode};

/// One parsed command-line invocation.
#[derive(Debug)]
pub(crate) enum Invocation {
    Open(Option<PathBuf>),
    List {
        json: bool,
    },
    AddProject {
        directory: PathBuf,
        space: Option<String>,
        json: bool,
    },
    RemoveProject {
        target: String,
        json: bool,
    },
    Deployment(Deployment),
}

/// One parsed deployment subcommand.
#[derive(Debug)]
pub(crate) enum Deployment {
    List {
        json: bool,
    },
    ConnectionAdd(ConnectionDraft),
    ConnectionSet {
        name: String,
        changes: ConnectionChanges,
    },
    ConnectionRemove {
        name: String,
        json: bool,
    },
    KeyAdd {
        name: String,
        path: PathBuf,
        json: bool,
    },
    KeyRemove {
        name: String,
        json: bool,
    },
    ProfileAdd(ProfileDraft),
    ProfileSet {
        name: String,
        changes: ProfileChanges,
    },
    ProfileRemove {
        name: String,
        space: Option<String>,
        json: bool,
    },
    Test {
        profile: String,
        space: Option<String>,
        trust: Option<String>,
        json: bool,
    },
    Upload {
        profile: String,
        path: PathBuf,
        space: Option<String>,
        trust: Option<String>,
        json: bool,
    },
    Download {
        profile: String,
        remote: String,
        overwrite: bool,
        space: Option<String>,
        trust: Option<String>,
        json: bool,
    },
    ListRemote {
        profile: String,
        directory: Option<String>,
        space: Option<String>,
        trust: Option<String>,
        json: bool,
    },
}

/// New connection values gathered by `deployment connection add`.
#[derive(Debug)]
pub(crate) struct ConnectionDraft {
    pub(crate) name: String,
    pub(crate) host: String,
    pub(crate) username: String,
    pub(crate) protocol: Option<String>,
    pub(crate) port: Option<String>,
    pub(crate) auth: Option<String>,
    pub(crate) key: Option<String>,
    pub(crate) password_stdin: bool,
    pub(crate) json: bool,
}

/// Changed connection values gathered by `deployment connection set`.
#[derive(Debug)]
pub(crate) struct ConnectionChanges {
    pub(crate) host: Option<String>,
    pub(crate) username: Option<String>,
    pub(crate) protocol: Option<String>,
    pub(crate) port: Option<String>,
    pub(crate) auth: Option<String>,
    pub(crate) key: Option<String>,
    pub(crate) password_stdin: bool,
    pub(crate) json: bool,
}

/// New profile values gathered by `deployment profile add`.
#[derive(Debug)]
pub(crate) struct ProfileDraft {
    pub(crate) name: String,
    pub(crate) project: PathBuf,
    pub(crate) connection: String,
    pub(crate) root: String,
    pub(crate) mappings: Vec<String>,
    pub(crate) excluded: Vec<String>,
    pub(crate) default: Option<bool>,
    pub(crate) space: Option<String>,
    pub(crate) json: bool,
}

/// Changed profile values gathered by `deployment profile set`.
#[derive(Debug)]
pub(crate) struct ProfileChanges {
    pub(crate) project: Option<PathBuf>,
    pub(crate) connection: Option<String>,
    pub(crate) root: Option<String>,
    pub(crate) mappings: Option<Vec<String>>,
    pub(crate) excluded: Option<Vec<String>>,
    pub(crate) clear_excluded: bool,
    pub(crate) default: Option<bool>,
    pub(crate) space: Option<String>,
    pub(crate) json: bool,
}

/// Parse the process arguments into one invocation.
pub(crate) fn parse(i18n: &I18n) -> Result<Invocation, Error> {
    let mut command = command(i18n);
    let arguments: Vec<OsString> = std::env::args_os().collect();
    command
        .try_get_matches_from_mut(arguments)
        .map(|matches| invocation(&matches))
}

/// Parse explicit arguments. Used by tests.
#[cfg(test)]
pub(crate) fn parse_from(i18n: &I18n, arguments: &[&str]) -> Result<Invocation, Error> {
    command(i18n)
        .try_get_matches_from(arguments.to_vec())
        .map(|matches| invocation(&matches))
}

/// Build the localized command tree.
pub(crate) fn command(i18n: &I18n) -> Command {
    // clap requires 'static section headings, so leak the localized words.
    // A command-line process builds the tree once and exits.
    let arguments: &'static str = i18n.text(Key::CliHelpArguments).leak();
    let options: &'static str = i18n.text(Key::CliHelpOptions).leak();
    let commands: &'static str = i18n.text(Key::CliHelpCommands).leak();
    Command::new("iwe")
        .version(env!("CARGO_PKG_VERSION"))
        .about(i18n.text(Key::CliAbout))
        .help_template(template(i18n))
        .disable_help_flag(true)
        .disable_version_flag(true)
        .disable_help_subcommand(true)
        .subcommand_help_heading(commands)
        .subcommand(open_command(i18n, arguments, options))
        .subcommand(workspace_command(i18n, arguments, options))
        .subcommand(deployment_command(i18n, arguments, options))
        .next_help_heading(options)
        .arg(flag_help(i18n))
        .arg(flag_version(i18n))
}

/// Build the help template with localized section labels.
fn template(i18n: &I18n) -> String {
    format!(
        "{{about-with-newline}}\n{} {{usage}}\n\n{{all-args}}",
        i18n.text(Key::CliHelpUsage)
    )
}

fn open_command(i18n: &I18n, arguments: &'static str, options: &'static str) -> Command {
    Command::new("open")
        .about(i18n.text(Key::CliOpenAbout))
        .help_template(template(i18n))
        .disable_help_flag(true)
        .disable_version_flag(true)
        .next_help_heading(arguments)
        .arg(
            Arg::new("directory")
                .value_name("directory")
                .value_parser(clap::value_parser!(OsString))
                .help(i18n.text(Key::CliOpenDirectoryHelp)),
        )
        .next_help_heading(options)
        .arg(flag_help(i18n))
}

fn workspace_command(i18n: &I18n, arguments: &'static str, options: &'static str) -> Command {
    let commands: &'static str = i18n.text(Key::CliHelpCommands).leak();
    Command::new("workspace")
        .about(i18n.text(Key::CliWorkspaceAbout))
        .help_template(template(i18n))
        .disable_help_flag(true)
        .disable_version_flag(true)
        .subcommand_required(true)
        .arg_required_else_help(true)
        .subcommand_help_heading(commands)
        .subcommand(list_command(i18n, options))
        .subcommand(add_project_command(i18n, arguments, options))
        .subcommand(remove_project_command(i18n, arguments, options))
        .next_help_heading(options)
        .arg(flag_help(i18n))
}

fn list_command(i18n: &I18n, options: &'static str) -> Command {
    Command::new("list")
        .about(i18n.text(Key::CliWorkspaceListAbout))
        .help_template(template(i18n))
        .disable_help_flag(true)
        .disable_version_flag(true)
        .next_help_heading(options)
        .arg(flag_json(i18n))
        .arg(flag_help(i18n))
}

fn add_project_command(i18n: &I18n, arguments: &'static str, options: &'static str) -> Command {
    Command::new("add-project")
        .about(i18n.text(Key::CliWorkspaceAddProjectAbout))
        .help_template(template(i18n))
        .disable_help_flag(true)
        .disable_version_flag(true)
        .next_help_heading(arguments)
        .arg(
            Arg::new("directory")
                .value_name("directory")
                .value_parser(clap::value_parser!(OsString))
                .required(true)
                .help(i18n.text(Key::CliAddDirectoryHelp)),
        )
        .next_help_heading(options)
        .arg(flag_space(i18n))
        .arg(flag_json(i18n))
        .arg(flag_help(i18n))
}

fn remove_project_command(i18n: &I18n, arguments: &'static str, options: &'static str) -> Command {
    Command::new("remove-project")
        .about(i18n.text(Key::CliWorkspaceRemoveProjectAbout))
        .help_template(template(i18n))
        .disable_help_flag(true)
        .disable_version_flag(true)
        .next_help_heading(arguments)
        .arg(
            Arg::new("project")
                .value_name("project")
                .required(true)
                .help(i18n.text(Key::CliRemoveTargetHelp)),
        )
        .next_help_heading(options)
        .arg(flag_json(i18n))
        .arg(flag_help(i18n))
}

fn deployment_command(i18n: &I18n, arguments: &'static str, options: &'static str) -> Command {
    let commands: &'static str = i18n.text(Key::CliHelpCommands).leak();
    Command::new("deployment")
        .about(i18n.text(Key::CliDeploymentAbout))
        .help_template(template(i18n))
        .disable_help_flag(true)
        .disable_version_flag(true)
        .subcommand_required(true)
        .arg_required_else_help(true)
        .subcommand_help_heading(commands)
        .subcommand(deployment_list_command(i18n, options))
        .subcommand(deployment_connection_command(
            i18n, arguments, options, commands,
        ))
        .subcommand(deployment_key_command(i18n, arguments, options, commands))
        .subcommand(deployment_profile_command(
            i18n, arguments, options, commands,
        ))
        .subcommand(deployment_test_command(i18n, arguments, options))
        .subcommand(deployment_upload_command(i18n, arguments, options))
        .subcommand(deployment_download_command(i18n, arguments, options))
        .subcommand(deployment_list_remote_command(i18n, arguments, options))
        .next_help_heading(options)
        .arg(flag_help(i18n))
}

fn deployment_list_command(i18n: &I18n, options: &'static str) -> Command {
    Command::new("list")
        .about(i18n.text(Key::CliDeploymentListAbout))
        .help_template(template(i18n))
        .disable_help_flag(true)
        .disable_version_flag(true)
        .next_help_heading(options)
        .arg(flag_json(i18n))
        .arg(flag_help(i18n))
}

fn deployment_connection_command(
    i18n: &I18n,
    arguments: &'static str,
    options: &'static str,
    commands: &'static str,
) -> Command {
    Command::new("connection")
        .about(i18n.text(Key::CliDeploymentConnectionAbout))
        .help_template(template(i18n))
        .disable_help_flag(true)
        .disable_version_flag(true)
        .subcommand_required(true)
        .arg_required_else_help(true)
        .subcommand_help_heading(commands)
        .subcommand(connection_add_command(i18n, arguments, options))
        .subcommand(connection_set_command(i18n, arguments, options))
        .subcommand(connection_remove_command(i18n, arguments, options))
        .next_help_heading(options)
        .arg(flag_help(i18n))
}

fn connection_add_command(i18n: &I18n, arguments: &'static str, options: &'static str) -> Command {
    Command::new("add")
        .about(i18n.text(Key::CliDeploymentConnectionAddAbout))
        .help_template(template(i18n))
        .disable_help_flag(true)
        .disable_version_flag(true)
        .next_help_heading(arguments)
        .arg(position_name(i18n))
        .next_help_heading(options)
        .arg(option_host(i18n).required(true))
        .arg(option_username(i18n).required(true))
        .arg(option_protocol(i18n))
        .arg(option_port(i18n))
        .arg(option_auth(i18n))
        .arg(option_key(i18n))
        .arg(flag_password_stdin(i18n))
        .arg(flag_json(i18n))
        .arg(flag_help(i18n))
}

fn connection_set_command(i18n: &I18n, arguments: &'static str, options: &'static str) -> Command {
    Command::new("set")
        .about(i18n.text(Key::CliDeploymentConnectionSetAbout))
        .help_template(template(i18n))
        .disable_help_flag(true)
        .disable_version_flag(true)
        .next_help_heading(arguments)
        .arg(position_name(i18n))
        .next_help_heading(options)
        .arg(option_host(i18n))
        .arg(option_username(i18n))
        .arg(option_protocol(i18n))
        .arg(option_port(i18n))
        .arg(option_auth(i18n))
        .arg(option_key(i18n))
        .arg(flag_password_stdin(i18n))
        .arg(flag_json(i18n))
        .arg(flag_help(i18n))
}

fn connection_remove_command(
    i18n: &I18n,
    arguments: &'static str,
    options: &'static str,
) -> Command {
    Command::new("remove")
        .about(i18n.text(Key::CliDeploymentConnectionRemoveAbout))
        .help_template(template(i18n))
        .disable_help_flag(true)
        .disable_version_flag(true)
        .next_help_heading(arguments)
        .arg(position_name(i18n))
        .next_help_heading(options)
        .arg(flag_json(i18n))
        .arg(flag_help(i18n))
}

fn deployment_key_command(
    i18n: &I18n,
    arguments: &'static str,
    options: &'static str,
    commands: &'static str,
) -> Command {
    Command::new("key")
        .about(i18n.text(Key::CliDeploymentKeyAbout))
        .help_template(template(i18n))
        .disable_help_flag(true)
        .disable_version_flag(true)
        .subcommand_required(true)
        .arg_required_else_help(true)
        .subcommand_help_heading(commands)
        .subcommand(key_add_command(i18n, arguments, options))
        .subcommand(key_remove_command(i18n, arguments, options))
        .next_help_heading(options)
        .arg(flag_help(i18n))
}

fn key_add_command(i18n: &I18n, arguments: &'static str, options: &'static str) -> Command {
    Command::new("add")
        .about(i18n.text(Key::CliDeploymentKeyAddAbout))
        .help_template(template(i18n))
        .disable_help_flag(true)
        .disable_version_flag(true)
        .next_help_heading(arguments)
        .arg(position_name(i18n))
        .arg(
            Arg::new("path")
                .value_name("path")
                .value_parser(clap::value_parser!(OsString))
                .required(true)
                .help(i18n.text(Key::CliDeploymentKeyPathArgHelp)),
        )
        .next_help_heading(options)
        .arg(flag_json(i18n))
        .arg(flag_help(i18n))
}

fn key_remove_command(i18n: &I18n, arguments: &'static str, options: &'static str) -> Command {
    Command::new("remove")
        .about(i18n.text(Key::CliDeploymentKeyRemoveAbout))
        .help_template(template(i18n))
        .disable_help_flag(true)
        .disable_version_flag(true)
        .next_help_heading(arguments)
        .arg(position_name(i18n))
        .next_help_heading(options)
        .arg(flag_json(i18n))
        .arg(flag_help(i18n))
}

fn deployment_profile_command(
    i18n: &I18n,
    arguments: &'static str,
    options: &'static str,
    commands: &'static str,
) -> Command {
    Command::new("profile")
        .about(i18n.text(Key::CliDeploymentProfileAbout))
        .help_template(template(i18n))
        .disable_help_flag(true)
        .disable_version_flag(true)
        .subcommand_required(true)
        .arg_required_else_help(true)
        .subcommand_help_heading(commands)
        .subcommand(profile_add_command(i18n, arguments, options))
        .subcommand(profile_set_command(i18n, arguments, options))
        .subcommand(profile_remove_command(i18n, arguments, options))
        .next_help_heading(options)
        .arg(flag_help(i18n))
}

fn profile_add_command(i18n: &I18n, arguments: &'static str, options: &'static str) -> Command {
    Command::new("add")
        .about(i18n.text(Key::CliDeploymentProfileAddAbout))
        .help_template(template(i18n))
        .disable_help_flag(true)
        .disable_version_flag(true)
        .next_help_heading(arguments)
        .arg(position_name(i18n))
        .next_help_heading(options)
        .arg(option_project(i18n).required(true))
        .arg(option_connection(i18n).required(true))
        .arg(option_root(i18n).required(true))
        .arg(option_mapping(i18n))
        .arg(option_exclude(i18n))
        .arg(flag_default(i18n))
        .arg(flag_space(i18n))
        .arg(flag_json(i18n))
        .arg(flag_help(i18n))
}

fn profile_set_command(i18n: &I18n, arguments: &'static str, options: &'static str) -> Command {
    Command::new("set")
        .about(i18n.text(Key::CliDeploymentProfileSetAbout))
        .help_template(template(i18n))
        .disable_help_flag(true)
        .disable_version_flag(true)
        .next_help_heading(arguments)
        .arg(position_name(i18n))
        .next_help_heading(options)
        .arg(option_project(i18n))
        .arg(option_connection(i18n))
        .arg(option_root(i18n))
        .arg(option_mapping(i18n))
        .arg(option_exclude(i18n))
        .arg(flag_clear_excluded(i18n))
        .arg(flag_default(i18n))
        .arg(flag_space(i18n))
        .arg(flag_json(i18n))
        .arg(flag_help(i18n))
}

fn profile_remove_command(i18n: &I18n, arguments: &'static str, options: &'static str) -> Command {
    Command::new("remove")
        .about(i18n.text(Key::CliDeploymentProfileRemoveAbout))
        .help_template(template(i18n))
        .disable_help_flag(true)
        .disable_version_flag(true)
        .next_help_heading(arguments)
        .arg(position_name(i18n))
        .next_help_heading(options)
        .arg(flag_space(i18n))
        .arg(flag_json(i18n))
        .arg(flag_help(i18n))
}

fn deployment_test_command(i18n: &I18n, arguments: &'static str, options: &'static str) -> Command {
    Command::new("test")
        .about(i18n.text(Key::CliDeploymentTestAbout))
        .help_template(template(i18n))
        .disable_help_flag(true)
        .disable_version_flag(true)
        .next_help_heading(arguments)
        .arg(position_profile(i18n))
        .next_help_heading(options)
        .arg(flag_space(i18n))
        .arg(flag_trust_host_key(i18n))
        .arg(flag_json(i18n))
        .arg(flag_help(i18n))
}

fn deployment_upload_command(
    i18n: &I18n,
    arguments: &'static str,
    options: &'static str,
) -> Command {
    Command::new("upload")
        .about(i18n.text(Key::CliDeploymentUploadAbout))
        .help_template(template(i18n))
        .disable_help_flag(true)
        .disable_version_flag(true)
        .next_help_heading(arguments)
        .arg(position_profile(i18n))
        .arg(
            Arg::new("path")
                .value_name("path")
                .value_parser(clap::value_parser!(OsString))
                .required(true)
                .help(i18n.text(Key::CliDeploymentPathArgHelp)),
        )
        .next_help_heading(options)
        .arg(flag_space(i18n))
        .arg(flag_trust_host_key(i18n))
        .arg(flag_json(i18n))
        .arg(flag_help(i18n))
}

fn deployment_download_command(
    i18n: &I18n,
    arguments: &'static str,
    options: &'static str,
) -> Command {
    Command::new("download")
        .about(i18n.text(Key::CliDeploymentDownloadAbout))
        .help_template(template(i18n))
        .disable_help_flag(true)
        .disable_version_flag(true)
        .next_help_heading(arguments)
        .arg(position_profile(i18n))
        .arg(
            Arg::new("remote")
                .value_name("remote")
                .required(true)
                .help(i18n.text(Key::CliDeploymentRemoteArgHelp)),
        )
        .next_help_heading(options)
        .arg(flag_overwrite(i18n))
        .arg(flag_space(i18n))
        .arg(flag_trust_host_key(i18n))
        .arg(flag_json(i18n))
        .arg(flag_help(i18n))
}

fn deployment_list_remote_command(
    i18n: &I18n,
    arguments: &'static str,
    options: &'static str,
) -> Command {
    Command::new("list-remote")
        .about(i18n.text(Key::CliDeploymentListRemoteAbout))
        .help_template(template(i18n))
        .disable_help_flag(true)
        .disable_version_flag(true)
        .next_help_heading(arguments)
        .arg(position_profile(i18n))
        .arg(
            Arg::new("directory")
                .value_name("directory")
                .required(false)
                .help(i18n.text(Key::CliDeploymentDirectoryArgHelp)),
        )
        .next_help_heading(options)
        .arg(flag_space(i18n))
        .arg(flag_trust_host_key(i18n))
        .arg(flag_json(i18n))
        .arg(flag_help(i18n))
}

fn position_name(i18n: &I18n) -> Arg {
    Arg::new("name")
        .value_name("name")
        .required(true)
        .help(i18n.text(Key::CliDeploymentNameArgHelp))
}

fn position_profile(i18n: &I18n) -> Arg {
    Arg::new("profile")
        .value_name("profile")
        .required(true)
        .help(i18n.text(Key::CliDeploymentProfileArgHelp))
}

fn option_host(i18n: &I18n) -> Arg {
    Arg::new("host")
        .long("host")
        .value_name("host")
        .help(i18n.text(Key::CliDeploymentHostHelp))
}

fn option_username(i18n: &I18n) -> Arg {
    Arg::new("username")
        .long("username")
        .value_name("name")
        .help(i18n.text(Key::CliDeploymentUsernameHelp))
}

fn option_protocol(i18n: &I18n) -> Arg {
    Arg::new("protocol")
        .long("protocol")
        .value_name("protocol")
        .help(i18n.text(Key::CliDeploymentProtocolHelp))
}

fn option_port(i18n: &I18n) -> Arg {
    Arg::new("port")
        .long("port")
        .value_name("port")
        .help(i18n.text(Key::CliDeploymentPortHelp))
}

fn option_auth(i18n: &I18n) -> Arg {
    Arg::new("auth")
        .long("auth")
        .value_name("auth")
        .help(i18n.text(Key::CliDeploymentAuthHelp))
}

fn option_key(i18n: &I18n) -> Arg {
    Arg::new("key")
        .long("key")
        .value_name("key")
        .help(i18n.text(Key::CliDeploymentKeyHelp))
}

fn option_project(i18n: &I18n) -> Arg {
    Arg::new("project")
        .long("project")
        .value_name("directory")
        .value_parser(clap::value_parser!(OsString))
        .help(i18n.text(Key::CliDeploymentProjectHelp))
}

fn option_connection(i18n: &I18n) -> Arg {
    Arg::new("connection")
        .long("connection")
        .value_name("connection")
        .help(i18n.text(Key::CliDeploymentConnectionHelp))
}

fn option_root(i18n: &I18n) -> Arg {
    Arg::new("root")
        .long("root")
        .value_name("root")
        .help(i18n.text(Key::CliDeploymentRootHelp))
}

fn option_mapping(i18n: &I18n) -> Arg {
    Arg::new("mapping")
        .long("mapping")
        .value_name("local=remote")
        .action(ArgAction::Append)
        .help(i18n.text(Key::CliDeploymentMappingHelp))
}

fn option_exclude(i18n: &I18n) -> Arg {
    Arg::new("exclude")
        .long("exclude")
        .value_name("path")
        .action(ArgAction::Append)
        .help(i18n.text(Key::CliDeploymentExcludeHelp))
}

fn flag_clear_excluded(i18n: &I18n) -> Arg {
    Arg::new("clear-excluded")
        .long("clear-excluded")
        .action(ArgAction::SetTrue)
        .help(i18n.text(Key::CliDeploymentClearExcludedHelp))
}

fn flag_default(i18n: &I18n) -> Arg {
    Arg::new("default")
        .long("default")
        .action(ArgAction::Set)
        .num_args(0..=1)
        .default_missing_value("true")
        .value_parser(clap::value_parser!(bool))
        .help(i18n.text(Key::CliDeploymentDefaultHelp))
}

fn flag_overwrite(i18n: &I18n) -> Arg {
    Arg::new("overwrite")
        .long("overwrite")
        .action(ArgAction::SetTrue)
        .help(i18n.text(Key::CliDeploymentOverwriteHelp))
}

fn flag_password_stdin(i18n: &I18n) -> Arg {
    Arg::new("password-stdin")
        .long("password-stdin")
        .action(ArgAction::SetTrue)
        .help(i18n.text(Key::CliDeploymentPasswordStdinHelp))
}

fn flag_trust_host_key(i18n: &I18n) -> Arg {
    Arg::new("trust-host-key")
        .long("trust-host-key")
        .value_name("fingerprint")
        .help(i18n.text(Key::CliDeploymentTrustHostKeyHelp))
}

fn flag_help(i18n: &I18n) -> Arg {
    Arg::new("help")
        .short('h')
        .long("help")
        .action(ArgAction::Help)
        .help(i18n.text(Key::CliHelpFlag))
}

fn flag_version(i18n: &I18n) -> Arg {
    Arg::new("version")
        .short('V')
        .long("version")
        .action(ArgAction::Version)
        .help(i18n.text(Key::CliVersionFlag))
}

fn flag_json(i18n: &I18n) -> Arg {
    Arg::new("json")
        .long("json")
        .action(ArgAction::SetTrue)
        .help(i18n.text(Key::CliJsonFlagHelp))
}

fn flag_space(i18n: &I18n) -> Arg {
    Arg::new("space")
        .long("space")
        .value_name("name")
        .help(i18n.text(Key::CliSpaceOptionHelp))
}

/// Convert parsed matches into one invocation.
fn invocation(matches: &ArgMatches) -> Invocation {
    match matches.subcommand() {
        Some(("open", matches)) => {
            Invocation::Open(matches.get_one::<OsString>("directory").map(PathBuf::from))
        }
        Some(("workspace", matches)) => workspace_invocation(matches),
        Some(("deployment", matches)) => Invocation::Deployment(deployment_invocation(matches)),
        // Without a subcommand the interface starts with its own arguments.
        _ => Invocation::Open(None),
    }
}

fn workspace_invocation(matches: &ArgMatches) -> Invocation {
    match matches.subcommand() {
        Some(("list", matches)) => Invocation::List {
            json: matches.get_flag("json"),
        },
        Some(("add-project", matches)) => Invocation::AddProject {
            directory: matches
                .get_one::<OsString>("directory")
                .map(PathBuf::from)
                .unwrap_or_default(),
            space: matches.get_one::<String>("space").cloned(),
            json: matches.get_flag("json"),
        },
        Some(("remove-project", matches)) => Invocation::RemoveProject {
            target: matches
                .get_one::<String>("project")
                .cloned()
                .unwrap_or_default(),
            json: matches.get_flag("json"),
        },
        // `workspace` requires a subcommand, so this arm never matches.
        _ => Invocation::List { json: false },
    }
}

/// Convert parsed deployment matches into one deployment subcommand.
fn deployment_invocation(matches: &ArgMatches) -> Deployment {
    match matches.subcommand() {
        Some(("list", matches)) => Deployment::List {
            json: matches.get_flag("json"),
        },
        Some(("connection", matches)) => connection_invocation(matches),
        Some(("key", matches)) => key_invocation(matches),
        Some(("profile", matches)) => profile_invocation(matches),
        Some(("test", matches)) => Deployment::Test {
            profile: text_of(matches, "profile"),
            space: text_option_of(matches, "space"),
            trust: text_option_of(matches, "trust-host-key"),
            json: matches.get_flag("json"),
        },
        Some(("upload", matches)) => Deployment::Upload {
            profile: text_of(matches, "profile"),
            path: matches
                .get_one::<OsString>("path")
                .map(PathBuf::from)
                .unwrap_or_default(),
            space: text_option_of(matches, "space"),
            trust: text_option_of(matches, "trust-host-key"),
            json: matches.get_flag("json"),
        },
        Some(("download", matches)) => Deployment::Download {
            profile: text_of(matches, "profile"),
            remote: text_of(matches, "remote"),
            overwrite: matches.get_flag("overwrite"),
            space: text_option_of(matches, "space"),
            trust: text_option_of(matches, "trust-host-key"),
            json: matches.get_flag("json"),
        },
        Some(("list-remote", matches)) => Deployment::ListRemote {
            profile: text_of(matches, "profile"),
            directory: text_option_of(matches, "directory"),
            space: text_option_of(matches, "space"),
            trust: text_option_of(matches, "trust-host-key"),
            json: matches.get_flag("json"),
        },
        // `deployment` requires a subcommand, so this arm never matches.
        _ => Deployment::List { json: false },
    }
}

fn connection_invocation(matches: &ArgMatches) -> Deployment {
    match matches.subcommand() {
        Some(("add", matches)) => Deployment::ConnectionAdd(ConnectionDraft {
            name: text_of(matches, "name"),
            host: text_of(matches, "host"),
            username: text_of(matches, "username"),
            protocol: text_option_of(matches, "protocol"),
            port: text_option_of(matches, "port"),
            auth: text_option_of(matches, "auth"),
            key: text_option_of(matches, "key"),
            password_stdin: matches.get_flag("password-stdin"),
            json: matches.get_flag("json"),
        }),
        Some(("set", matches)) => Deployment::ConnectionSet {
            name: text_of(matches, "name"),
            changes: ConnectionChanges {
                host: text_option_of(matches, "host"),
                username: text_option_of(matches, "username"),
                protocol: text_option_of(matches, "protocol"),
                port: text_option_of(matches, "port"),
                auth: text_option_of(matches, "auth"),
                key: text_option_of(matches, "key"),
                password_stdin: matches.get_flag("password-stdin"),
                json: matches.get_flag("json"),
            },
        },
        Some(("remove", matches)) => Deployment::ConnectionRemove {
            name: text_of(matches, "name"),
            json: matches.get_flag("json"),
        },
        // `connection` requires a subcommand, so this arm never matches.
        _ => Deployment::List { json: false },
    }
}

fn key_invocation(matches: &ArgMatches) -> Deployment {
    match matches.subcommand() {
        Some(("add", matches)) => Deployment::KeyAdd {
            name: text_of(matches, "name"),
            path: matches
                .get_one::<OsString>("path")
                .map(PathBuf::from)
                .unwrap_or_default(),
            json: matches.get_flag("json"),
        },
        Some(("remove", matches)) => Deployment::KeyRemove {
            name: text_of(matches, "name"),
            json: matches.get_flag("json"),
        },
        // `key` requires a subcommand, so this arm never matches.
        _ => Deployment::List { json: false },
    }
}

fn profile_invocation(matches: &ArgMatches) -> Deployment {
    match matches.subcommand() {
        Some(("add", matches)) => Deployment::ProfileAdd(ProfileDraft {
            name: text_of(matches, "name"),
            project: matches
                .get_one::<OsString>("project")
                .map(PathBuf::from)
                .unwrap_or_default(),
            connection: text_of(matches, "connection"),
            root: text_of(matches, "root"),
            mappings: text_list_of(matches, "mapping"),
            excluded: text_list_of(matches, "exclude"),
            default: bool_option_of(matches, "default"),
            space: text_option_of(matches, "space"),
            json: matches.get_flag("json"),
        }),
        Some(("set", matches)) => Deployment::ProfileSet {
            name: text_of(matches, "name"),
            changes: ProfileChanges {
                project: matches.get_one::<OsString>("project").map(PathBuf::from),
                connection: text_option_of(matches, "connection"),
                root: text_option_of(matches, "root"),
                mappings: provided_list_of(matches, "mapping"),
                excluded: provided_list_of(matches, "exclude"),
                clear_excluded: matches.get_flag("clear-excluded"),
                default: bool_option_of(matches, "default"),
                space: text_option_of(matches, "space"),
                json: matches.get_flag("json"),
            },
        },
        Some(("remove", matches)) => Deployment::ProfileRemove {
            name: text_of(matches, "name"),
            space: text_option_of(matches, "space"),
            json: matches.get_flag("json"),
        },
        // `profile` requires a subcommand, so this arm never matches.
        _ => Deployment::List { json: false },
    }
}

fn text_of(matches: &ArgMatches, name: &str) -> String {
    matches.get_one::<String>(name).cloned().unwrap_or_default()
}

fn text_option_of(matches: &ArgMatches, name: &str) -> Option<String> {
    matches.get_one::<String>(name).cloned()
}

fn text_list_of(matches: &ArgMatches, name: &str) -> Vec<String> {
    matches
        .get_many::<String>(name)
        .map(|values| values.cloned().collect())
        .unwrap_or_default()
}

/// Distinguish an absent repeatable option from an empty explicit list.
fn provided_list_of(matches: &ArgMatches, name: &str) -> Option<Vec<String>> {
    matches.get_many::<String>(name).map(|values| {
        let values: Vec<String> = values.cloned().collect();
        values
    })
}

fn bool_option_of(matches: &ArgMatches, name: &str) -> Option<bool> {
    matches.get_one::<bool>(name).copied()
}

/// Render a parse failure as a localized message and exit code.
pub(crate) fn report(i18n: &I18n, error: &Error) -> ExitCode {
    match error.kind() {
        ErrorKind::DisplayHelp | ErrorKind::DisplayVersion => {
            let _ = error.print();
            ExitCode::SUCCESS
        }
        ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand => {
            let _ = error.print();
            ExitCode::from(2)
        }
        ErrorKind::InvalidSubcommand => usage(i18n, Key::CliErrorUnknownCommand, error),
        ErrorKind::UnknownArgument => usage(i18n, Key::CliErrorUnknownArgument, error),
        ErrorKind::MissingRequiredArgument | ErrorKind::MissingSubcommand => {
            usage(i18n, Key::CliErrorMissingArgument, error)
        }
        _ => {
            eprintln!("{}", i18n.text(Key::CliErrorUnexpected));
            ExitCode::from(2)
        }
    }
}

/// Print one localized usage error naming the offending value.
fn usage(i18n: &I18n, key: Key, error: &Error) -> ExitCode {
    if let Some(value) = invalid_value(error) {
        let mut arguments = FluentArgs::new();
        arguments.set("value", value);
        eprintln!("{}", i18n.format(key, &arguments));
    } else {
        eprintln!("{}", i18n.text(Key::CliErrorUnexpected));
    }
    ExitCode::from(2)
}

fn invalid_value(error: &Error) -> Option<String> {
    for kind in [
        ContextKind::InvalidArg,
        ContextKind::InvalidSubcommand,
        ContextKind::InvalidValue,
    ] {
        match error.get(kind) {
            Some(ContextValue::String(value)) => return Some(value.clone()),
            Some(ContextValue::Strings(values)) => {
                if let Some(value) = values.first() {
                    return Some(value.clone());
                }
            }
            _ => {}
        }
    }
    None
}
