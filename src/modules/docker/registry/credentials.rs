//! Resolve login information only for the explicitly selected registry.
use super::api::Error;
use crate::{domain::images::Registry, infrastructure::process};
use serde::Deserialize;
use std::{collections::BTreeMap, process::Stdio, time::Duration};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    process::Command,
};
use zeroize::{Zeroize, Zeroizing};

#[derive(Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct Config {
    creds_store: String,
    cred_helpers: BTreeMap<String, String>,
}
#[derive(Deserialize)]
pub(super) struct Login {
    #[serde(rename = "Username")]
    pub username: String,
    #[serde(rename = "Secret")]
    pub secret: String,
}
impl Drop for Login {
    fn drop(&mut self) {
        self.secret.zeroize();
    }
}
pub(super) async fn get(registry: &Registry) -> Result<Login, Error> {
    let directory = std::env::var_os("DOCKER_CONFIG")
        .map(std::path::PathBuf::from)
        .or_else(|| directories::BaseDirs::new().map(|d| d.home_dir().join(".docker")))
        .ok_or(Error::Login)?;
    let file = tokio::fs::File::open(directory.join("config.json"))
        .await
        .map_err(|_| Error::Login)?;
    let mut raw = Zeroizing::new(Vec::new());
    file.take(1024 * 1024 + 1)
        .read_to_end(&mut raw)
        .await
        .map_err(|_| Error::Login)?;
    if raw.len() > 1024 * 1024 {
        return Err(Error::Login);
    }
    let config: Config = serde_json::from_slice(&raw).map_err(|_| Error::Login)?;
    let server = if registry.host() == "docker.io" {
        "https://index.docker.io/v1/"
    } else {
        registry.host()
    };
    let helper = config
        .cred_helpers
        .get(registry.host())
        .or_else(|| config.cred_helpers.get(server))
        .unwrap_or(&config.creds_store);
    if helper.is_empty()
        || !helper
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'-' | b'_'))
    {
        return Err(Error::Login);
    }
    let mut command = Command::new(process::executable(&format!("docker-credential-{helper}")));
    command
        .arg("get")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    #[cfg(windows)]
    command.creation_flags(0x08000000);
    tokio::time::timeout(Duration::from_secs(15), async {
        let mut child = command.spawn().map_err(|_| Error::Login)?;
        let mut input = child.stdin.take().ok_or(Error::Login)?;
        input
            .write_all(server.as_bytes())
            .await
            .map_err(|_| Error::Login)?;
        drop(input);
        let mut bytes = Zeroizing::new(Vec::new());
        child
            .stdout
            .take()
            .ok_or(Error::Login)?
            .take(65537)
            .read_to_end(&mut bytes)
            .await
            .map_err(|_| Error::Login)?;
        if bytes.len() > 65536 || !child.wait().await.map_err(|_| Error::Login)?.success() {
            return Err(Error::Login);
        }
        serde_json::from_slice(&bytes).map_err(|_| Error::Login)
    })
    .await
    .map_err(|_| Error::Login)?
}
