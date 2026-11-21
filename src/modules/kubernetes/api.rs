//! Run kubectl with explicit targets and bounded, cancellable I/O.
use super::model::{self, Kind, Resource};
use crate::{i18n::Key, infrastructure::process};
use iced::{Task, futures::SinkExt};
use serde_json::Value;
use std::{collections::BTreeSet, path::PathBuf, process::Stdio, time::Duration};
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWriteExt},
    process::Command,
};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct Target {
    pub file: Option<PathBuf>,
    pub context: String,
    pub namespace: String,
}
impl Target {
    pub fn flags(&self, ns: &str) -> Vec<String> {
        let mut flags = Vec::new();
        if let Some(file) = &self.file {
            flags.push(format!("--kubeconfig={}", file.display()))
        }
        if !self.context.is_empty() {
            flags.push(format!("--context={}", self.context))
        }
        if !ns.is_empty() {
            flags.push(format!("--namespace={ns}"))
        }
        flags
    }
    fn command(&self, ns: &str, args: &[String]) -> Command {
        let mut cmd = Command::new(process::executable("kubectl"));
        cmd.args(self.flags(ns))
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        #[cfg(windows)]
        cmd.creation_flags(0x08000000);
        cmd
    }
}
pub(super) fn words(parts: &[&str]) -> Vec<String> {
    parts.iter().map(|s| (*s).to_owned()).collect()
}
pub(super) fn failure(err: &[u8]) -> Key {
    let e = String::from_utf8_lossy(err).to_ascii_lowercase();
    if e.contains("forbidden") || e.contains("unauthorized") || e.contains("permission denied") {
        Key::KubeDenied
    } else if e.contains("notfound") || e.contains("not found") {
        Key::KubeMissing
    } else if e.contains("conflict") || e.contains("alreadyexists") || e.contains("test failed") {
        Key::KubeConflict
    } else if e.contains("unable to connect")
        || e.contains("connection refused")
        || e.contains("i/o timeout")
        || e.contains("no such host")
    {
        Key::KubeOffline
    } else {
        Key::KubeFailed
    }
}
async fn bounded(r: impl AsyncRead + Unpin, limit: usize) -> Result<Vec<u8>, Key> {
    let mut bytes = Vec::new();
    r.take(limit as u64 + 1)
        .read_to_end(&mut bytes)
        .await
        .map_err(|_| Key::KubeFailed)?;
    if bytes.len() > limit {
        return Err(Key::DockerLarge);
    }
    Ok(bytes)
}
pub(super) async fn run(
    t: &Target,
    ns: &str,
    args: &[String],
    input: Option<&[u8]>,
) -> Result<Vec<u8>, Key> {
    let mut cmd = t.command(ns, args);
    if input.is_some() {
        cmd.stdin(Stdio::piped());
    }
    let mut child = cmd.spawn().map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            Key::KubeInstall
        } else {
            Key::KubeFailed
        }
    })?;
    let mut stdin = child.stdin.take();
    let stdout = child.stdout.take().ok_or(Key::KubeFailed)?;
    let stderr = child.stderr.take().ok_or(Key::KubeFailed)?;
    tokio::time::timeout(Duration::from_secs(45), async {
        let writing = async move {
            if let (Some(ref mut pipe), Some(bytes)) = (stdin.as_mut(), input) {
                pipe.write_all(bytes).await.map_err(|_| Key::KubeFailed)?;
            }
            drop(stdin);
            Ok(())
        };
        let (out, err, status, ()) = tokio::try_join!(
            bounded(stdout, 16 * 1024 * 1024),
            bounded(stderr, 128 * 1024),
            async { child.wait().await.map_err(|_| Key::KubeFailed) },
            writing
        )?;
        if status.success() {
            Ok(out)
        } else {
            Err(failure(&err))
        }
    })
    .await
    .map_err(|_| Key::KubeOffline)?
}
pub(super) async fn json(t: &Target, ns: &str, args: &[&str]) -> Result<Value, Key> {
    serde_json::from_slice(&run(t, ns, &words(args), None).await?).map_err(|_| Key::KubeFailed)
}
pub(super) async fn contexts(
    file: Option<PathBuf>,
) -> Result<(Vec<(String, String)>, String), Key> {
    let t = Target {
        file,
        ..Default::default()
    };
    let fmt = "jsonpath={.current-context}{'\\n'}{range .contexts[*]}{.name}{'\\t'}{.context.namespace}{'\\n'}{end}";
    let bytes = run(&t, "", &words(&["config", "view", "-o", fmt]), None).await?;
    let text = String::from_utf8(bytes).map_err(|_| Key::KubeFailed)?;
    let mut lines = text.lines();
    let current = lines.next().unwrap_or_default().to_owned();
    let mut options = lines
        .filter_map(|l| l.split_once('\t'))
        .map(|(name, ns)| {
            (
                name.to_owned(),
                if ns.is_empty() {
                    "default".into()
                } else {
                    ns.into()
                },
            )
        })
        .collect::<Vec<_>>();
    options.sort();
    Ok((options, current))
}
pub(super) async fn list(
    t: &Target,
    kind: Kind,
    parent: Option<&Resource>,
) -> Result<Vec<Resource>, Key> {
    let ns = if kind == Kind::Node {
        ""
    } else if let Some(parent) = parent {
        &parent.ns
    } else {
        &t.namespace
    };
    let mut args = vec!["get", kind.api(), "-o", "json"];
    if ns.is_empty() && kind != Kind::Node {
        args.push("--all-namespaces");
    }
    let mut data = json(t, ns, &args).await?;
    if let Some(parent) = parent {
        let sets = json(t, ns, &["get", "replicasets.apps", "-o", "json"]).await?;
        let owners: BTreeSet<_> = sets["items"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|v| model::controlled_by(v, &parent.uid))
            .map(|v| model::string(v, "/metadata/uid"))
            .collect();
        if let Some(pods) = data["items"].as_array_mut() {
            pods.retain(|p| {
                p.pointer("/metadata/ownerReferences")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .any(|o| {
                        o["controller"] == true
                            && o["kind"] == "ReplicaSet"
                            && o["uid"].as_str().is_some_and(|uid| owners.contains(uid))
                    })
            })
        }
    }
    Ok(model::resources(kind, &data))
}
#[derive(Debug, Clone)]
pub(crate) struct Details {
    pub version: String,
    pub yaml: String,
    pub events: Result<Vec<String>, Key>,
    pub containers: Vec<model::Container>,
    pub replicas: i64,
    pub ports: Vec<u16>,
}
pub(super) async fn details(t: &Target, kind: Kind, r: &Resource) -> Result<Details, Key> {
    let path = format!("{}/{}", kind.api(), r.name);
    let selector = format!("involvedObject.uid={}", r.uid);
    let read = ["get", &path, "-o", "json"];
    let history = ["get", "events", "--field-selector", &selector, "-o", "json"];
    let (object, events) = tokio::join!(json(t, &r.ns, &read), json(t, &r.ns, &history));
    let v = object?;
    if model::string(&v, "/metadata/uid") != r.uid {
        return Err(Key::KubeConflict);
    }
    let containers = model::containers(kind, &v);
    let ports = if kind == Kind::Service {
        v.pointer("/spec/ports")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|p| p["port"].as_u64().and_then(|n| u16::try_from(n).ok()))
            .collect()
    } else {
        v.pointer("/spec/containers")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .flat_map(|c| c["ports"].as_array().into_iter().flatten())
            .filter_map(|p| {
                p["containerPort"]
                    .as_u64()
                    .and_then(|n| u16::try_from(n).ok())
            })
            .collect()
    };
    let events = events.map(|e| {
        e["items"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|v| {
                let reason = v["reason"].as_str().unwrap_or_default();
                if kind == Kind::Secret {
                    reason.to_owned()
                } else {
                    format!("{reason}  {}", v["message"].as_str().unwrap_or_default())
                }
            })
            .collect()
    });
    let replicas = v
        .pointer("/spec/replicas")
        .and_then(Value::as_i64)
        .unwrap_or(1);
    let version = model::string(&v, "/metadata/resourceVersion").into();
    let yaml = serde_saphyr::to_string(&model::redact(v)).map_err(|_| Key::KubeFailed)?;
    Ok(Details {
        version,
        yaml,
        events,
        containers,
        replicas,
        ports,
    })
}

#[derive(Debug, Clone)]
pub(crate) enum StreamUpdate {
    Data(String),
    Listening,
    End(Result<(), Key>),
}

pub(super) async fn pull_failures(t: &Target, deployment: &Resource) -> Result<String, Key> {
    let pods = list(t, Kind::Pod, Some(deployment)).await?;
    let mut messages = Vec::new();
    for pod in pods
        .into_iter()
        .filter(|p| {
            matches!(
                p.status.as_str(),
                "ErrImagePull" | "ImagePullBackOff" | "InvalidImageName"
            )
        })
        .take(8)
    {
        messages.push(format!("{}/{}  {}", pod.ns, pod.name, pod.status));
        let selector = format!("involvedObject.uid={}", pod.uid);
        let events = json(
            t,
            &pod.ns,
            &["get", "events", "--field-selector", &selector, "-o", "json"],
        )
        .await;
        if let Ok(events) = events {
            for event in events["items"]
                .as_array()
                .into_iter()
                .flatten()
                .filter(|e| e["type"] == "Warning")
                .take(4)
            {
                if let Some(message) = event["message"].as_str() {
                    messages.push(message.to_owned());
                }
            }
        }
    }
    Ok(messages.join("\n"))
}
pub(super) fn stream(
    t: Target,
    ns: String,
    args: Vec<String>,
    forward: bool,
    id: u64,
) -> Task<super::Event> {
    let source = iced::stream::channel(8, async move |mut sender| {
        let result = async {
            let mut child = t
                .command(&ns, &args)
                .spawn()
                .map_err(|_| Key::KubeInstall)?;
            let mut out = child.stdout.take().ok_or(Key::KubeFailed)?;
            let err = child.stderr.take().ok_or(Key::KubeFailed)?;
            let read = async {
                let mut buf = [0u8; 8192];
                let mut started = false;
                let mut pending = Vec::new();
                loop {
                    let n = out.read(&mut buf).await.map_err(|_| Key::KubeFailed)?;
                    if n == 0 {
                        break;
                    }
                    if forward {
                        if !started {
                            started = true;
                            sender
                                .send(super::Event::Stream(id, StreamUpdate::Listening))
                                .await
                                .map_err(|_| Key::KubeFailed)?;
                        }
                    } else {
                        pending.extend_from_slice(&buf[..n]);
                        let cut = match std::str::from_utf8(&pending) {
                            Err(e) if e.error_len().is_none() => e.valid_up_to(),
                            _ => pending.len(),
                        };
                        if cut > 0 {
                            let text = String::from_utf8_lossy(&pending[..cut]).into_owned();
                            pending.drain(..cut);
                            sender
                                .send(super::Event::Stream(id, StreamUpdate::Data(text)))
                                .await
                                .map_err(|_| Key::KubeFailed)?;
                        }
                    }
                }
                if !pending.is_empty() {
                    let _ = sender
                        .send(super::Event::Stream(
                            id,
                            StreamUpdate::Data(String::from_utf8_lossy(&pending).into_owned()),
                        ))
                        .await;
                }
                Ok(())
            };
            let ((), err, status) = tokio::try_join!(read, bounded(err, 128 * 1024), async {
                child.wait().await.map_err(|_| Key::KubeFailed)
            })?;
            if status.success() {
                Ok(())
            } else {
                Err(failure(&err))
            }
        }
        .await;
        let _ = sender
            .send(super::Event::Stream(id, StreamUpdate::End(result)))
            .await;
    });
    Task::run(source, |e| e)
}
