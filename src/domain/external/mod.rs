//! Interpret requests that arrive from outside the application.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use url::Url;

/// A request to open content that arrived from outside the application.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(crate) enum Request {
    /// Raise the running instance without opening content.
    Focus,
    /// Open a file from the local file system.
    File { path: PathBuf },
    /// Switch to an existing project with the given directory.
    Project { directory: PathBuf },
}

/// A deep link that targets the application.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Link {
    /// Open the linked file.
    Open(PathBuf),
    /// Switch to the linked project.
    Project(PathBuf),
}

impl Link {
    const SCHEME: &'static str = "iwe";

    /// Parse a deep link such as `iwe://open?path=/notes/ideas.md`.
    ///
    /// Returns `None` for other schemes, unknown actions, and relative paths.
    pub(crate) fn parse(text: &str) -> Option<Self> {
        let url = Url::parse(text).ok()?;
        if url.scheme() != Self::SCHEME {
            return None;
        }
        let action = url.host_str()?.to_ascii_lowercase();
        let path = query_path(&url)?;
        match action.as_str() {
            "open" => Some(Self::Open(path)),
            "project" => Some(Self::Project(path)),
            _ => None,
        }
    }

    /// Build a deep link that opens the given file.
    #[cfg(test)]
    pub(crate) fn open_text(path: &std::path::Path) -> String {
        let mut value = path.to_string_lossy().into_owned();
        if !value.starts_with('/') {
            value = format!("/{value}");
        }
        format!("{}://open?path={}", Self::SCHEME, value)
    }
}

fn query_path(url: &Url) -> Option<PathBuf> {
    let value = url
        .query_pairs()
        .find(|(name, _)| name == "path")
        .map(|(_, value)| value.into_owned())?;
    let path = PathBuf::from(value);
    path.is_absolute().then_some(path)
}

#[cfg(test)]
mod tests;
