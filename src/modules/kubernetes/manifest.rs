//! Freeze local manifests before showing their server-side preview.
use super::{
    api::{self, Target, words},
    model,
};
use crate::i18n::Key;
use serde_json::Value;
use std::{fmt, io::Read, path::Path};

pub(crate) struct Change {
    pub label: String,
    pub diff: String,
    pub secret_changed: bool,
    namespace: String,
    body: Vec<u8>,
    exists: bool,
}
pub(crate) struct Plan(pub Vec<Change>);
impl fmt::Debug for Plan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Plan").field(&self.0.len()).finish()
    }
}
fn documents(path: &Path) -> Result<Vec<Value>, Key> {
    let mut buf = Vec::new();
    std::fs::File::open(path)
        .and_then(|f| f.take(2_097_153).read_to_end(&mut buf))
        .map_err(|_| Key::KubeManifestError)?;
    if buf.len() > 2_097_152 {
        return Err(Key::DockerLarge);
    }
    let mut out = Vec::new();
    for mut value in
        serde_saphyr::from_slice_multiple::<Value>(&buf).map_err(|_| Key::KubeManifestError)?
    {
        if value["kind"] == "List" {
            out.append(
                value["items"]
                    .as_array_mut()
                    .ok_or(Key::KubeManifestError)?,
            )
        } else {
            out.push(value)
        }
    }
    if out.is_empty() || out.len() > 100 {
        return Err(Key::KubeManifestError);
    }
    Ok(out)
}
async fn current(t: &Target, ns: &str, data: &[u8]) -> Result<Option<Value>, Key> {
    let out = api::run(
        t,
        ns,
        &words(&["get", "-f", "-", "--ignore-not-found", "-o", "json"]),
        Some(data),
    )
    .await?;
    if out.iter().all(u8::is_ascii_whitespace) {
        return Ok(None);
    }
    let mut v: Value = serde_json::from_slice(&out).map_err(|_| Key::KubeManifestError)?;
    if v["kind"] == "List" {
        let items = v["items"].as_array_mut().ok_or(Key::KubeManifestError)?;
        if items.len() > 1 {
            return Err(Key::KubeManifestError);
        }
        Ok(items.pop())
    } else {
        Ok(Some(v))
    }
}

fn printable(v: Value) -> Result<String, Key> {
    let mut v = model::redact(v);
    if let Some(m) = v.as_object_mut() {
        m.remove("status");
    }
    if let Some(m) = v["metadata"].as_object_mut() {
        for k in ["uid", "resourceVersion", "generation", "creationTimestamp"] {
            m.remove(k);
        }
    }
    serde_saphyr::to_string(&v).map_err(|_| Key::KubeManifestError)
}
fn args(dry: bool) -> Vec<String> {
    let mut a = words(&[
        "apply",
        "--server-side",
        "--field-manager=iwe",
        "-f",
        "-",
        "-o",
        "json",
    ]);
    if dry {
        a.push("--dry-run=server".into())
    }
    a
}
pub(super) async fn preview(t: &Target, path: std::path::PathBuf) -> Result<Plan, Key> {
    let docs = tokio::task::spawn_blocking(move || documents(&path))
        .await
        .map_err(|_| Key::KubeFailed)??;
    let mut changes = Vec::new();
    for doc in docs {
        let name = model::string(&doc, "/metadata/name");
        let kind = model::string(&doc, "/kind");
        if name.is_empty() || kind.is_empty() {
            return Err(Key::KubeManifestError);
        }
        let explicit = model::string(&doc, "/metadata/namespace");
        if !explicit.is_empty() && !t.namespace.is_empty() && explicit != t.namespace {
            return Err(Key::KubeNamespaceMismatch);
        }
        let ns = if explicit.is_empty() {
            t.namespace.as_str()
        } else {
            explicit
        };
        let body = serde_json::to_vec(&doc).map_err(|_| Key::KubeManifestError)?;
        let before = current(t, ns, &body).await?;
        let after: Value =
            serde_json::from_slice(&api::run(t, ns, &args(true), Some(&body)).await?)
                .map_err(|_| Key::KubeManifestError)?;
        let label = format!(
            "{kind}/{name}  {}",
            model::string(&after, "/metadata/namespace")
        );
        let exists = before.is_some();
        let secret_changed = after["kind"] == "Secret"
            && before.as_ref().and_then(|v| v.get("data")) != after.get("data");
        let mut frozen = doc.clone();
        if let Some(prior) = &before {
            frozen["metadata"]["resourceVersion"] = prior["metadata"]["resourceVersion"].clone();
        }
        let body = serde_json::to_vec(&frozen).map_err(|_| Key::KubeManifestError)?;
        let before = before.map(printable).transpose()?.unwrap_or_default();
        let after = printable(after)?;
        let diff = similar::TextDiff::from_lines(&before, &after)
            .unified_diff()
            .header("−", "+")
            .to_string();
        changes.push(Change {
            label,
            diff,
            secret_changed,
            namespace: ns.into(),
            body,
            exists,
        });
    }
    Ok(Plan(changes))
}
pub(super) async fn apply(t: &Target, plan: &Plan) -> Result<(), Key> {
    for (index, c) in plan.0.iter().enumerate() {
        let args = if c.exists {
            args(false)
        } else {
            words(&["create", "--field-manager=iwe", "-f", "-", "-o", "name"])
        };
        let result = api::run(t, &c.namespace, &args, Some(&c.body)).await;
        if let Err(k) = result {
            return Err(if index == 0 { k } else { Key::KubePartial });
        }
    }
    Ok(())
}
