//! Keep automatic names stable for the lifetime of each tab.

use super::{Event, FocusTarget, State, Tab};
use crate::i18n::{FluentArgs, I18n, Key};
use iced::{Point, Task};
use std::{borrow::Cow, collections::HashSet};

pub(crate) struct TabTitle {
    automatic: Automatic,
    custom: Option<String>,
    pub control: TitleControl,
}

enum Automatic {
    Local(u64),
    Named(String),
}

#[derive(Default)]
pub(crate) enum TitleControl {
    #[default]
    Closed,
    Menu(Point),
    Editing(String),
}

#[derive(Clone, Debug)]
pub(crate) enum TitleAction {
    Menu(Point),
    Rename,
    Edit(String),
    Save,
    Dismiss,
}

impl TabTitle {
    pub(super) fn local(number: u64) -> Self {
        Self {
            automatic: Automatic::Local(number),
            custom: None,
            control: TitleControl::Closed,
        }
    }

    pub(super) fn named(name: &str, tabs: &[Tab]) -> Self {
        let used: HashSet<_> = tabs
            .iter()
            .filter_map(|tab| match &tab.title.automatic {
                Automatic::Named(name) => Some(name.as_str()),
                Automatic::Local(_) => None,
            })
            .collect();
        let mut automatic = name.to_owned();
        let mut number = 1;
        while used.contains(automatic.as_str()) {
            number += 1;
            automatic = format!("{name} ({number})");
        }
        Self {
            automatic: Automatic::Named(automatic),
            custom: None,
            control: TitleControl::Closed,
        }
    }

    pub(crate) fn label<'a>(&'a self, i18n: &I18n) -> Cow<'a, str> {
        self.custom
            .as_deref()
            .map_or_else(|| self.automatic(i18n), Cow::Borrowed)
    }

    pub(crate) fn automatic<'a>(&'a self, i18n: &I18n) -> Cow<'a, str> {
        match &self.automatic {
            Automatic::Named(name) => Cow::Borrowed(name),
            Automatic::Local(number) => {
                let mut args = FluentArgs::new();
                args.set("number", *number);
                i18n.format(Key::TerminalTab, &args).into()
            }
        }
    }
}

impl State {
    pub(crate) fn dismiss_title_edit(&mut self) {
        for tab in &mut self.tabs {
            tab.title.control = TitleControl::Closed;
            tab.pending_focus = None;
        }
    }

    pub(super) fn update_title(&mut self, request: u64, action: TitleAction) -> Task<Event> {
        let Some(index) = self.tabs.iter().position(|tab| tab.request == request) else {
            return Task::none();
        };
        let tab = &mut self.tabs[index];
        match action {
            TitleAction::Menu(point) => {
                self.dismiss_title_edit();
                self.tabs[index].title.control = TitleControl::Menu(point);
            }
            TitleAction::Rename if matches!(tab.title.control, TitleControl::Menu(_)) => {
                tab.title.control =
                    TitleControl::Editing(tab.title.custom.clone().unwrap_or_default());
                self.active = Some(request);
                return tab.focus_after_layout(FocusTarget::Rename);
            }
            TitleAction::Edit(text) if tab.editing_title() => {
                tab.title.control = TitleControl::Editing(text);
            }
            TitleAction::Save if tab.editing_title() => {
                if let TitleControl::Editing(text) = std::mem::take(&mut tab.title.control) {
                    tab.title.custom = (!text.trim().is_empty()).then_some(text);
                }
                return tab.focus_screen();
            }
            TitleAction::Dismiss => {
                let editing = tab.editing_title();
                tab.title.control = TitleControl::Closed;
                if editing {
                    return tab.focus_screen();
                }
            }
            _ => {}
        }
        Task::none()
    }
}
