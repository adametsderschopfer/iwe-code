//! Project localized workspace entries into the native application menu.

use super::{App, Message, Navigation, SettingsTab, spaces, windows};
use crate::{
    i18n::Key,
    infrastructure::native_menu::{self, Action, Model},
};
use iced::{
    Subscription, Task,
    futures::{Stream, StreamExt, channel::mpsc, stream},
    window,
};

#[derive(Default)]
pub(super) struct State {
    status: Status,
    model: Option<Model>,
}

#[derive(Default, PartialEq, Eq)]
enum Status {
    #[default]
    WaitingForWindow,
    Listening,
    Ready,
    Unavailable,
}

#[derive(Debug, Clone)]
pub(crate) enum Event {
    Ready,
    Selected(Action),
    Unavailable,
}

fn events() -> impl Stream<Item = Message> {
    let (sender, actions) = mpsc::unbounded();
    native_menu::listen(move |action| {
        let _ = sender.unbounded_send(action);
    });
    stream::once(async { Message::NativeMenu(Event::Ready) })
        .chain(actions.map(|action| Message::NativeMenu(Event::Selected(action))))
}

impl App {
    pub(super) fn start_native_menu(&mut self) {
        if self.native_menu.status == Status::WaitingForWindow {
            self.native_menu.status = Status::Listening;
        }
    }

    pub(super) fn native_menu_subscription(&self) -> Subscription<Message> {
        if matches!(self.native_menu.status, Status::Listening | Status::Ready) {
            Subscription::run(events)
        } else {
            Subscription::none()
        }
    }

    pub(super) fn sync_native_menu(&mut self) -> Task<Message> {
        let Some(id) = self
            .window_id
            .filter(|_| self.native_menu.status == Status::Ready)
        else {
            return Task::none();
        };
        let model = self.menu_model();
        if self.native_menu.model.as_ref() == Some(&model) {
            return Task::none();
        }
        self.native_menu.model = Some(model.clone());
        window::run(id, move |_| native_menu::sync(&model)).then(|result| match result {
            Ok(()) => Task::none(),
            Err(()) => Task::done(Message::NativeMenu(Event::Unavailable)),
        })
    }

    fn menu_model(&self) -> Model {
        Model {
            application: self.i18n.text(Key::AppTitle),
            settings: self.i18n.text(Key::Settings),
            about: self.i18n.text(Key::About),
            deployment: self.i18n.text(Key::Deployment),
            deployment_enabled: self.module_enabled(crate::modules::ModuleId::Deployment),
            connections: self.i18n.text(Key::DeploymentConnections),
            profiles: self.i18n.text(Key::DeploymentProfiles),
            spaces_title: self.i18n.text(Key::Spaces),
            projects_title: self.i18n.text(Key::Projects),
            new_space: self.i18n.text(Key::NewSpace),
            no_projects: self.i18n.text(Key::NoProjects),
            hide: self.i18n.text(Key::MenuHide),
            hide_others: self.i18n.text(Key::MenuHideOthers),
            show_all: self.i18n.text(Key::MenuShowAll),
            services: self.i18n.text(Key::MenuServices),
            quit: self.i18n.text(Key::MenuQuit),
            recording_shortcut: self.shortcuts.recording.is_some(),
            enabled: self.menu_actions_enabled(),
            spaces: self
                .workspace
                .spaces
                .iter()
                .map(|space| {
                    (
                        space.name.clone(),
                        space
                            .projects
                            .iter()
                            .map(|project| (project.name.clone(), project.directory.clone()))
                            .collect(),
                    )
                })
                .collect(),
        }
    }

    pub(super) fn update_native_menu(&mut self, event: Event) -> Task<Message> {
        match event {
            Event::Ready => {
                self.native_menu.status = Status::Ready;
                Task::none()
            }
            Event::Selected(action) => self.select_native_menu_action(action),
            Event::Unavailable => {
                self.native_menu.status = Status::Unavailable;
                self.notifications.error(Key::MenuUnavailable, None);
                Task::none()
            }
        }
    }

    fn select_native_menu_action(&mut self, action: Action) -> Task<Message> {
        match action {
            Action::Settings => self.update(Message::Navigate(Navigation::Settings)),
            Action::DeploymentConnections | Action::DeploymentProfiles
                if self.menu_actions_enabled()
                    && self.module_enabled(crate::modules::ModuleId::Deployment) =>
            {
                let tab = match action {
                    Action::DeploymentConnections => SettingsTab::Connections,
                    _ => SettingsTab::Deployment,
                };
                self.update(Message::SelectSettingsTab(tab))
            }
            Action::About if !self.modules.deployment.has_dialog() => {
                self.shortcuts = super::shortcuts::State::default();
                self.settings_tab = SettingsTab::About;
                self.update(Message::Navigate(Navigation::Settings))
            }
            Action::Quit => self.window_id.map_or_else(Task::none, |id| {
                self.update_window(windows::Event::Close(id))
            }),
            Action::NewSpace | Action::Project(..) if self.menu_actions_enabled() => {
                let focus = self.window_id.map_or_else(Task::none, |id| {
                    window::minimize::<Message>(id, false).chain(window::gain_focus(id))
                });
                let task = match action {
                    Action::NewSpace => self.update(Message::Space(spaces::Message::New)),
                    Action::Project(space, path) => {
                        let project = self.workspace.spaces.get(space).and_then(|space| {
                            space
                                .projects
                                .iter()
                                .position(|project| project.directory == path)
                        });
                        project.map_or_else(Task::none, |project| {
                            self.update(Message::SelectProject(space, project))
                        })
                    }
                    _ => Task::none(),
                };
                Task::batch([focus, task])
            }
            _ => Task::none(),
        }
    }
}

#[cfg(test)]
mod tests;
