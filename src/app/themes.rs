//! Coordinate theme installation and workspace appearance preferences.

use super::{App, Message};
use crate::{
    domain::{
        theme::{BUILTIN_ID, Definition, ISLANDS_ID},
        workspace::Appearance,
    },
    i18n::Key,
    infrastructure::themes::{self, Error},
    ui::theme::{InstalledTheme, Theme},
};
use iced::Task;
use std::collections::BTreeMap;

#[derive(Debug, Clone)]
pub(crate) enum Event {
    Loaded(Vec<Definition>),
    Import,
    Imported(Result<Option<Box<Definition>>, Error>),
    Select(String),
}

#[derive(Default)]
pub(crate) struct State {
    installed: BTreeMap<String, InstalledTheme>,
    pub(crate) ready: bool,
    pub(crate) importing: bool,
    pub(crate) error: Option<Key>,
}

impl State {
    fn add(&mut self, definition: Definition) {
        self.installed.insert(
            definition.id.clone(),
            InstalledTheme::from_definition(definition),
        );
    }

    pub(crate) fn active(&self, id: Option<&str>, appearance: Appearance) -> Theme {
        if id == Some(ISLANDS_ID) {
            return Theme::islands(appearance);
        }
        id.and_then(|id| self.installed.get(id)).map_or_else(
            || Theme::builtin(appearance),
            |theme| theme.variant(appearance),
        )
    }

    pub(crate) fn choices(&self) -> impl Iterator<Item = (&str, &str)> {
        self.installed
            .iter()
            .map(|(id, theme)| (id.as_str(), theme.label.as_str()))
    }
}

impl App {
    pub(super) fn update_themes(&mut self, event: Event) -> Task<Message> {
        match event {
            Event::Loaded(definitions) => {
                for definition in definitions {
                    self.themes.add(definition);
                }
                self.themes.ready = true;
            }
            Event::Import if self.loaded && self.themes.ready && !self.themes.importing => {
                self.themes.importing = true;
                self.themes.error = None;
                return Task::perform(
                    themes::pick_and_install(
                        self.i18n.text(Key::ThemeImport),
                        self.i18n.text(Key::ThemeFile),
                    ),
                    |result| {
                        Message::Themes(Event::Imported(result.map(|theme| theme.map(Box::new))))
                    },
                );
            }
            Event::Imported(result) => {
                self.themes.importing = false;
                match result {
                    Ok(Some(definition)) => {
                        self.workspace.preferences.theme = Some(definition.id.clone());
                        self.themes.add(*definition);
                        return self.save();
                    }
                    Ok(None) => {}
                    Err(error) => {
                        self.themes.error = Some(match error {
                            Error::Read => Key::ThemeReadFailed,
                            Error::Invalid => Key::ThemeInvalid,
                            Error::TooMany => Key::ThemeLimit,
                            Error::Save => Key::ThemeSaveFailed,
                        });
                    }
                }
            }
            Event::Select(id) if !self.themes.importing => {
                let selected = if id == BUILTIN_ID {
                    None
                } else if id == ISLANDS_ID || self.themes.installed.contains_key(&id) {
                    Some(id)
                } else {
                    return Task::none();
                };
                self.themes.error = None;
                self.workspace.preferences.theme = selected;
                return self.save();
            }
            _ => {}
        }
        Task::none()
    }
}

#[cfg(test)]
mod tests;
