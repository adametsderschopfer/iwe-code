use crate::domain::images::ImageRef;
use serde_json::{Value, json};
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum Kind {
    #[default]
    Deployment,
    StatefulSet,
    DaemonSet,
    Pod,
    Service,
    Ingress,
    Job,
    CronJob,
    ConfigMap,
    Secret,
    Node,
}
impl Kind {
    pub const ALL: [Self; 11] = [
        Self::Deployment,
        Self::StatefulSet,
        Self::DaemonSet,
        Self::Pod,
        Self::Service,
        Self::Ingress,
        Self::Job,
        Self::CronJob,
        Self::ConfigMap,
        Self::Secret,
        Self::Node,
    ];
    pub fn api(self) -> &'static str {
        match self {
            Self::Deployment => "deployments.apps",
            Self::StatefulSet => "statefulsets.apps",
            Self::DaemonSet => "daemonsets.apps",
            Self::Pod => "pods",
            Self::Service => "services",
            Self::Ingress => "ingresses.networking.k8s.io",
            Self::Job => "jobs.batch",
            Self::CronJob => "cronjobs.batch",
            Self::ConfigMap => "configmaps",
            Self::Secret => "secrets",
            Self::Node => "nodes",
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Deployment => "Deployment",
            Self::StatefulSet => "StatefulSet",
            Self::DaemonSet => "DaemonSet",
            Self::Pod => "Pod",
            Self::Service => "Service",
            Self::Ingress => "Ingress",
            Self::Job => "Job",
            Self::CronJob => "CronJob",
            Self::ConfigMap => "ConfigMap",
            Self::Secret => "Secret",
            Self::Node => "Node",
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Resource {
    pub name: String,
    pub ns: String,
    pub uid: String,
    pub version: String,
    pub status: String,
}
pub fn string<'a>(v: &'a Value, path: &str) -> &'a str {
    v.pointer(path).and_then(Value::as_str).unwrap_or_default()
}
pub fn resources(kind: Kind, v: &Value) -> Vec<Resource> {
    let mut rows = v["items"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|v| {
            let name = string(v, "/metadata/name");
            if name.is_empty() {
                return None;
            }
            let n = |p: &str| v.pointer(p).and_then(Value::as_i64).unwrap_or(0);
            let status = match kind {
                Kind::Pod => v
                    .pointer("/status/containerStatuses")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .find_map(|c| c.pointer("/state/waiting/reason").and_then(Value::as_str))
                    .unwrap_or(string(v, "/status/phase"))
                    .to_owned(),
                Kind::Deployment | Kind::StatefulSet => {
                    format!("{}/{}", n("/status/readyReplicas"), n("/spec/replicas"))
                }
                Kind::DaemonSet => format!(
                    "{}/{}",
                    n("/status/numberReady"),
                    n("/status/desiredNumberScheduled")
                ),
                Kind::Job => format!(
                    "{}/{}",
                    n("/status/succeeded"),
                    v.pointer("/spec/completions")
                        .and_then(Value::as_i64)
                        .unwrap_or(1)
                ),
                Kind::Service => string(v, "/spec/type").into(),
                Kind::CronJob => string(v, "/spec/schedule").into(),
                Kind::Secret => string(v, "/type").into(),
                Kind::Ingress => string(v, "/spec/ingressClassName").into(),
                Kind::Node => v
                    .pointer("/status/conditions")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .find(|c| c["type"] == "Ready")
                    .and_then(|c| c["status"].as_str())
                    .unwrap_or_default()
                    .into(),
                Kind::ConfigMap => v["data"].as_object().map_or(0, |m| m.len()).to_string(),
            };
            Some(Resource {
                name: name.into(),
                ns: string(v, "/metadata/namespace").into(),
                uid: string(v, "/metadata/uid").into(),
                version: string(v, "/metadata/resourceVersion").into(),
                status,
            })
        })
        .collect::<Vec<_>>();
    rows.sort_by(|a, b| (&a.ns, &a.name).cmp(&(&b.ns, &b.name)));
    rows
}
pub fn redact(mut v: Value) -> Value {
    if v["kind"] == "Secret" {
        let mut meta = serde_json::Map::new();
        for k in [
            "name",
            "namespace",
            "uid",
            "resourceVersion",
            "creationTimestamp",
        ] {
            if let Some(val) = v["metadata"].get(k) {
                meta.insert(k.into(), val.clone());
            }
        }
        return json!({"apiVersion":v["apiVersion"],"kind":"Secret","metadata":meta,"type":v["type"],"data":"••••"});
    }
    if let Some(meta) = v.get_mut("metadata").and_then(Value::as_object_mut) {
        meta.remove("managedFields");
        if let Some(a) = meta.get_mut("annotations").and_then(Value::as_object_mut) {
            a.remove("kubectl.kubernetes.io/last-applied-configuration");
        }
    }
    if let Some(items) = v.get_mut("items").and_then(Value::as_array_mut) {
        for i in items {
            *i = redact(i.take());
        }
    }
    v
}
pub fn controlled_by(v: &Value, uid: &str) -> bool {
    !uid.is_empty()
        && v.pointer("/metadata/ownerReferences")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .any(|o| o["uid"] == uid && o["controller"] == true)
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Container {
    pub name: String,
    pub image: String,
    pub running: Option<ImageRef>,
    pub index: usize,
}
pub fn containers(kind: Kind, v: &Value) -> Vec<Container> {
    let groups = match kind {
        Kind::Deployment => vec![("/spec/template/spec/containers", "")],
        Kind::Pod => vec![
            ("/spec/containers", "/status/containerStatuses"),
            ("/spec/initContainers", "/status/initContainerStatuses"),
            (
                "/spec/ephemeralContainers",
                "/status/ephemeralContainerStatuses",
            ),
        ],
        _ => vec![],
    };
    groups
        .into_iter()
        .flat_map(|(spec, status)| {
            v.pointer(spec)
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .enumerate()
                .filter_map(move |(index, c)| {
                    let name = c["name"].as_str()?;
                    let image = c["image"].as_str()?;
                    let running = v
                        .pointer(status)
                        .and_then(Value::as_array)
                        .into_iter()
                        .flatten()
                        .find(|s| s["name"] == name)
                        .and_then(|s| s["imageID"].as_str())
                        .filter(|s| s.contains('@'))
                        .and_then(ImageRef::parse)
                        .filter(|r| crate::domain::images::digest(&r.version));
                    Some(Container {
                        name: name.into(),
                        image: image.into(),
                        running,
                        index,
                    })
                })
        })
        .collect()
}
pub fn image_patch(resource: &Resource, c: &Container, image: &ImageRef) -> Value {
    let path = format!("/spec/template/spec/containers/{}", c.index);
    json!([{"op":"test","path":"/metadata/uid","value":resource.uid},{"op":"test","path":"/metadata/resourceVersion","value":resource.version},{"op":"test","path":format!("{path}/name"),"value":c.name},{"op":"test","path":format!("{path}/image"),"value":c.image},{"op":"replace","path":format!("{path}/image"),"value":image.to_string()}])
}
