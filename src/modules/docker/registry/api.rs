//! Read OCI and Docker distributions independently of the selected Docker daemon.
use crate::domain::images::{Registry, digest, repository, tag};
use reqwest::{Client, Response, StatusCode, header};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{sync::LazyLock, time::Duration};
use url::Url;
use zeroize::Zeroizing;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Error {
    Network,
    Denied,
    Login,
    Missing,
    Invalid,
    Limit,
}
#[derive(Debug, Clone, Default)]
pub(crate) struct Page {
    pub names: Vec<String>,
    pub next: Option<String>,
}
#[derive(Debug, Clone)]
pub(crate) struct Manifest {
    pub digest: String,
    pub platforms: Vec<String>,
}
const ACCEPT: &str = "application/vnd.oci.image.index.v1+json, application/vnd.docker.distribution.manifest.list.v2+json, application/vnd.oci.image.manifest.v1+json, application/vnd.docker.distribution.manifest.v2+json";
fn client() -> Result<Client, Error> {
    Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(25))
        .connect_timeout(Duration::from_secs(8))
        .build()
        .map_err(|_| Error::Network)
}
async fn bytes(mut response: Response) -> Result<Vec<u8>, Error> {
    if !response.status().is_success() {
        return Err(match response.status().as_u16() {
            401 | 403 => Error::Denied,
            404 | 405 => Error::Missing,
            _ => Error::Network,
        });
    }
    let mut out = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|_| Error::Network)? {
        if out.len() + chunk.len() > 4 * 1024 * 1024 {
            return Err(Error::Limit);
        }
        out.extend(chunk);
    }
    Ok(out)
}
fn secure(url: &Url) -> bool {
    url.scheme() == "https"
        && url.username().is_empty()
        && url.password().is_none()
        && url.host().is_some()
}
fn parameter<'a>(challenge: &'a str, name: &str) -> Option<&'a str> {
    static RE: LazyLock<regex::Regex> = LazyLock::new(|| {
        regex::Regex::new(r#"([a-zA-Z]+)\s*=\s*"([^"]*)""#).expect("auth parameter pattern")
    });
    RE.captures_iter(challenge).find_map(|m| {
        m[1].eq_ignore_ascii_case(name)
            .then(|| m.get(2).unwrap().as_str())
    })
}
async fn get(
    client: &Client,
    reg: &Registry,
    url: Url,
    scope: Option<&str>,
) -> Result<Response, Error> {
    let base = Url::parse(&reg.endpoint).map_err(|_| Error::Invalid)?;
    if url.origin() != base.origin() {
        return Err(Error::Invalid);
    }
    let send = || client.get(url.clone()).header(header::ACCEPT, ACCEPT);
    let response = send().send().await.map_err(|_| Error::Network)?;
    if response.status() != StatusCode::UNAUTHORIZED {
        return Ok(response);
    }
    let challenge = response
        .headers()
        .get(header::WWW_AUTHENTICATE)
        .and_then(|v| v.to_str().ok())
        .ok_or(Error::Denied)?;
    let (scheme, params) = challenge.split_once(' ').ok_or(Error::Denied)?;
    if scheme.eq_ignore_ascii_case("basic") && reg.credentials && secure(&base) {
        let login = super::credentials::get(reg).await?;
        return send()
            .basic_auth(&login.username, Some(&login.secret))
            .send()
            .await
            .map_err(|_| Error::Network);
    }
    if !scheme.eq_ignore_ascii_case("bearer") {
        return Err(Error::Denied);
    }
    let realm = Url::parse(parameter(params, "realm").ok_or(Error::Invalid)?)
        .map_err(|_| Error::Invalid)?;
    if !(secure(&realm)
        || base.scheme() == "http" && realm.origin() == base.origin() && !reg.credentials)
    {
        return Err(Error::Denied);
    }
    let mut params = Vec::new();
    if let Some(service) = parameter(challenge, "service") {
        params.push(("service", service))
    }
    if let Some(scope) = scope.or_else(|| parameter(challenge, "scope")) {
        params.push(("scope", scope))
    }
    let token_request = if reg.credentials {
        let trusted = realm.origin() == base.origin()
            || reg.host() == "docker.io"
                && realm.origin().ascii_serialization() == "https://auth.docker.io"
            || reg.auth_origin == realm.origin().ascii_serialization();
        if !trusted {
            return Err(Error::Login);
        }
        let login = super::credentials::get(reg).await?;
        if login.username == "<token>" {
            params.push(("grant_type", "refresh_token"));
            params.push(("refresh_token", &login.secret));
            params.push(("client_id", "iwe"));
            client.post(realm).form(&params)
        } else {
            client
                .get(realm)
                .query(&params)
                .basic_auth(&login.username, Some(&login.secret))
        }
    } else {
        client.get(realm).query(&params)
    };
    let data =
        Zeroizing::new(bytes(token_request.send().await.map_err(|_| Error::Network)?).await?);
    #[derive(serde::Deserialize)]
    struct Token<'a> {
        token: Option<&'a str>,
        access_token: Option<&'a str>,
    }
    let token: Token<'_> = serde_json::from_slice(&data).map_err(|_| Error::Denied)?;
    let value = token
        .token
        .filter(|s| !s.is_empty())
        .or(token.access_token.filter(|s| !s.is_empty()))
        .ok_or(Error::Denied)?;
    send()
        .bearer_auth(value)
        .send()
        .await
        .map_err(|_| Error::Network)
}
pub(super) async fn check(reg: &Registry) -> Result<(), Error> {
    bytes(
        get(
            &client()?,
            reg,
            Url::parse(&format!("{}/v2/", reg.endpoint)).map_err(|_| Error::Invalid)?,
            None,
        )
        .await?,
    )
    .await
    .map(|_| ())
}
fn continuation(base: &Url, raw: &str) -> Option<String> {
    let url = base.join(raw).ok()?;
    (url.origin() == base.origin()
        && url.path() == base.path()
        && url.username().is_empty()
        && url.password().is_none()
        && url.fragment().is_none())
    .then(|| url.to_string())
}
pub(super) async fn list(
    reg: &Registry,
    repo: Option<&str>,
    next: Option<&str>,
) -> Result<Page, Error> {
    if repo.is_some_and(|r| !repository(r)) {
        return Err(Error::Invalid);
    }
    let hub = repo.is_none() && reg.host() == "docker.io";
    let path = if hub {
        "https://hub.docker.com/v2/namespaces/library/repositories?page_size=40".into()
    } else if let Some(r) = repo {
        format!("{}/v2/{r}/tags/list?n=40", reg.endpoint)
    } else {
        format!("{}/v2/_catalog?n=40", reg.endpoint)
    };
    let base = Url::parse(&path).map_err(|_| Error::Invalid)?;
    let url = if let Some(n) = next {
        Url::parse(&continuation(&base, n).ok_or(Error::Invalid)?).map_err(|_| Error::Invalid)?
    } else {
        base.clone()
    };
    let client = client()?;
    let response = if hub {
        client.get(url).send().await.map_err(|_| Error::Network)?
    } else {
        let scope = repo
            .map(|r| format!("repository:{r}:pull"))
            .unwrap_or_else(|| "registry:catalog:*".into());
        get(&client, reg, url, Some(&scope)).await?
    };
    let link = response
        .headers()
        .get(header::LINK)
        .and_then(|l| l.to_str().ok())
        .and_then(|l| {
            l.split(',')
                .find(|s| s.contains("rel=\"next\"") || s.contains("rel=next"))
        })
        .and_then(|l| l.split_once('<')?.1.split_once('>'))
        .and_then(|(p, _)| continuation(&base, p));
    let body: Value =
        serde_json::from_slice(&bytes(response).await?).map_err(|_| Error::Invalid)?;
    let names = if hub {
        body["results"]
            .as_array()
            .ok_or(Error::Invalid)?
            .iter()
            .filter_map(|v| v["name"].as_str())
            .filter(|r| repository(r))
            .map(|n| format!("library/{n}"))
            .collect()
    } else {
        let v = &body[if repo.is_some() {
            "tags"
        } else {
            "repositories"
        }];
        if v.is_null() {
            vec![]
        } else {
            v.as_array()
                .ok_or(Error::Invalid)?
                .iter()
                .filter_map(Value::as_str)
                .filter(|n| {
                    if repo.is_some() {
                        tag(n)
                    } else {
                        repository(n)
                    }
                })
                .map(str::to_owned)
                .collect()
        }
    };
    Ok(Page {
        names,
        next: if hub {
            body["next"].as_str().and_then(|n| continuation(&base, n))
        } else {
            link
        },
    })
}
pub(super) async fn manifest(reg: &Registry, repo: &str, version: &str) -> Result<Manifest, Error> {
    if !repository(repo) || !(tag(version) || digest(version)) {
        return Err(Error::Invalid);
    }
    let client = client()?;
    let scope = format!("repository:{repo}:pull");
    let url = |path: &str| {
        Url::parse(&format!("{}/v2/{repo}/{path}", reg.endpoint)).map_err(|_| Error::Invalid)
    };
    let response = get(
        &client,
        reg,
        url(&format!("manifests/{version}"))?,
        Some(&scope),
    )
    .await?;
    let supplied = response
        .headers()
        .get("docker-content-digest")
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned);
    let data = bytes(response).await?;
    let id = format!("sha256:{:x}", Sha256::digest(&data));
    if supplied.as_ref().is_some_and(|s| *s != id) || digest(version) && version != id {
        return Err(Error::Invalid);
    }
    let manifest: Value = serde_json::from_slice(&data).map_err(|_| Error::Invalid)?;
    fn platform(v: &Value) -> Option<String> {
        let os = v["os"].as_str()?;
        let arch = v["architecture"].as_str()?;
        if os == "unknown" || arch == "unknown" {
            return None;
        }
        Some(match v["variant"].as_str() {
            Some(s) => format!("{os}/{arch}/{s}"),
            None => format!("{os}/{arch}"),
        })
    }
    let mut platforms = Vec::new();
    if let Some(list) = manifest["manifests"].as_array() {
        platforms.extend(list.iter().filter_map(|m| platform(&m["platform"])))
    } else if let Some(config) = manifest["config"]["digest"].as_str().filter(|s| digest(s)) {
        let mut response =
            get(&client, reg, url(&format!("blobs/{config}"))?, Some(&scope)).await?;
        for _ in 0..3 {
            if !response.status().is_redirection() {
                break;
            }
            let location = response
                .headers()
                .get(header::LOCATION)
                .and_then(|h| h.to_str().ok())
                .and_then(|l| response.url().join(l).ok())
                .filter(secure)
                .ok_or(Error::Denied)?;
            response = client
                .get(location)
                .send()
                .await
                .map_err(|_| Error::Network)?;
        }
        let data = bytes(response).await?;
        if format!("sha256:{:x}", Sha256::digest(&data)) != config {
            return Err(Error::Invalid);
        }
        platforms.extend(platform(
            &serde_json::from_slice(&data).map_err(|_| Error::Invalid)?,
        ));
    }
    platforms.sort();
    platforms.dedup();
    Ok(Manifest {
        digest: id,
        platforms,
    })
}
