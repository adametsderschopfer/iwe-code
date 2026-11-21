//! Independent cluster browser. All cluster requests carry an explicit context.
mod api;
mod manifest;
mod model;
mod view;
use crate::{domain::images::ImageRef, i18n::Key};
use api::{Details, StreamUpdate, Target, words};
use iced::Task;
use model::{Kind, Resource};
use std::{collections::BTreeMap, path::PathBuf, sync::Arc};
pub(crate) use view::panel;

type Contexts = (Vec<(String, String)>, String);
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Operation {
    Scale,
    Restart,
    Image,
    Forward,
}
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum Tab {
    #[default]
    Properties,
    Events,
    Yaml,
}
#[derive(Debug, Clone)]
pub(crate) enum Event {
    Open,
    Hide,
    Refresh,
    Config,
    Standard,
    ConfigFile(u64, Option<PathBuf>),
    Contexts(u64, Result<Contexts, Key>),
    Context(String),
    Namespace(String),
    NamespaceInput(String),
    Kind(Kind),
    Listed(u64, Result<Vec<Resource>, Key>),
    Namespaces(u64, Result<Vec<String>, Key>),
    Select(Resource),
    Details(u64, Result<Arc<Details>, Key>),
    Tab(Tab),
    Page(usize),
    Container(String),
    Related,
    Logs,
    Shell,
    Rollout,
    Begin(Operation),
    Input(String),
    Remote(String),
    Review,
    Confirm,
    Cancel,
    PickRegistry(Option<ImageRef>),
    OpenRegistry(ImageRef),
    Picked(ImageRef),
    Manifest,
    ManifestFile(u64, Option<PathBuf>),
    Planned(u64, Result<Arc<manifest::Plan>, Key>),
    Completed(u64, Result<(), Key>),
    Status(u64, Result<String, Key>),
    PodIssues(u64, Result<String, Key>),
    Stream(u64, StreamUpdate),
    CloseSession(u64),
}
struct Draft {
    op: Operation,
    input: String,
    remote: String,
    review: bool,
}
struct Session {
    label: String,
    text: String,
    forward: bool,
    ready: bool,
    ended: bool,
    error: Option<Key>,
    _handle: iced::task::Handle,
}
#[derive(Default)]
pub(crate) struct State {
    pub open: bool,
    pub terminal_tabs: Vec<u64>,
    epoch: u64,
    scope: u64,
    target: Target,
    contexts: Vec<(String, String)>,
    namespaces: Vec<String>,
    ns_input: String,
    kind: Kind,
    rows: Vec<Resource>,
    page: usize,
    parent: Option<Resource>,
    selected: Option<Resource>,
    details: Option<Arc<Details>>,
    container: String,
    tab: Tab,
    busy: bool,
    notice: Option<Key>,
    status: String,
    draft: Option<Draft>,
    plan: Option<Arc<manifest::Plan>>,
    sessions: BTreeMap<u64, Session>,
    sequence: u64,
    picking: bool,
}
impl State {
    pub fn reset(&mut self) {
        *self = Self {
            epoch: self.epoch + 1,
            scope: self.scope + 1,
            sequence: self.sequence,
            ..Default::default()
        };
    }
    pub fn scope_id(&self) -> u64 {
        self.scope
    }
    pub fn picking(&self) -> bool {
        self.picking
            && self
                .draft
                .as_ref()
                .is_some_and(|d| d.op == Operation::Image)
    }
    pub fn finish_pick(&mut self) {
        self.picking = false
    }
    pub fn choose(&mut self) -> bool {
        self.picking = self
            .draft
            .as_ref()
            .is_some_and(|d| d.op == Operation::Image && !d.review);
        self.picking
    }
    fn begin(&mut self) -> u64 {
        self.epoch += 1;
        self.busy = true;
        self.notice = None;
        self.status.clear();
        self.epoch
    }
    fn clear_details(&mut self) {
        self.details = None;
        self.selected = None;
        self.draft = None;
        self.plan = None;
        self.picking = false;
        self.container.clear();
        self.tab = Tab::Properties
    }
    fn list(&mut self) -> Task<Event> {
        self.clear_details();
        self.page = 0;
        self.rows.clear();
        let id = self.begin();
        if self.target.context.is_empty() {
            self.busy = false;
            return Task::none();
        }
        let target = self.target.clone();
        let kind = self.kind;
        let parent = self.parent.clone();
        Task::perform(
            async move { api::list(&target, kind, parent.as_ref()).await },
            move |r| Event::Listed(id, r),
        )
    }
    fn contexts(&mut self) -> Task<Event> {
        self.scope += 1;
        self.sessions.clear();
        self.parent = None;
        self.target.context.clear();
        self.namespaces.clear();
        self.contexts.clear();
        self.rows.clear();
        self.clear_details();
        let file = self.target.file.clone();
        let id = self.begin();
        Task::perform(api::contexts(file), move |r| Event::Contexts(id, r))
    }
    fn container(&self) -> Option<&model::Container> {
        self.details
            .as_ref()?
            .containers
            .iter()
            .find(|c| c.name == self.container)
    }
    pub fn shell(&self) -> Option<Vec<String>> {
        if self.kind != Kind::Pod {
            return None;
        }
        let r = self.selected.as_ref()?;
        let mut a = self.target.flags(&r.ns);
        a.extend(words(&[
            "exec",
            "-it",
            &r.name,
            "-c",
            &self.container()?.name,
            "--",
            "sh",
        ]));
        Some(a)
    }
    fn rollout(&mut self) -> Task<Event> {
        let Some(r) = self
            .selected
            .clone()
            .filter(|_| self.kind == Kind::Deployment)
        else {
            return Task::none();
        };
        let id = self.begin();
        let t = self.target.clone();
        Task::perform(
            async move {
                api::run(
                    &t,
                    &r.ns,
                    &words(&[
                        "rollout",
                        "status",
                        &format!("deployment/{}", r.name),
                        "--timeout=35s",
                    ]),
                    None,
                )
                .await
                .map(|v| String::from_utf8_lossy(&v).trim().to_owned())
            },
            move |v| Event::Status(id, v),
        )
    }
    fn start_session(&mut self, args: Vec<String>, forward: bool, label: String) -> Task<Event> {
        let Some(r) = &self.selected else {
            return Task::none();
        };
        self.sequence += 1;
        let id = self.sequence;
        let (task, handle) =
            api::stream(self.target.clone(), r.ns.clone(), args, forward, id).abortable();
        self.sessions.insert(
            id,
            Session {
                label,
                text: String::new(),
                forward,
                ready: false,
                ended: false,
                error: None,
                _handle: handle.abort_on_drop(),
            },
        );
        task
    }
    fn valid(&self, d: &Draft) -> bool {
        match d.op {
            Operation::Scale => d.input.parse::<u32>().is_ok(),
            Operation::Restart => true,
            Operation::Image => {
                ImageRef::parse(&d.input).is_some()
                    && self.container().is_some_and(|c| d.input != c.image)
            }
            Operation::Forward => [&d.input, &d.remote]
                .iter()
                .all(|v| v.parse::<u16>().is_ok_and(|p| p > 0)),
        }
    }
    fn confirm(&mut self) -> Task<Event> {
        if self.busy {
            return Task::none();
        }
        if let Some(plan) = self.plan.take() {
            let id = self.begin();
            let t = self.target.clone();
            return Task::perform(async move { manifest::apply(&t, &plan).await }, move |r| {
                Event::Completed(id, r)
            });
        }
        let Some(d) = self.draft.take() else {
            return Task::none();
        };
        if !d.review || !self.valid(&d) {
            return Task::none();
        }
        let Some(r) = self.selected.clone() else {
            return Task::none();
        };
        let t = self.target.clone();
        let path = format!("{}/{}", self.kind.api(), r.name);
        let args = match d.op {
            Operation::Forward => {
                let ports = format!("{}:{}", d.input, d.remote);
                return self.start_session(
                    words(&["port-forward", "--address=127.0.0.1", &path, &ports]),
                    true,
                    format!("{} · {} · {path} · 127.0.0.1:{ports}", t.context, r.ns),
                );
            }
            Operation::Scale => words(&[
                "scale",
                &path,
                &format!("--replicas={}", d.input),
                &format!("--resource-version={}", r.version),
            ]),
            Operation::Restart => {
                let timestamp = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_nanos()
                    .to_string();
                let patch = serde_json::json!({"metadata":{"uid":r.uid,"resourceVersion":r.version},"spec":{"template":{"metadata":{"annotations":{"iwe.dev/restart":timestamp}}}}});
                words(&[
                    "patch",
                    &path,
                    "--type=merge",
                    "--patch",
                    &patch.to_string(),
                ])
            }
            Operation::Image => {
                let Some(c) = self.container() else {
                    return Task::none();
                };
                let Some(i) = ImageRef::parse(&d.input) else {
                    return Task::none();
                };
                words(&[
                    "patch",
                    &path,
                    "--type=json",
                    "--patch",
                    &model::image_patch(&r, c, &i).to_string(),
                ])
            }
        };
        let id = self.begin();
        Task::perform(
            async move { api::run(&t, &r.ns, &args, None).await.map(|_| ()) },
            move |r| Event::Completed(id, r),
        )
    }
    pub fn update(&mut self, event: Event) -> Task<Event> {
        match event {
            Event::Open if self.target.context.is_empty() => return self.contexts(),
            Event::Refresh if self.target.context.is_empty() => return self.contexts(),
            Event::Refresh => return self.list(),
            Event::Config => {
                let id = self.epoch;
                return Task::perform(
                    async {
                        rfd::AsyncFileDialog::new()
                            .pick_file()
                            .await
                            .map(|f| f.path().to_owned())
                    },
                    move |f| Event::ConfigFile(id, f),
                );
            }
            Event::ConfigFile(id, Some(f)) if id == self.epoch => {
                self.target.file = Some(f);
                return self.contexts();
            }
            Event::Standard => {
                self.target.file = None;
                return self.contexts();
            }
            Event::Contexts(id, r) if id == self.epoch => {
                self.busy = false;
                match r {
                    Err(k) => self.notice = Some(k),
                    Ok((cx, current)) => {
                        self.contexts = cx;
                        if let Some(c) = self
                            .contexts
                            .iter()
                            .find(|(c, _)| *c == current)
                            .or(self.contexts.first())
                        {
                            return self.update(Event::Context(c.0.clone()));
                        }
                        self.notice = Some(Key::KubeNoContexts)
                    }
                }
            }
            Event::Context(c) => {
                let Some((_, ns)) = self.contexts.iter().find(|(name, _)| *name == c) else {
                    return Task::none();
                };
                self.target.namespace = ns.clone();
                self.ns_input = ns.clone();
                self.target.context = c;
                self.parent = None;
                self.sessions.clear();
                self.namespaces.clear();
                self.scope += 1;
                let scope = self.scope;
                let t = self.target.clone();
                return Task::batch([
                    self.list(),
                    Task::perform(
                        async move {
                            api::json(&t, "", &["get", "namespaces", "-o", "json"])
                                .await
                                .map(|v| {
                                    v["items"]
                                        .as_array()
                                        .into_iter()
                                        .flatten()
                                        .filter_map(|v| {
                                            v["metadata"]["name"].as_str().map(str::to_owned)
                                        })
                                        .collect()
                                })
                        },
                        move |r| Event::Namespaces(scope, r),
                    ),
                ]);
            }
            Event::Namespaces(id, Ok(v)) if id == self.scope => self.namespaces = v,
            Event::NamespaceInput(v) => self.ns_input = v,
            Event::Namespace(v) if v.is_empty() || valid_namespace(&v) => {
                self.ns_input = v.clone();
                self.target.namespace = v;
                self.parent = None;
                return self.list();
            }
            Event::Kind(k) => {
                self.kind = k;
                self.parent = None;
                return self.list();
            }
            Event::Listed(id, r) if id == self.epoch => {
                self.busy = false;
                match r {
                    Ok(rows) => self.rows = rows,
                    Err(k) => self.notice = Some(k),
                }
            }
            Event::Page(p) => self.page = p.min(self.rows.len().saturating_sub(1) / 50),
            Event::Select(r) if self.rows.contains(&r) => {
                self.clear_details();
                self.selected = Some(r.clone());
                let id = self.begin();
                let t = self.target.clone();
                let k = self.kind;
                return Task::perform(
                    async move { api::details(&t, k, &r).await.map(Arc::new) },
                    move |r| Event::Details(id, r),
                );
            }
            Event::Details(id, r) if id == self.epoch => {
                self.busy = false;
                match r {
                    Ok(d) => {
                        if !d.containers.iter().any(|c| c.name == self.container) {
                            self.container = d
                                .containers
                                .first()
                                .map(|c| c.name.clone())
                                .unwrap_or_default();
                        }
                        if let Some(r) = &mut self.selected {
                            r.version = d.version.clone()
                        }
                        self.details = Some(d)
                    }
                    Err(k) => self.notice = Some(k),
                }
            }
            Event::Tab(tab) => self.tab = tab,
            Event::Container(c)
                if self.draft.is_none()
                    && self
                        .details
                        .as_ref()
                        .is_some_and(|d| d.containers.iter().any(|v| v.name == c)) =>
            {
                self.container = c
            }
            Event::Related if self.kind == Kind::Deployment => {
                self.parent = self.selected.clone();
                self.kind = Kind::Pod;
                return self.list();
            }
            Event::Rollout => return self.rollout(),
            Event::Logs if self.kind == Kind::Pod => {
                if let (Some(r), Some(c)) = (&self.selected, self.container()) {
                    let args = words(&["logs", "--follow", "--tail=200", &r.name, "-c", &c.name]);
                    return self.start_session(
                        args,
                        false,
                        format!("{}/{} · {}", r.ns, r.name, c.name),
                    );
                }
            }
            Event::Begin(op) if self.details.is_some() && !self.busy => {
                let d = self.details.as_ref().expect("details");
                self.draft = Some(Draft {
                    op,
                    input: match op {
                        Operation::Scale => d.replicas.to_string(),
                        Operation::Image => self
                            .container()
                            .map(|c| c.image.clone())
                            .unwrap_or_default(),
                        Operation::Forward => {
                            d.ports.first().map(u16::to_string).unwrap_or_default()
                        }
                        Operation::Restart => String::new(),
                    },
                    remote: d.ports.first().map(u16::to_string).unwrap_or_default(),
                    review: op == Operation::Restart,
                })
            }
            Event::Input(v) => {
                if let Some(d) = &mut self.draft
                    && !d.review
                {
                    d.input = v
                }
            }
            Event::Remote(v) => {
                if let Some(d) = &mut self.draft
                    && !d.review
                {
                    d.remote = v
                }
            }
            Event::Review => {
                if self.draft.as_ref().is_some_and(|d| self.valid(d))
                    && let Some(d) = &mut self.draft
                {
                    d.review = true
                }
            }
            Event::Confirm => return self.confirm(),
            Event::Cancel => {
                self.draft = None;
                self.plan = None;
                self.picking = false
            }
            Event::Picked(i) if self.picking() => {
                self.picking = false;
                return self.update(Event::Input(i.to_string()));
            }
            Event::Manifest if !self.target.context.is_empty() => {
                let id = self.epoch;
                return Task::perform(
                    async {
                        rfd::AsyncFileDialog::new()
                            .add_filter("YAML", &["yaml", "yml", "json"])
                            .pick_file()
                            .await
                            .map(|f| f.path().to_owned())
                    },
                    move |f| Event::ManifestFile(id, f),
                );
            }
            Event::ManifestFile(id, Some(f)) if id == self.epoch => {
                self.clear_details();
                let id = self.begin();
                let t = self.target.clone();
                return Task::perform(
                    async move { manifest::preview(&t, f).await.map(Arc::new) },
                    move |r| Event::Planned(id, r),
                );
            }
            Event::Planned(id, r) if id == self.epoch => {
                self.busy = false;
                match r {
                    Ok(p) => self.plan = Some(p),
                    Err(k) => self.notice = Some(k),
                }
            }
            Event::Completed(id, r) if id == self.epoch => {
                self.busy = false;
                match r {
                    Ok(()) if self.selected.is_some() => return self.rollout(),
                    Ok(()) => return self.list(),
                    Err(k) => self.notice = Some(k),
                }
            }
            Event::Status(id, r) if id == self.epoch => {
                self.busy = false;
                let Some(resource) = self.selected.clone() else {
                    return Task::none();
                };
                let target = self.target.clone();
                match r {
                    Ok(v) => {
                        self.status = v;
                        self.busy = true;
                        return Task::perform(
                            async move {
                                api::details(&target, Kind::Deployment, &resource)
                                    .await
                                    .map(Arc::new)
                            },
                            move |r| Event::Details(id, r),
                        );
                    }
                    Err(k) => {
                        self.notice = Some(k);
                        return Task::perform(
                            async move { api::pull_failures(&target, &resource).await },
                            move |result| Event::PodIssues(id, result),
                        );
                    }
                }
            }
            Event::PodIssues(id, Ok(text)) if id == self.epoch => self.status = text,
            Event::CloseSession(id) => {
                self.sessions.remove(&id);
            }
            Event::Stream(id, update) => {
                if let Some(s) = self.sessions.get_mut(&id) {
                    match update {
                        StreamUpdate::Listening => s.ready = true,
                        StreamUpdate::End(r) => {
                            s.ended = true;
                            s.error = r.err()
                        }
                        StreamUpdate::Data(v) => {
                            s.text.push_str(&v);
                            if s.text.len() > 131072 {
                                let mut start = s.text.len() - 131072;
                                while !s.text.is_char_boundary(start) {
                                    start += 1;
                                }
                                s.text.drain(..start);
                            }
                        }
                    }
                }
            }
            _ => {}
        }
        Task::none()
    }
}
fn valid_namespace(v: &str) -> bool {
    !v.is_empty()
        && v.len() <= 63
        && !v.starts_with('-')
        && !v.ends_with('-')
        && v.bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}
#[cfg(test)]
mod tests;
