use super::{
    Entry, Progress, Secret, Transport, child_path, copy, socket,
    ssh_config::{Agent, Config},
};
use crate::domain::deployment::{Authentication, Connection, Connections, Error};
use base64::Engine;
use sha2::{Digest, Sha256};
use std::{
    io::{Read, Write},
    path::Path,
};

pub(super) struct Sftp {
    _session: ssh2::Session,
    files: ssh2::Sftp,
}

impl Sftp {
    pub(super) fn connect(
        connection: &Connection,
        settings: &Connections,
        secret: &Secret,
    ) -> Result<Self, Error> {
        Self::connect_with_agent(connection, settings, secret, None)
    }

    pub(super) fn connect_with_agent(
        connection: &Connection,
        settings: &Connections,
        secret: &Secret,
        agent_path: Option<&Path>,
    ) -> Result<Self, Error> {
        let config = if connection.uses_ssh_config() {
            Some(Config::load(&connection.host)?)
        } else {
            None
        };
        Self::connect_resolved(connection, settings, secret, agent_path, config.as_ref())
    }

    pub(super) fn connect_resolved(
        connection: &Connection,
        settings: &Connections,
        secret: &Secret,
        agent_path: Option<&Path>,
        config: Option<&Config>,
    ) -> Result<Self, Error> {
        let host = config.map_or(connection.host.as_str(), |config| config.host.as_str());
        let port = if connection.port == 0 {
            config.ok_or(Error::Configuration)?.port
        } else {
            connection.port
        };
        let user = if connection.username.is_empty() {
            config.ok_or(Error::Configuration)?.user.as_str()
        } else {
            connection.username.as_str()
        };
        let mut session = ssh2::Session::new().map_err(|_| Error::Connection)?;
        session.set_tcp_stream(socket(host, port)?);
        session.set_timeout(super::TIMEOUT.as_millis() as u32);
        session.handshake().map_err(|_| Error::Connection)?;
        let (key, _) = session.host_key().ok_or(Error::Connection)?;
        let fingerprint = format!(
            "SHA256:{}",
            base64::engine::general_purpose::STANDARD_NO_PAD.encode(Sha256::digest(key))
        );
        match &connection.trusted_host_key {
            None => return Err(Error::HostUnknown(fingerprint)),
            Some(known) if *known != fingerprint => return Err(Error::HostChanged),
            _ => {}
        }
        match &connection.authentication {
            Authentication::Password => session
                .userauth_password(user, &secret.0)
                .map_err(|_| Error::Credentials)?,
            Authentication::Agent => authenticate_agent(&session, user, agent_path, None)?,
            Authentication::Key(id) => {
                let key = settings
                    .keys
                    .iter()
                    .find(|key| key.id == *id)
                    .ok_or(Error::Configuration)?;
                session
                    .userauth_pubkey_file(
                        user,
                        None,
                        &key.path,
                        (!secret.0.is_empty()).then_some(secret.0.as_str()),
                    )
                    .map_err(|_| Error::Credentials)?;
            }
            Authentication::SshConfig => {
                authenticate_config(&session, user, config.ok_or(Error::Configuration)?)?
            }
        }
        let files = session.sftp().map_err(|_| Error::Connection)?;
        Ok(Self {
            _session: session,
            files,
        })
    }
}

fn authenticate_agent(
    session: &ssh2::Session,
    user: &str,
    path: Option<&Path>,
    allowed: Option<&[Vec<u8>]>,
) -> Result<(), Error> {
    let mut agent = session.agent().map_err(|_| Error::Credentials)?;
    if let Some(path) = path {
        agent
            .set_identity_path(path)
            .map_err(|_| Error::Credentials)?;
    }
    agent.connect().map_err(|_| Error::Credentials)?;
    agent.list_identities().map_err(|_| Error::Credentials)?;
    let identities = agent.identities().map_err(|_| Error::Credentials)?;
    identities
        .iter()
        .any(|identity| {
            allowed.is_none_or(|keys| keys.iter().any(|key| key == identity.blob()))
                && agent.userauth(user, identity).is_ok()
        })
        .then_some(())
        .ok_or(Error::Credentials)
}

fn authenticate_config(session: &ssh2::Session, user: &str, config: &Config) -> Result<(), Error> {
    if !config.public_key_auth {
        return Err(Error::Unsupported);
    }
    if !matches!(config.agent, Agent::Disabled) {
        let path = match &config.agent {
            Agent::Socket(path) => Some(path.as_path()),
            _ => None,
        };
        let allowed = config.identities_only.then(|| {
            config
                .files
                .iter()
                .filter_map(|path| {
                    let mut public_path = path.as_os_str().to_os_string();
                    public_path.push(".pub");
                    let mut text = String::new();
                    std::fs::File::open(public_path)
                        .ok()?
                        .take(64 * 1024)
                        .read_to_string(&mut text)
                        .ok()?;
                    base64::engine::general_purpose::STANDARD
                        .decode(text.split_whitespace().nth(1)?)
                        .ok()
                })
                .collect::<Vec<_>>()
        });
        if authenticate_agent(session, user, path, allowed.as_deref()).is_ok() {
            return Ok(());
        }
    }
    config
        .files
        .iter()
        .any(|path| session.userauth_pubkey_file(user, None, path, None).is_ok())
        .then_some(())
        .ok_or(Error::Credentials)
}

fn entry(path: String, stat: ssh2::FileStat) -> Entry {
    Entry {
        name: path.rsplit('/').next().unwrap_or("").to_owned(),
        path,
        directory: stat.is_dir(),
        symlink: !stat.is_file() && !stat.is_dir(),
        size: stat.size.unwrap_or(0),
    }
}

impl Transport for Sftp {
    fn initial_directory(&mut self) -> Result<String, Error> {
        let directory = self
            .files
            .realpath(Path::new("."))
            .map_err(|_| Error::Access)?;
        crate::domain::deployment::remote_path(directory.to_str().ok_or(Error::Unsupported)?)
    }

    fn list(&mut self, path: &str) -> Result<Vec<Entry>, Error> {
        let mut directory = self
            .files
            .opendir(Path::new(path))
            .map_err(|_| Error::Access)?;
        let mut entries = Vec::new();
        loop {
            match directory.readdir() {
                Ok((name, stat)) => {
                    let name = name.to_str().ok_or(Error::Unsupported)?;
                    if matches!(name, "." | "..") {
                        continue;
                    }
                    entries.push(entry(child_path(path, name)?, stat));
                    if entries.len() > super::ENTRY_LIMIT {
                        return Err(Error::TooLarge);
                    }
                }
                Err(error) if error.code() == ssh2::ErrorCode::Session(-16) => break,
                Err(_) => return Err(Error::Connection),
            }
        }
        Ok(entries)
    }
    fn stat(&mut self, path: &str) -> Result<Option<Entry>, Error> {
        match self.files.lstat(Path::new(path)) {
            Ok(stat) => Ok(Some(entry(path.to_owned(), stat))),
            Err(error) if error.code() == ssh2::ErrorCode::SFTP(2) => Ok(None),
            Err(_) => Err(Error::Access),
        }
    }
    fn mkdir(&mut self, path: &str) -> Result<(), Error> {
        self.files
            .mkdir(Path::new(path), 0o755)
            .map_err(|_| Error::Access)
    }
    fn read(
        &mut self,
        path: &str,
        output: &mut dyn Write,
        progress: &Progress,
    ) -> Result<(), Error> {
        let mut file = self
            .files
            .open(Path::new(path))
            .map_err(|_| Error::Access)?;
        copy(&mut file, output, progress)
    }
    fn write(
        &mut self,
        path: &str,
        input: &mut dyn Read,
        progress: &Progress,
    ) -> Result<(), Error> {
        let mut file = self
            .files
            .create(Path::new(path))
            .map_err(|_| Error::Access)?;
        copy(input, &mut file, progress)?;
        file.close().map_err(|_| Error::Connection)
    }
}
