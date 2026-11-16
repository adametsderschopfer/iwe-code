//! Resolve SFTP connection details with the system OpenSSH client.

use super::TIMEOUT;
use crate::{domain::deployment::Error, infrastructure::process};
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub(super) struct Config {
    pub host: String,
    pub port: u16,
    pub user: String,
    pub files: Vec<PathBuf>,
    pub agent: Agent,
    pub identities_only: bool,
    pub public_key_auth: bool,
}

#[derive(Debug)]
pub(super) enum Agent {
    Default,
    Disabled,
    Socket(PathBuf),
}

impl Config {
    pub(super) fn load(alias: &str) -> Result<Self, Error> {
        let dirs = directories::BaseDirs::new().ok_or(Error::Configuration)?;
        Self::read(alias, dirs.home_dir())
    }

    pub(super) fn read(alias: &str, home: &Path) -> Result<Self, Error> {
        let file = home.join(".ssh/config");
        let file = file.to_str().ok_or(Error::Configuration)?;
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|_| Error::Configuration)?;
        let output = runtime
            .block_on(process::run_with_timeout(
                "ssh",
                &["-G", "-F", file, "--", alias],
                None,
                TIMEOUT,
            ))
            .map_err(|_| Error::Configuration)?;
        Self::parse(
            std::str::from_utf8(&output).map_err(|_| Error::Configuration)?,
            home,
        )
    }

    pub(super) fn parse(output: &str, home: &Path) -> Result<Self, Error> {
        let mut config = Self {
            host: String::new(),
            port: 0,
            user: String::new(),
            files: Vec::new(),
            agent: Agent::Default,
            identities_only: false,
            public_key_auth: true,
        };
        for (key, value) in output.lines().filter_map(|line| line.split_once(' ')) {
            match (key, value) {
                ("hostname", value) => config.host = value.to_owned(),
                ("port", value) => config.port = value.parse().map_err(|_| Error::Configuration)?,
                ("user", value) => config.user = value.to_owned(),
                ("identityfile", "none") => {}
                ("identityfile", value) => config.files.push(path(value, home)?),
                ("identityagent", "none") => config.agent = Agent::Disabled,
                ("identityagent", "SSH_AUTH_SOCK") => config.agent = Agent::Default,
                ("identityagent", value) => {
                    config.agent = match value.strip_prefix('$') {
                        Some(variable) => std::env::var_os(variable)
                            .map(PathBuf::from)
                            .map_or(Agent::Disabled, Agent::Socket),
                        None => Agent::Socket(path(value, home)?),
                    };
                }
                ("identitiesonly", "yes") => config.identities_only = true,
                ("proxyjump" | "proxycommand" | "certificatefile", value) if value != "none" => {
                    return Err(Error::Unsupported);
                }
                ("pubkeyauthentication", "no" | "false") => config.public_key_auth = false,
                _ => {}
            }
        }
        if config.host.is_empty() || config.port == 0 || config.user.is_empty() {
            return Err(Error::Configuration);
        }
        Ok(config)
    }
}

fn path(value: &str, home: &Path) -> Result<PathBuf, Error> {
    if value.contains(['%', '$']) {
        return Err(Error::Unsupported);
    }
    if let Some(relative) = value.strip_prefix("~/") {
        Ok(home.join(relative))
    } else if Path::new(value).is_absolute() {
        Ok(PathBuf::from(value))
    } else {
        Err(Error::Unsupported)
    }
}
