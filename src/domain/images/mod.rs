//! Image references are the only data exchanged between the two service browsers.
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::{fmt, sync::LazyLock};
use url::Url;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ImageRef {
    pub host: String,
    pub repository: String,
    pub version: String,
}
impl fmt::Display for ImageRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let delimiter = if digest(&self.version) { '@' } else { ':' };
        write!(
            f,
            "{}/{}{}{}",
            self.host, self.repository, delimiter, self.version
        )
    }
}

pub(crate) fn digest(s: &str) -> bool {
    s.strip_prefix("sha256:").is_some_and(|v| {
        v.len() == 64
            && v.bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    })
}
pub(crate) fn repository(s: &str) -> bool {
    static RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"^[a-z0-9]+(?:(?:[.]|__?|[-]+)[a-z0-9]+)*(?:/[a-z0-9]+(?:(?:[.]|__?|[-]+)[a-z0-9]+)*)*$").expect("repository pattern")
    });
    s.len() <= 255 && RE.is_match(s)
}
pub(crate) fn tag(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 128
        && !s.starts_with(['.', '-'])
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
}
impl ImageRef {
    pub fn parse(value: &str) -> Option<Self> {
        let value = value.strip_prefix("docker-pullable://").unwrap_or(value);
        if value.contains("://") || value.contains('\\') || value.contains(char::is_whitespace) {
            return None;
        }
        let (name, pin) = match value.split_once('@') {
            Some((n, d)) if digest(d) => (n, Some(d)),
            Some(_) => return None,
            None => (value, None),
        };
        let colon = name
            .rfind(':')
            .filter(|i| name.rfind('/').is_none_or(|slash| *i > slash));
        let (name, version) = match colon {
            Some(i) if tag(&name[i + 1..]) => (&name[..i], &name[i + 1..]),
            Some(_) => return None,
            None => (name, "latest"),
        };
        let (host, path) = name
            .split_once('/')
            .filter(|(h, _)| h.contains(['.', ':']) || *h == "localhost")
            .unwrap_or(("docker.io", name));
        let host = match host {
            "registry-1.docker.io" | "index.docker.io" => "docker.io",
            h => h,
        };
        let url = Url::parse(&format!("https://{host}")).ok()?;
        if url.host().is_none()
            || url.path() != "/"
            || !url.username().is_empty()
            || url.password().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
            || !repository(path)
        {
            return None;
        }
        let repository = if host == "docker.io" && !path.contains('/') {
            format!("library/{path}")
        } else {
            path.into()
        };
        Some(Self {
            host: host.into(),
            repository,
            version: pin.unwrap_or(version).into(),
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Registry {
    pub name: String,
    pub endpoint: String,
    pub credentials: bool,
    #[serde(default)]
    pub auth_origin: String,
}
impl fmt::Display for Registry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.name)
    }
}
impl Default for Registry {
    fn default() -> Self {
        Self {
            name: "Docker Hub".into(),
            endpoint: "https://registry-1.docker.io".into(),
            credentials: false,
            auth_origin: String::new(),
        }
    }
}
impl Registry {
    pub fn new(name: &str, endpoint: &str, credentials: bool) -> Option<Self> {
        let url = Url::parse(endpoint.trim()).ok()?;
        if name.trim().is_empty()
            || name.len() > 128
            || name.chars().any(char::is_control)
            || url.host().is_none()
            || !url.username().is_empty()
            || url.password().is_some()
            || url.path() != "/"
            || url.query().is_some()
            || url.fragment().is_some()
        {
            return None;
        }
        let local = match url.host()? {
            url::Host::Ipv4(ip) => ip.is_loopback(),
            url::Host::Ipv6(ip) => ip.is_loopback(),
            url::Host::Domain(h) => h == "localhost",
        };
        if !(url.scheme() == "https" || url.scheme() == "http" && local && !credentials) {
            return None;
        }
        let endpoint = if matches!(url.host_str(), Some("docker.io" | "index.docker.io"))
            && url.port().is_none()
        {
            "https://registry-1.docker.io".into()
        } else {
            url.origin().ascii_serialization()
        };
        // Docker interprets single-label names without a port as Hub namespaces.
        let endpoint = if matches!(url.host(),Some(url::Host::Domain(h)) if h != "localhost" && !h.contains('.'))
            && url.port().is_none()
        {
            format!("{endpoint}:443")
        } else {
            endpoint
        };
        Some(Self {
            name: name.trim().into(),
            endpoint,
            credentials,
            auth_origin: String::new(),
        })
    }
    pub fn host(&self) -> &str {
        let host = self.endpoint.split_once("://").map_or("", |(_, h)| h);
        if host == "registry-1.docker.io" {
            "docker.io"
        } else {
            host
        }
    }
    pub fn image(&self, repo: &str, version: &str) -> Option<ImageRef> {
        let sep = if digest(version) {
            '@'
        } else if tag(version) {
            ':'
        } else {
            return None;
        };
        if !repository(repo) {
            return None;
        }
        ImageRef::parse(&format!("{}/{repo}{sep}{version}", self.host()))
    }
}
#[cfg(test)]
mod tests;
