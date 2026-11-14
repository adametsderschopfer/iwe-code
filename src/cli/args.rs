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
