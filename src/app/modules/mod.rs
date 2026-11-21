//! Compose enabled module runtimes and route their typed events.

use crate::modules::{docker::registry, kubernetes, terminal};

use super::{App, Message};
use crate::modules::ModuleId;
use iced::{Subscription, Task};
mod deployment;
pub(crate) mod external;
pub(crate) mod git;
pub(crate) mod pipelines;

#[derive(Default)]
pub(crate) struct State {
    pub git: crate::modules::git::State,
    pub pipelines: crate::modules::pipelines::State,
    pub terminal: terminal::State,
    pub kubernetes: kubernetes::State,
    pub docker: crate::modules::docker::State,
    pub deployment: crate::modules::deployment::State,
    pub external: external::State,
}

#[derive(Debug, Clone)]
pub(crate) enum Event {
    Toggle(ModuleId),
    Kubernetes(kubernetes::Event),
    SetEnabled(ModuleId, bool),
    Git(crate::modules::git::Event),
    Pipelines(crate::modules::pipelines::Event),
    Terminal(terminal::Event),
    Docker(crate::modules::docker::Event),
    Deployment(crate::modules::deployment::Event),
    External(external::Event),
}

impl App {
    pub(super) fn update_module(&mut self, event: Event) -> Task<Message> {
        match event {
            Event::SetEnabled(id, enabled) => self.set_module_enabled(id, enabled),
            Event::External(event) => self.update_external(event),
            Event::Git(event) if self.module_enabled(ModuleId::Git) => {
                self.update(git_event(event))
            }
            Event::Pipelines(event) if self.module_enabled(ModuleId::Pipelines) => {
                self.update(pipelines_event(event))
            }
            Event::Git(_) | Event::Pipelines(_) => Task::none(),
            Event::Toggle(id)
                if self.module_enabled(id) && self.workspace.directory().is_some() =>
            {
                match id {
                    ModuleId::Git | ModuleId::Pipelines => {
                        self.update(Message::TogglePanel(super::Panel::Module(id)))
                    }
                    ModuleId::Kubernetes => {
                        self.modules.kubernetes.open = !self.modules.kubernetes.open;
                        let task = if self.modules.kubernetes.open {
                            self.hide_other_bottom_panels(id);
                            self.update_module(Event::Kubernetes(kubernetes::Event::Open))
                        } else {
                            Task::none()
                        };
                        self.sync_panels();
                        task
                    }
                    ModuleId::Docker => {
                        self.modules.docker.open = !self.modules.docker.open;
                        let task = if self.modules.docker.open {
                            self.hide_other_bottom_panels(ModuleId::Docker);
                            self.update_module(Event::Docker(
                                crate::modules::docker::Event::Connections,
                            ))
                        } else {
                            Task::none()
                        };
                        self.sync_panels();
                        task
                    }
                    ModuleId::Deployment => {
                        self.update_deployment(crate::modules::deployment::Event::Toggle)
                    }
                    ModuleId::Terminal => {
                        if self.modules.terminal.open {
                            return self.update_terminal(terminal::Event::Hide);
                        }
                        if self.modules.terminal.tabs.is_empty() {
                            return self.update_terminal(terminal::Event::Start);
                        }
                        self.modules.terminal.open = true;
                        let save = self.select_terminal_panel();
                        self.sync_panels();
                        save
                    }
                }
            }
            Event::Terminal(event) if self.module_enabled(ModuleId::Terminal) => {
                self.update_terminal(event)
            }
            Event::Terminal(_) => Task::none(),
            Event::Kubernetes(event) if self.module_enabled(ModuleId::Kubernetes) => {
                use kubernetes::Event as K;
                match event {
                    K::Hide => {
                        self.modules.kubernetes.open = false;
                        self.sync_panels();
                        Task::none()
                    }
                    K::OpenRegistry(image) if self.module_enabled(ModuleId::Docker) => {
                        self.open_registry(registry::Event::Find(image))
                    }
                    K::PickRegistry(image)
                        if self.module_enabled(ModuleId::Docker)
                            && self.modules.kubernetes.choose() =>
                    {
                        self.open_registry(registry::Event::Pick(image))
                    }
                    K::Shell(request, name) if self.module_enabled(ModuleId::Terminal) => {
                        if let Some((title, args)) = self.modules.kubernetes.shell(request, &name) {
                            return self.update_terminal(terminal::Event::StartProgram {
                                program: terminal::Program::Kubernetes,
                                executable: crate::infrastructure::process::executable("kubectl"),
                                args,
                                title,
                            });
                        }
                        Task::none()
                    }
                    e => {
                        let old = self.modules.kubernetes.scope_id();
                        let task = self
                            .modules
                            .kubernetes
                            .update(e)
                            .map(|e| Message::Module(Event::Kubernetes(e)));
                        if old != self.modules.kubernetes.scope_id() {
                            self.modules.terminal.close_cluster_tabs(false);
                            self.sync_panels();
                        }
                        task
                    }
                }
            }
            Event::Kubernetes(_) => Task::none(),
            Event::Docker(event) if self.module_enabled(ModuleId::Docker) => {
                use crate::modules::docker::Event as Docker;
                let terminal = self.module_enabled(ModuleId::Terminal);
                match event {
                    Docker::Registry(registry::Event::Return | registry::Event::Chosen(_))
                        if self.modules.docker.registry.picking()
                            && self.modules.kubernetes.picking()
                            && self.module_enabled(ModuleId::Kubernetes) =>
                    {
                        self.modules.docker.registry.finish_pick();
                        self.hide_other_bottom_panels(ModuleId::Kubernetes);
                        self.modules.kubernetes.open = true;
                        self.sync_panels();
                        if let Docker::Registry(registry::Event::Chosen(image)) = event {
                            return self.update_module(Event::Kubernetes(
                                kubernetes::Event::Picked(image),
                            ));
                        }
                        self.modules.kubernetes.finish_pick();
                        Task::none()
                    }
                    Docker::Hide => {
                        self.modules.docker.open = false;
                        self.sync_panels();
                        Task::none()
                    }
                    Docker::Shell(id) if terminal => {
                        let Some((title, args)) = self.modules.docker.take_shell(id) else {
                            return Task::none();
                        };
                        self.update_terminal(terminal::Event::StartProgram {
                            program: terminal::Program::Docker,
                            executable: crate::infrastructure::process::executable("docker"),
                            args,
                            title,
                        })
                    }
                    _ => {
                        let Some(root) = self.workspace.directory() else {
                            return Task::none();
                        };
                        self.modules
                            .docker
                            .profiles
                            .clone_from(&self.workspace.preferences.registries);
                        let task = self
                            .modules
                            .docker
                            .update(event, root, terminal)
                            .map(docker_message);
                        if self.workspace.preferences.registries != self.modules.docker.profiles {
                            self.workspace
                                .preferences
                                .registries
                                .clone_from(&self.modules.docker.profiles);
                            return Task::batch([task, self.save()]);
                        }
                        task
                    }
                }
            }
            Event::Docker(_) => Task::none(),
            Event::Deployment(event) if self.module_enabled(ModuleId::Deployment) => {
                self.update_deployment(event)
            }
            Event::Deployment(_) => Task::none(),
            Event::Toggle(_) => Task::none(),
        }
    }

    fn open_registry(&mut self, event: registry::Event) -> Task<Message> {
        self.hide_other_bottom_panels(ModuleId::Docker);
        self.modules.docker.open = true;
        self.sync_panels();
        let browse = self.update_module(Event::Docker(crate::modules::docker::Event::Registry(
            event,
        )));
        if self.modules.docker.needs_connection() {
            let connect =
                self.update_module(Event::Docker(crate::modules::docker::Event::Connections));
            Task::batch([browse, connect])
        } else {
            browse
        }
    }
    pub(super) fn module_subscription(&self) -> Subscription<Message> {
        Subscription::batch([
            if self.module_enabled(ModuleId::Terminal) {
                self.modules.terminal.subscription().map(terminal_message)
            } else {
                Subscription::none()
            },
            if self.module_enabled(ModuleId::Deployment) {
                self.modules
                    .deployment
                    .subscription()
                    .map(deployment_message)
            } else {
                Subscription::none()
            },
            if self.module_open(ModuleId::Docker) {
                iced::time::every(self.modules.docker.refresh_interval())
                    .map(|_| docker_message(crate::modules::docker::Event::Tick))
            } else {
                Subscription::none()
            },
            self.external_subscription(),
        ])
    }

    fn update_terminal(&mut self, event: terminal::Event) -> Task<Message> {
        match event {
            terminal::Event::Focus(request) => {
                if self.modules.terminal.open && self.modules.terminal.active == Some(request) {
                    self.modules.git.branch_selection.selected.clear();
                    for preview in &mut self.tabs.files {
                        if let Some(document) = &preview.document {
                            preview
                                .selection
                                .update(crate::ui::code::selection::Action::Clear, document);
                        }
                    }
                }
                return Task::none();
            }
            terminal::Event::PasteRejected(request) => {
                if self.modules.terminal.open && self.modules.terminal.active == Some(request) {
                    self.notifications
                        .error(crate::i18n::Key::TerminalPasteTooLarge, None);
                }
                return Task::none();
            }
            _ => {}
        }
        let opens_panel = matches!(
            event,
            terminal::Event::Start | terminal::Event::StartProgram { .. }
        );
        let was_open = self.modules.terminal.open;
        let task = self
            .modules
            .terminal
            .update(
                event,
                self.workspace.directory().map(std::path::PathBuf::as_path),
            )
            .map(terminal_message);
        if opens_panel || was_open != self.modules.terminal.open {
            let save = if self.modules.terminal.open {
                self.select_terminal_panel()
            } else {
                Task::none()
            };
            self.sync_panels();
            return Task::batch([task, save]);
        }
        task
    }

    fn select_terminal_panel(&mut self) -> Task<Message> {
        let preferences = &self.workspace.preferences;
        let changed = preferences.show_git || preferences.show_pipelines;
        self.hide_other_bottom_panels(ModuleId::Terminal);
        self.modules.git.branch_attention = None;
        self.modules.pipelines.jobs.overview.popup = None;
        if changed { self.save() } else { Task::none() }
    }

    pub(crate) fn module_open(&self, id: ModuleId) -> bool {
        if !self.module_enabled(id) {
            return false;
        }
        match id {
            ModuleId::Git => self.workspace.preferences.show_git,
            ModuleId::Pipelines => self.workspace.preferences.show_pipelines,
            ModuleId::Kubernetes => self.modules.kubernetes.open,
            ModuleId::Docker => self.modules.docker.open,
            ModuleId::Terminal => self.modules.terminal.open,
            ModuleId::Deployment => self.modules.deployment.open,
        }
    }
}

pub(crate) fn docker_message(event: crate::modules::docker::Event) -> Message {
    Message::Module(Event::Docker(event))
}

fn terminal_message(event: terminal::Event) -> Message {
    Message::Module(Event::Terminal(event))
}

pub(crate) fn deployment_message(event: crate::modules::deployment::Event) -> Message {
    Message::Module(Event::Deployment(event))
}

#[cfg(test)]
mod tests;

pub(crate) fn git_message(event: crate::modules::git::Event) -> Message {
    Message::Module(Event::Git(event))
}
pub(crate) fn pipelines_message(event: crate::modules::pipelines::Event) -> Message {
    Message::Module(Event::Pipelines(event))
}
fn git_event(event: crate::modules::git::Event) -> Message {
    use crate::modules::git::Event as E;
    match event {
        E::Git(v) => Message::Git(v),
        E::BranchSelection(v) => Message::BranchSelection(v),
        E::Refresh => Message::Refresh,
        E::Hide => Message::TogglePanel(super::Panel::Module(ModuleId::Git)),
        E::ResizeGit(v) => Message::ResizeGit(v),
        E::Search(v) => Message::Search(v),
        E::AllRepositories => Message::AllRepositories,
        E::ToggleBranchFolder(v) => Message::ToggleBranchFolder(v),
        E::SelectBranch(v) => Message::SelectBranch(v),
        E::SelectCommit(v) => Message::SelectCommit(v),
        E::Explain(target, agent) => Message::Explain(super::explain::Event::Choose(target, agent)),
        E::CopyCommitId(v) => Message::CopyCommitId(v),
        E::HistoryScrolled { offset, remaining } => Message::HistoryScrolled { offset, remaining },
        E::LoadMoreHistory => Message::LoadMoreHistory,
        E::OpenCommitFile(v) => Message::OpenCommitFile(v),
        E::ToggleCommitFolder(v) => Message::ToggleCommitFolder(v),
        E::CloseCommitFiles => Message::CloseCommitFiles,
    }
}
fn pipelines_event(event: crate::modules::pipelines::Event) -> Message {
    use crate::modules::pipelines::Event as E;
    match event {
        E::RefreshPipelines => Message::RefreshPipelines,
        E::Jobs(v) => Message::Jobs(v),
        E::SignIn => Message::SignIn,
        E::Disconnect => Message::Auth(super::auth::Message::Disconnect),
        E::Hide => Message::TogglePanel(super::Panel::Module(ModuleId::Pipelines)),
        E::ChooseProvider(v) => Message::ChooseProvider(v),
        E::OpenUrl(v) => Message::OpenUrl(v),
        E::ExplainMenu(target, point) => {
            Message::Explain(super::explain::Event::Menu(target, point))
        }
    }
}
impl App {
    pub(crate) fn module_enabled(&self, id: ModuleId) -> bool {
        !self
            .workspace
            .preferences
            .disabled_modules
            .contains(id.key())
    }
    fn set_module_enabled(&mut self, id: ModuleId, enabled: bool) -> Task<Message> {
        if self.module_enabled(id) == enabled {
            return Task::none();
        }
        if enabled {
            self.workspace.preferences.disabled_modules.remove(id.key());
        } else {
            self.workspace
                .preferences
                .disabled_modules
                .insert(id.key().to_owned());
        }
        let mut tasks = Vec::new();
        if !enabled {
            match id {
                ModuleId::Kubernetes => {
                    self.modules.terminal.close_cluster_tabs(true);
                    self.modules.kubernetes.reset();
                    self.modules.docker.registry.finish_pick();
                }
                ModuleId::Docker => {
                    self.modules.docker.reset();
                    self.modules.kubernetes.finish_pick()
                }
                ModuleId::Terminal => self.modules.terminal.reset(),
                ModuleId::Deployment => {
                    self.modules.deployment.suspend();
                    for window in [
                        self.windows.deployment.take(),
                        self.windows.connections.take(),
                    ]
                    .into_iter()
                    .flatten()
                    {
                        tasks.push(iced::window::close(window));
                    }
                }
                ModuleId::Git => {
                    self.modules.git.git_actions.request += 1;
                    self.modules.git.git_actions.pending = false;
                    self.modules.git.git_actions.close_menu();
                    self.modules.git.git_actions.dialog = None;
                    self.modules.git.git_actions.history = Default::default();
                    self.modules.git.git_actions.bulk = Default::default();
                    self.modules.git.branch_selection = Default::default();
                    self.modules.git.branch = None;
                    self.modules.git.commit_request += 1;
                    self.close_commit_files();
                    self.repo_request += 1;
                    self.loading = false;
                    self.modules.git.history_loading = false;
                    self.changes.suspend();
                    self.project_changes.suspend();
                    self.repository_menu = None;
                    if self.file_panel == super::FilePanel::Changes {
                        self.file_panel = super::FilePanel::Project;
                    }
                    tasks.push(self.load_repository());
                }
                ModuleId::Pipelines => {
                    self.cancel_auth();
                    if !self.module_enabled(ModuleId::Git) {
                        self.repo_request += 1;
                        self.repository = None;
                        self.loading = false;
                    }
                    self.modules.pipelines.pipeline_request += 1;
                    self.modules.pipelines.pipeline_refreshing = false;
                    self.modules.pipelines.pipelines =
                        crate::modules::pipelines::PipelineState::Idle;
                    self.modules.pipelines.pipeline_reload_after_repository = false;
                    self.reset_jobs();
                }
            }
        } else {
            match id {
                ModuleId::Git => {
                    tasks.push(self.load_repository());
                    tasks.push(self.refresh_changes());
                }
                ModuleId::Pipelines => {
                    if self.has_remote() {
                        tasks.push(self.load_pipelines());
                    } else {
                        tasks.push(self.load_repository());
                    }
                }
                _ => {}
            }
        }
        self.sync_panels();
        tasks.push(self.save());
        Task::batch(tasks)
    }
}

#[cfg(test)]
pub(crate) fn host_message(message: Message) -> Message {
    match message {
        Message::Module(Event::Git(event)) => git_event(event),
        Message::Module(Event::Pipelines(event)) => pipelines_event(event),
        message => message,
    }
}
