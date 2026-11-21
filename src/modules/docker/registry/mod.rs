//! Browse distribution endpoints independently of Docker Engine.
mod api;
mod credentials;
mod view;
use super::{adapter, model::Item};
use crate::{
    domain::images::{ImageRef, Registry, repository, tag},
    i18n::Key,
};
use api::{Error, Manifest, Page};
use iced::Task;
use std::path::Path;
pub(super) use view::{edit_overlay, panel};

#[derive(Debug, Clone)]
pub(crate) enum Event {
    Open,
    Select(Registry),
    Edit(bool),
    Name(String),
    Endpoint(String),
    Issuer(String),
    Login(bool),
    Save,
    Remove,
    Check,
    RepoInput(String),
    VersionInput(String),
    Repository(String),
    Version(String),
    Catalog,
    Next,
    Previous,
    Refresh,
    Pull(bool),
    Publish(Item),
    Review,
    Confirm,
    Copy(ImageRef),
    Choose(bool),
    Chosen(ImageRef),
    Cancel,
    Return,
    Find(ImageRef),
    Pick(Option<ImageRef>),
    Tags(u64, Result<Page, Error>),
    Listed(u64, Result<Page, Error>),
    Inspected(u64, Result<Manifest, Error>),
    Checked(u64, Result<(), Error>),
    Transferred(u64, Result<(), Key>),
}
#[derive(Debug, Clone)]
enum Mode {
    Browse,
    Pick,
    Publish(Item),
}
#[derive(Clone)]
enum Form {
    Edit(Editor),
    Remove,
    Transfer { context: String, target: ImageRef },
}
#[derive(Clone)]
pub(super) struct Editor {
    existing: Option<String>,
    name: String,
    endpoint: String,
    issuer: String,
    login: bool,
    error: Option<Key>,
}
pub(crate) struct State {
    revision: u64,
    pub selected: Option<Registry>,
    mode: Mode,
    form: Option<Form>,
    pending: Option<ImageRef>,
    repo: String,
    input: String,
    version: String,
    version_input: String,
    page: Page,
    history: Vec<Option<String>>,
    manifest: Option<Manifest>,
    busy: bool,
    transferring: bool,
    notice: Option<Key>,
    published: Option<ImageRef>,
}
impl Default for State {
    fn default() -> Self {
        Self {
            revision: 0,
            selected: None,
            mode: Mode::Browse,
            form: None,
            pending: None,
            repo: String::new(),
            input: String::new(),
            version: String::new(),
            version_input: String::new(),
            page: Default::default(),
            history: vec![None],
            manifest: None,
            busy: false,
            transferring: false,
            notice: None,
            published: None,
        }
    }
}
impl Error {
    fn key(self) -> Key {
        match self {
            Self::Denied => Key::RegistryDenied,
            Self::Login => Key::RegistryLoginError,
            Self::Missing => Key::RegistryMissing,
            Self::Invalid => Key::DockerInvalid,
            Self::Network => Key::RegistryOffline,
            Self::Limit => Key::DockerLarge,
        }
    }
}
impl State {
    pub fn reset(&mut self) {
        *self = Self {
            revision: self.revision + 1,
            ..Default::default()
        }
    }
    fn begin(&mut self) -> u64 {
        self.revision += 1;
        self.transferring = false;
        self.busy = true;
        self.notice = None;
        self.revision
    }
    fn clear_page(&mut self) {
        self.page = Default::default();
        self.version.clear();
        self.manifest = None;
        self.history = vec![None];
    }
    fn list(&mut self) -> Task<Event> {
        let Some(reg) = self.selected.clone() else {
            return Task::none();
        };
        self.manifest = None;
        self.version.clear();
        self.page = Default::default();
        let repo = (!self.repo.is_empty()).then(|| self.repo.clone());
        let cursor = self.history.last().cloned().flatten();
        let id = self.begin();
        Task::perform(
            async move { api::list(&reg, repo.as_deref(), cursor.as_deref()).await },
            move |r| Event::Listed(id, r),
        )
    }
    fn inspect(&mut self, version: String) -> Task<Event> {
        let Some(reg) = self.selected.clone() else {
            return Task::none();
        };
        if reg.image(&self.repo, &version).is_none() {
            self.notice = Some(Key::DockerInvalid);
            return Task::none();
        }
        self.version = version.clone();
        self.version_input = version.clone();
        self.manifest = None;
        let repo = self.repo.clone();
        let id = self.begin();
        Task::perform(
            async move { api::manifest(&reg, &repo, &version).await },
            move |r| Event::Inspected(id, r),
        )
    }
    fn select(&mut self, reg: Registry) -> Task<Event> {
        self.selected = Some(reg);
        self.form = None;
        self.repo.clear();
        self.input.clear();
        self.version_input.clear();
        self.published = None;
        self.clear_page();
        self.list()
    }
    fn find(&mut self, image: ImageRef, profiles: &[Registry]) -> Task<Event> {
        if let Some(reg) = profiles.iter().find(|r| r.host() == image.host) {
            self.selected = Some(reg.clone());
            self.form = None;
            self.clear_page();
            self.repo = image.repository.clone();
            self.input = image.repository;
            let repo = self.repo.clone();
            let registry = reg.clone();
            let inspect = self.inspect(image.version);
            let id = self.revision;
            return Task::batch([
                inspect,
                Task::perform(
                    async move { api::list(&registry, Some(&repo), None).await },
                    move |r| Event::Tags(id, r),
                ),
            ]);
        }
        self.revision += 1;
        self.busy = false;
        self.pending = Some(image.clone());
        self.selected = None;
        self.form = Some(Form::Edit(Editor {
            error: None,
            existing: None,
            name: image.host.clone(),
            endpoint: format!("https://{}", image.host),
            issuer: String::new(),
            login: false,
        }));
        Task::none()
    }
    pub(super) fn editor(&self) -> Option<&Editor> {
        match &self.form {
            Some(Form::Edit(editor)) => Some(editor),
            _ => None,
        }
    }
    fn image(&self, pin: bool) -> Option<ImageRef> {
        let m = self.manifest.as_ref()?;
        self.selected
            .as_ref()?
            .image(&self.repo, if pin { &m.digest } else { &self.version })
    }
    pub fn engine_changed(&mut self) {
        if self.transferring
            || matches!(self.mode, Mode::Publish(_))
            || matches!(self.form, Some(Form::Transfer { .. }))
        {
            self.reset();
        }
    }
    pub fn picking(&self) -> bool {
        matches!(self.mode, Mode::Pick)
    }
    pub fn finish_pick(&mut self) {
        self.mode = Mode::Browse
    }
    pub fn update(
        &mut self,
        e: Event,
        profiles: &mut Vec<Registry>,
        engine: Option<&str>,
        root: &Path,
    ) -> Task<Event> {
        match e {
            Event::Open => {
                self.mode = Mode::Browse;
                self.pending = None;
                self.form = None;
                let reg = self
                    .selected
                    .as_ref()
                    .and_then(|s| profiles.iter().find(|r| r.endpoint == s.endpoint))
                    .or(profiles.first())
                    .cloned();
                if let Some(r) = reg {
                    return self.select(r);
                }
                self.reset();
            }
            Event::Find(image) => {
                self.mode = Mode::Browse;
                return self.find(image, profiles);
            }
            Event::Pick(image) => {
                self.mode = Mode::Pick;
                self.form = None;
                if let Some(image) = image {
                    return self.find(image, profiles);
                }
                if let Some(reg) = profiles.first() {
                    return self.select(reg.clone());
                }
            }
            Event::Select(reg) if profiles.contains(&reg) => {
                self.pending = None;
                if matches!(self.mode, Mode::Publish(_)) {
                    self.revision += 1;
                    self.busy = false;
                    self.selected = Some(reg);
                    self.form = None;
                    self.notice = None;
                    return Task::none();
                }
                return self.select(reg);
            }
            Event::Edit(edit) => {
                let old = edit.then(|| self.selected.clone()).flatten();
                self.form = Some(Form::Edit(Editor {
                    error: None,
                    existing: old.as_ref().map(|r| r.endpoint.clone()),
                    name: old.as_ref().map_or_else(String::new, |r| r.name.clone()),
                    endpoint: old
                        .as_ref()
                        .map_or_else(String::new, |r| r.endpoint.clone()),
                    issuer: old
                        .as_ref()
                        .map_or_else(String::new, |r| r.auth_origin.clone()),
                    login: old.is_some_and(|r| r.credentials),
                }));
            }
            Event::Name(v) => {
                if let Some(Form::Edit(editor)) = &mut self.form {
                    editor.name = v
                }
            }
            Event::Endpoint(v) => {
                if let Some(Form::Edit(editor)) = &mut self.form {
                    editor.endpoint = v
                }
            }
            Event::Issuer(v) => {
                if let Some(Form::Edit(editor)) = &mut self.form {
                    editor.issuer = v
                }
            }
            Event::Login(v) => {
                if let Some(Form::Edit(editor)) = &mut self.form {
                    editor.login = v
                }
            }
            Event::Save => {
                if let Some(Form::Edit(Editor {
                    error,
                    existing,
                    name,
                    endpoint,
                    issuer,
                    login,
                })) = &mut self.form
                {
                    let Some(mut reg) = Registry::new(name, endpoint, *login) else {
                        *error = Some(Key::DockerInvalid);
                        return Task::none();
                    };
                    if !issuer.trim().is_empty() {
                        let Some(origin) = Registry::new("issuer", issuer, true) else {
                            *error = Some(Key::DockerInvalid);
                            return Task::none();
                        };
                        reg.auth_origin = origin.endpoint;
                    }
                    if profiles
                        .iter()
                        .any(|r| r.host() == reg.host() && Some(&r.endpoint) != existing.as_ref())
                    {
                        *error = Some(Key::RegistryDuplicate);
                        return Task::none();
                    }
                    if let Some(pos) = profiles
                        .iter()
                        .position(|r| Some(&r.endpoint) == existing.as_ref())
                    {
                        profiles[pos] = reg.clone()
                    } else {
                        profiles.push(reg.clone())
                    }
                    self.form = None;
                    if let Some(image) = self.pending.take().filter(|r| r.host == reg.host()) {
                        return self.find(image, profiles);
                    }
                    return self.select(reg);
                }
            }
            Event::Remove if self.selected.is_some() => self.form = Some(Form::Remove),
            Event::Confirm if matches!(self.form, Some(Form::Remove)) => {
                profiles.retain(|r| Some(r) != self.selected.as_ref());
                self.reset();
                if let Some(r) = profiles.first() {
                    return self.select(r.clone());
                }
            }
            Event::Check => {
                if let Some(r) = self.selected.clone() {
                    let id = self.begin();
                    return Task::perform(async move { api::check(&r).await }, move |r| {
                        Event::Checked(id, r)
                    });
                }
            }
            Event::Checked(id, r) if id == self.revision => {
                self.busy = false;
                self.notice = Some(r.map_or_else(Error::key, |()| Key::RegistryConnected))
            }
            Event::RepoInput(v) => self.input = v,
            Event::VersionInput(v) => self.version_input = v,
            Event::Catalog => {
                self.repo.clear();
                self.input.clear();
                self.clear_page();
                return self.list();
            }
            Event::Repository(v) if repository(&v) => {
                self.input = v.clone();
                self.repo = v;
                self.version_input.clear();
                self.clear_page();
                return self.list();
            }
            Event::Version(v) => return self.inspect(v),
            Event::Next => {
                if let Some(next) = self
                    .page
                    .next
                    .clone()
                    .filter(|s| !self.history.iter().flatten().any(|c| c == s))
                {
                    self.history.push(Some(next));
                    return self.list();
                }
            }
            Event::Previous if self.history.len() > 1 => {
                self.history.pop();
                return self.list();
            }
            Event::Refresh if !self.version.is_empty() => {
                return self.inspect(self.version.clone());
            }
            Event::Refresh => return self.list(),
            Event::Tags(id, Ok(p)) if id == self.revision => self.page = p,
            Event::Listed(id, r) if id == self.revision => {
                self.busy = false;
                match r {
                    Ok(p) => self.page = p,
                    Err(e) => {
                        self.notice = Some(
                            if self.repo.is_empty() && matches!(e, Error::Denied | Error::Missing) {
                                Key::RegistryRestricted
                            } else {
                                e.key()
                            },
                        )
                    }
                }
            }
            Event::Inspected(id, r) if id == self.revision => {
                self.busy = false;
                match r {
                    Ok(m) => self.manifest = Some(m),
                    Err(e) => self.notice = Some(e.key()),
                }
            }
            Event::Publish(item) => {
                self.mode = Mode::Publish(item.clone());
                self.form = None;
                self.pending = None;
                self.selected = self.selected.clone().or_else(|| profiles.first().cloned());
                self.clear_page();
                self.revision += 1;
                self.busy = false;
                self.notice = None;
                self.published = None;
                self.input = String::new();
                self.version_input = "latest".into();
                if let Some(img) = ImageRef::parse(&item.id) {
                    self.input = img.repository;
                    self.version_input = img.version;
                }
            }
            Event::Review => {
                if let (Mode::Publish(_), Some(reg), Some(cx)) =
                    (&self.mode, &self.selected, engine)
                    && tag(&self.version_input)
                    && let Some(target) = reg.image(&self.input, &self.version_input)
                {
                    self.form = Some(Form::Transfer {
                        context: cx.into(),
                        target,
                    })
                }
            }
            Event::Pull(pin) => {
                if let (Some(image), Some(cx)) = (self.image(pin), engine) {
                    self.form = Some(Form::Transfer {
                        context: cx.into(),
                        target: image,
                    })
                }
            }
            Event::Confirm => {
                if let Some(Form::Transfer { context, target }) = self.form.take() {
                    if Some(context.as_str()) != engine {
                        self.notice = Some(Key::RegistryEngine);
                        return Task::none();
                    }
                    let source = if let Mode::Publish(s) = &self.mode {
                        Some(s.detail.clone())
                    } else {
                        None
                    };
                    let dest = target.to_string();
                    let root = root.to_owned();
                    self.published = None;
                    if source.is_some() {
                        self.pending = Some(target)
                    }
                    let id = self.begin();
                    self.transferring = true;
                    return Task::perform(
                        async move {
                            if let Some(src) = source {
                                adapter::run(
                                    Some(&context),
                                    &["image".into(), "tag".into(), "--".into(), src, dest.clone()],
                                    &root,
                                    true,
                                    false,
                                )
                                .await
                                .map_err(super::view::error_key)?;
                                adapter::run(
                                    Some(&context),
                                    &["image".into(), "push".into(), "--".into(), dest],
                                    &root,
                                    true,
                                    false,
                                )
                                .await
                            } else {
                                adapter::run(
                                    Some(&context),
                                    &["image".into(), "pull".into(), "--".into(), dest],
                                    &root,
                                    true,
                                    false,
                                )
                                .await
                            }
                            .map(|_| ())
                            .map_err(super::view::error_key)
                        },
                        move |r| Event::Transferred(id, r),
                    );
                }
            }
            Event::Transferred(id, r) if id == self.revision => {
                self.transferring = false;
                self.busy = false;
                match r {
                    Err(k) => {
                        self.notice = Some(k);
                        self.pending = None
                    }
                    Ok(()) => {
                        if let Some(image) = self.pending.take() {
                            self.mode = Mode::Browse;
                            self.repo = image.repository.clone();
                            self.input = self.repo.clone();
                            self.clear_page();
                            self.published = Some(image);
                            return self.list();
                        }
                        self.notice = Some(Key::RegistryPulled)
                    }
                }
            }
            Event::Choose(pin) if self.picking() => {
                if let Some(i) = self.image(pin) {
                    return Task::done(Event::Chosen(i));
                }
            }
            Event::Copy(image) => return iced::clipboard::write(image.to_string()),
            Event::Cancel => {
                if self.form.take().is_some() {
                    self.pending = None;
                    self.notice = None
                } else {
                    if self.picking() {
                        return Task::done(Event::Return);
                    }
                    self.mode = Mode::Browse;
                    return self.list();
                }
            }
            _ => {}
        }
        Task::none()
    }
}
#[cfg(test)]
mod tests;
