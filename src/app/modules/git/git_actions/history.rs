use super::{App, AppMessage, Message, error_key};
use crate::{
    domain::{
        git::Error,
        repository::{Provider, project::CommitKey},
    },
    i18n::Key,
    modules::git::{
        actions::history::{Action, Dialog, Event, Input, Output, Target},
        transport::history as git,
    },
};
use iced::Task;

fn task(event: Event) -> AppMessage {
    AppMessage::Git(Message::History(Box::new(event)))
}

impl App {
    pub(super) fn inspect_git_menu(&mut self, target: Target) -> Task<AppMessage> {
        let actions = &mut self.modules.git.git_actions;
        actions.history.menu_context = None;
        actions.request += 1;
        let request = actions.request;
        Task::perform(async move { git::inspect(&target).await }, move |result| {
            task(Event::MenuInspected(request, result))
        })
    }

    pub(super) fn update_git_history(&mut self, event: Event) -> Task<AppMessage> {
        if matches!(event, Event::Menu(..) | Event::Choose(..))
            && (self.space_draft.is_some()
                || self.project_add.is_some()
                || self.auth_dialog.is_some())
        {
            return Task::none();
        }
        let actions = &mut self.modules.git.git_actions;
        match event {
            Event::MenuInspected(request, result) if request == actions.request => {
                if actions.menu.is_some() || actions.history.menu.is_some() {
                    actions.history.commit_menu_loading = false;
                    actions.history.menu_context = result.ok();
                }
            }
            Event::Inspected(request, target, action, result)
                if request == actions.request && actions.pending =>
            {
                actions.pending = false;
                match result {
                    Ok(context) => {
                        let message = if action == Action::Message {
                            context.message.clone().unwrap_or_default()
                        } else {
                            String::new()
                        };
                        let editor = iced::widget::text_editor::Content::with_text(&message);
                        let input = Input {
                            message,
                            checkout: action == Action::Create,
                            name: if action == Action::PushTag {
                                self.repository
                                    .as_ref()
                                    .and_then(|repos| repos.get(target.root()))
                                    .and_then(|repo| {
                                        repo.remotes
                                            .iter()
                                            .find(|remote| *remote == "origin")
                                            .or(repo.remotes.first())
                                    })
                                    .cloned()
                                    .unwrap_or_default()
                            } else {
                                String::new()
                            },
                            ..Default::default()
                        };
                        actions.history.dialog = Some(Dialog {
                            target,
                            action,
                            context,
                            input,
                            editor,
                            output: None,
                            error: None,
                        });
                        if action.reads() {
                            return self.update_git_history(Event::Submit);
                        }
                    }
                    Err(error) => self.notifications.error(error_key(error), None),
                }
            }
            Event::Finished(request, result, context)
                if request == actions.request && actions.pending =>
            {
                actions.pending = false;
                let Some(dialog) = &mut actions.history.dialog else {
                    return Task::none();
                };
                match result {
                    Ok(Output::Done) => {
                        actions.history.dialog = None;
                        self.notifications.success(Key::GitHistoryDone, None);
                        return self.load_directory();
                    }
                    Ok(Output::Navigate(ids)) if ids.len() == 1 => {
                        return self.update_git_history(Event::Navigate(ids[0].clone()));
                    }
                    Ok(output) => dialog.output = Some(output),
                    Err(error) => {
                        if error == Error::Stale {
                            actions.history.dialog = None;
                            self.notifications.error(error_key(error), None);
                        } else if let Some(context) =
                            context.filter(|context| context.unfinished.is_some())
                        {
                            dialog.context = context;
                            dialog.error = Some(Error::Unfinished);
                        } else {
                            dialog.error = Some(error);
                        }
                        return Task::batch([self.load_repository(), self.refresh_changes()]);
                    }
                }
            }
            Event::CommitLoaded(request, key, result)
                if request == actions.request && actions.pending =>
            {
                actions.pending = false;
                match result {
                    Ok(commit) => {
                        actions.history.revealed =
                            Some(crate::domain::repository::project::ScopedCommit {
                                root: key.root.clone(),
                                commit: commit.clone(),
                            });
                        return Task::batch([
                            self.reset_history_scroll(),
                            self.select_resolved_commit(key, commit),
                        ]);
                    }
                    Err(error) => self.notifications.error(error_key(error), None),
                }
            }
            _ if actions.pending => {}
            Event::Menu(key, point) if !actions.has_dialog() => {
                if self
                    .repository
                    .as_ref()
                    .and_then(|repo| repo.commit(&key))
                    .is_none()
                    && !actions
                        .history
                        .revealed
                        .as_ref()
                        .is_some_and(|entry| entry.key() == key)
                {
                    return Task::none();
                }
                self.project_menu = None;
                self.space_menu = None;
                self.modules.git.branch_selection.selected.clear();
                actions.menu = None;
                actions.bulk.menu = None;
                actions.history.menu = Some((key.clone(), point));
                actions.history.commit_menu_loading = true;
                return self.inspect_git_menu(Target::Commit(key));
            }
            Event::Choose(target, action)
                if !actions.has_dialog() || matches!(action, Action::Continue | Action::Abort) =>
            {
                if matches!(action, Action::Continue | Action::Abort) {
                    actions.history.dialog = None;
                }
                let Some(repo) = self
                    .repository
                    .as_ref()
                    .and_then(|repos| repos.get(target.root()))
                else {
                    return Task::none();
                };
                let context = match &target {
                    Target::Commit(key)
                        if actions
                            .history
                            .menu
                            .as_ref()
                            .is_some_and(|(active, _)| active == key) =>
                    {
                        actions.history.menu_context.clone()
                    }
                    Target::Branch(key)
                        if actions
                            .menu
                            .as_ref()
                            .is_some_and(|(active, _, _)| active == key) =>
                    {
                        actions.history.menu_context.clone()
                    }
                    _ => None,
                };
                actions.history.clear_menu();
                actions.menu = None;
                match action {
                    Action::Copy => return iced::clipboard::write(target.revision().to_owned()),
                    Action::Browser => {
                        let Some(remote) = &repo.remote else {
                            return Task::none();
                        };
                        let segment = match remote.provider {
                            Some(Provider::GitHub) => "commit",
                            Some(Provider::GitLab) => "-/commit",
                            None => return Task::none(),
                        };
                        let url = format!(
                            "https://{}/{}/{segment}/{}",
                            remote.host,
                            remote.project,
                            target.revision()
                        );
                        return Task::perform(
                            async move {
                                tokio::task::spawn_blocking(move || open::that(url))
                                    .await
                                    .ok()
                                    .is_some_and(|result| result.is_ok())
                            },
                            |ok| AppMessage::Git(Message::HistoryBrowserOpened(ok)),
                        );
                    }
                    _ => {}
                }
                actions.pending = true;
                actions.request += 1;
                let request = actions.request;
                return Task::perform(
                    async move {
                        let result = match context {
                            Some(context) => Ok(context),
                            None => git::inspect(&target).await,
                        };
                        (target, result)
                    },
                    move |(target, result)| task(Event::Inspected(request, target, action, result)),
                );
            }
            Event::Input(input) => {
                if let Some(dialog) = &mut actions.history.dialog {
                    dialog.input = input;
                    dialog.error = None;
                }
            }
            Event::Edit(action) => {
                if let Some(dialog) = &mut actions.history.dialog {
                    dialog.editor.perform(action);
                    dialog.input.message = dialog.editor.text();
                    dialog.error = None;
                }
            }
            Event::Browse => {
                let Some(dialog) = &actions.history.dialog else {
                    return Task::none();
                };
                actions.request += 1;
                let request = actions.request;
                let picker = rfd::AsyncFileDialog::new()
                    .set_directory(dialog.target.root())
                    .set_file_name("commit.patch");
                return Task::perform(
                    async move { picker.save_file().await.map(|file| file.path().to_owned()) },
                    move |path| task(Event::Path(request, path)),
                );
            }
            Event::Path(request, path) if request == actions.request => {
                if let (Some(dialog), Some(path)) = (&mut actions.history.dialog, path) {
                    dialog.input.path = Some(path);
                }
            }
            Event::Submit => {
                let Some(dialog) = actions.history.dialog.clone() else {
                    return Task::none();
                };
                return self.execute_git_history(dialog);
            }
            Event::Navigate(id) => {
                let Some(dialog) = actions.history.dialog.take() else {
                    return Task::none();
                };
                let key = CommitKey {
                    root: dialog.target.root().clone(),
                    id,
                };
                if self
                    .repository
                    .as_ref()
                    .and_then(|repo| repo.commit(&key))
                    .is_some()
                {
                    self.modules.git.query.clear();
                    self.modules.git.git_actions.history.revealed = None;
                    let position = self
                        .repository
                        .as_ref()
                        .and_then(|repo| repo.commits.iter().position(|entry| entry.key() == key))
                        .unwrap_or_default();
                    let top = position.saturating_sub(2) as f32 * 30.;
                    self.modules.git.history_scroll_y = top;
                    let scroll = iced::widget::operation::scroll_to(
                        "commit-history",
                        iced::widget::operation::AbsoluteOffset { x: 0., y: top },
                    );
                    return Task::batch([scroll, self.select_commit(key)]);
                }
                self.modules.git.query.clear();
                actions.pending = true;
                actions.request += 1;
                let request = actions.request;
                return Task::perform(
                    git::load_commit(key.root.clone(), key.id.clone()),
                    move |result| task(Event::CommitLoaded(request, key.clone(), result)),
                );
            }
            Event::ReadFile(path) => {
                let Some(dialog) = &actions.history.dialog else {
                    return Task::none();
                };
                if !matches!(&dialog.output, Some(Output::Snapshot(paths)) if paths.contains(&path))
                {
                    return Task::none();
                }
                let id = dialog.context.id.clone();
                let root = dialog.target.root().clone();
                actions.history.dialog = None;
                return self.open_git_revision(root, path, id);
            }
            _ => {}
        }
        Task::none()
    }

    fn execute_git_history(&mut self, dialog: Dialog) -> Task<AppMessage> {
        let actions = &mut self.modules.git.git_actions;
        actions.pending = true;
        actions.request += 1;
        let request = actions.request;
        Task::perform(
            async move {
                let result = git::execute(
                    &dialog.target,
                    dialog.action,
                    &dialog.context,
                    &dialog.input,
                )
                .await;
                let context = if result.is_err() && !dialog.action.reads() {
                    git::inspect(&dialog.target).await.ok()
                } else {
                    None
                };
                (result, context)
            },
            move |(result, context)| task(Event::Finished(request, result, context)),
        )
    }
}
