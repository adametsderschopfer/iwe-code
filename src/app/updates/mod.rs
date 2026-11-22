//! Coordinate release checks, downloads, and restart after workspace persistence.

use super::{App, Message};
use crate::{
    domain::update::{Offer, Release},
    infrastructure::updates::{self, Error, Prepared},
};
use iced::Task;
use std::{
    sync::Arc,
    time::{Duration, SystemTime},
};

const CHECK_INTERVAL: Duration = Duration::from_secs(60 * 60);
const RETRY_INTERVAL: Duration = Duration::from_secs(15 * 60);

#[derive(Default)]
pub(super) struct Schedule {
    last_attempt: Option<SystemTime>,
    retry: bool,
    checking: bool,
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum Trigger {
    Periodic,
    Focused,
}

impl Schedule {
    fn due(&self, trigger: Trigger, now: SystemTime) -> bool {
        let interval = if self.retry || matches!(trigger, Trigger::Focused) {
            RETRY_INTERVAL
        } else {
            CHECK_INTERVAL
        };
        self.last_attempt.is_none_or(|last| {
            now.duration_since(last)
                .map_or(true, |elapsed| elapsed >= interval)
        })
    }
}

#[derive(Debug, Clone, Default)]
pub(crate) enum State {
    #[default]
    Idle,
    Checking,
    Current,
    Available(Release),
    Downloading(Release),
    Ready(Arc<Prepared>),
    Saving(Arc<Prepared>),
    Installing(Arc<Prepared>),
    Failed(Error),
}

impl State {
    pub(crate) fn version(&self) -> Option<&str> {
        match self {
            Self::Available(release) | Self::Downloading(release) => Some(&release.version),
            Self::Ready(prepared) | Self::Saving(prepared) | Self::Installing(prepared) => {
                Some(&prepared.version)
            }
            _ => None,
        }
    }
}

#[derive(Default)]
pub(crate) struct Dialog {
    pub open: bool,
    pub install_after_download: bool,
    pub notes: Vec<String>,
    pub download_error: Option<crate::i18n::Key>,
    offered_version: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) enum Event {
    Check,
    Automatic(Trigger),
    Checked(Result<Option<Offer>, Error>),
    OpenDialog,
    DismissDialog,
    InstallAfterDownload(bool),
    Download,
    Downloaded(Result<Arc<Prepared>, Error>),
    Install,
    InstallFailed(Error),
    DismissNotes,
}

impl App {
    pub(crate) fn update_prompt_visible(&self) -> bool {
        self.loaded
            && self.update_dialog.open
            && self.updates.version().is_some()
            && self.whats_new.is_none()
            && self.editor.pending.is_none()
            && self.files.operations.draft.is_none()
            && self.auth_dialog.is_none()
            && self.line_navigation.is_none()
            && self.search_everywhere.dialog.is_none()
            && self.project_add.is_none()
            && self.space_draft.is_none()
            && !self.modules.git.git_actions.has_dialog()
            && !self.modules.docker.has_dialog()
            && self.shortcuts.recording.is_none()
    }

    pub(super) fn update_release(&mut self, event: Event) -> Task<Message> {
        match event {
            Event::DismissNotes => self.whats_new = None,
            Event::OpenDialog if self.updates.version().is_some() => {
                self.update_dialog.open = true;
                if let Some(id) = self.window_id {
                    return iced::window::gain_focus(id);
                }
            }
            Event::DismissDialog => {
                self.update_dialog.open = false;
                self.update_dialog.install_after_download = false;
            }
            Event::InstallAfterDownload(enabled) if matches!(self.updates, State::Available(_)) => {
                self.update_dialog.install_after_download = enabled;
            }
            Event::Automatic(trigger) => {
                if matches!(
                    self.updates,
                    State::Idle
                        | State::Current
                        | State::Available(_)
                        | State::Failed(Error::Network | Error::Invalid)
                ) && self.update_schedule.due(trigger, SystemTime::now())
                {
                    return self.update_release(Event::Check);
                }
            }
            Event::Check
                if !self.update_schedule.checking
                    && matches!(
                        self.updates,
                        State::Idle | State::Current | State::Available(_) | State::Failed(_)
                    ) =>
            {
                self.update_schedule.last_attempt = Some(SystemTime::now());
                self.update_schedule.retry = false;
                self.update_schedule.checking = true;
                if !matches!(self.updates, State::Available(_)) {
                    self.updates = State::Checking;
                }
                return Task::perform(updates::check(), |result| {
                    Message::Update(Event::Checked(result))
                });
            }
            Event::Checked(result) if self.update_schedule.checking => {
                self.update_schedule.checking = false;
                if !matches!(self.updates, State::Checking | State::Available(_)) {
                    return Task::none();
                }
                self.update_schedule.retry = result.is_err();
                match result {
                    Ok(Some(Offer { release, notes })) => {
                        if self.update_dialog.offered_version.as_ref() != Some(&release.version) {
                            self.update_dialog.offered_version = Some(release.version.clone());
                            self.update_dialog.open = true;
                            self.update_dialog.install_after_download = false;
                            self.update_dialog.download_error = None;
                        }
                        self.update_dialog.notes = notes;
                        self.updates = State::Available(release);
                    }
                    Ok(None) => {
                        self.update_dialog.open = false;
                        self.updates = State::Current;
                    }
                    Err(error) if matches!(self.updates, State::Checking) => {
                        self.updates = State::Failed(error);
                    }
                    Err(_) => {}
                }
            }
            Event::Download => {
                if let State::Available(release) = &self.updates {
                    let release = release.clone();
                    self.update_dialog.download_error = None;
                    self.updates = State::Downloading(release.clone());
                    return Task::perform(updates::download(release), |result| {
                        Message::Update(Event::Downloaded(result))
                    });
                }
            }
            Event::Downloaded(result) => {
                let State::Downloading(release) = &self.updates else {
                    return Task::none();
                };
                let install = std::mem::take(&mut self.update_dialog.install_after_download);
                match result {
                    Ok(prepared) => {
                        self.updates = State::Ready(prepared);
                        if install {
                            // Route through the same file and editor guards as a manual install.
                            return Task::done(Message::Update(Event::Install));
                        }
                    }
                    Err(error) => {
                        self.updates = State::Available(release.clone());
                        let key = error_key(error);
                        self.update_dialog.download_error = Some(key);
                        if !self.update_dialog.open {
                            self.notifications.error(key, None);
                        }
                    }
                }
            }
            Event::Install => {
                if let State::Ready(prepared) = &self.updates {
                    if self.save_blocked || self.window_id.is_none() {
                        self.update_dialog.open = false;
                        self.notifications.error(error_key(Error::Install), None);
                        self.updates = State::Failed(Error::Install);
                    } else {
                        self.updates = State::Saving(Arc::clone(prepared));
                        return self.save();
                    }
                }
            }
            Event::InstallFailed(error) if matches!(self.updates, State::Installing(_)) => {
                self.update_dialog.open = false;
                self.notifications.error(error_key(error), None);
                self.updates = State::Failed(error);
            }
            _ => {}
        }
        Task::none()
    }

    pub(super) fn restart_after_save(&mut self, success: bool) -> Task<Message> {
        let State::Saving(prepared) = &self.updates else {
            return Task::none();
        };
        if !success {
            self.update_dialog.open = false;
            self.updates = State::Failed(Error::Install);
            return Task::none();
        }
        let prepared = Arc::clone(prepared);
        self.updates = State::Installing(Arc::clone(&prepared));
        let terminals = self.modules.terminal.lifetime.clone();
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || {
                    updates::install_and_exit(&prepared, || {
                        if let Some(stopped) = terminals.shutdown() {
                            iced::futures::executor::block_on(stopped);
                        }
                    })
                })
                .await
                .unwrap_or(Error::Install)
            },
            |error| Message::Update(Event::InstallFailed(error)),
        )
    }
}

pub(super) fn restarted_after_update() -> bool {
    std::env::args_os().nth(1).as_deref() == Some("--iwe-update-cleanup".as_ref())
        && std::env::var_os("IWE_UPDATE_FAILED").as_deref() == Some("0".as_ref())
}

#[derive(serde::Deserialize)]
pub(crate) struct WhatsNew {
    pub version: String,
    items: Vec<Note>,
}

#[derive(serde::Deserialize)]
struct Note {
    #[serde(default)]
    platforms: Vec<String>,
    text: std::collections::BTreeMap<String, String>,
}

impl WhatsNew {
    pub(super) fn current() -> Self {
        let version = env!("CARGO_PKG_VERSION");
        let ledger = include_str!("../../../website/app/release-notes/releases.json");
        Self::from_ledger(version, ledger)
    }

    fn from_ledger(version: &str, ledger: &str) -> Self {
        serde_json::from_str::<Vec<Self>>(ledger)
            .unwrap_or_default()
            .into_iter()
            .find(|entry| entry.version == version)
            .unwrap_or_else(|| Self {
                version: version.to_owned(),
                items: Vec::new(),
            })
    }

    pub(crate) fn url(&self) -> String {
        format!("https://iwe-code.ru/docs/release-notes/{}/", self.version)
    }

    pub(crate) fn notes<'a>(
        &'a self,
        language: crate::i18n::Language,
        platform: &'a str,
    ) -> impl Iterator<Item = &'a str> {
        self.items
            .iter()
            .filter(move |item| {
                item.platforms.is_empty() || item.platforms.iter().any(|value| value == platform)
            })
            .filter_map(move |item| {
                item.text
                    .get(language.code())
                    .or_else(|| item.text.get("en"))
                    .map(String::as_str)
            })
    }
}

#[cfg(test)]
mod tests;

pub(crate) fn error_key(error: Error) -> crate::i18n::Key {
    use crate::i18n::Key;
    match error {
        Error::Invalid => Key::UpdateInvalid,
        Error::Install => Key::UpdateInstallFailed,
        Error::Unsupported => Key::UpdateUnsupported,
        Error::Network => Key::UpdateCheckFailed,
    }
}
