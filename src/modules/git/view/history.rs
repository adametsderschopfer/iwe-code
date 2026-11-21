use super::{Event as Message, View};
use crate::{
    domain::repository::{ReferenceKind, project::BranchKey},
    i18n::Key,
    modules::git::actions::{
        error_key,
        history::{Action, Dialog, Event, Input, Output, Reset, Target},
    },
    ui::{context_menu, ellipsis, icons::Icon, modal, theme},
};
use iced::{
    Element, Length, Size,
    widget::{
        Column, button, column, container, pick_list, row, scrollable, text, text_editor,
        text_input, toggler,
    },
};

pub(super) fn event(value: Event) -> Message {
    Message::Git(crate::modules::git::actions::Message::History(Box::new(
        value,
    )))
}

fn close() -> Message {
    Message::Git(crate::modules::git::actions::Message::Close)
}

fn key(action: Action) -> Key {
    match action {
        Action::Tip => Key::GitHistoryTip,
        Action::Merge => Key::GitHistoryMerge,
        Action::Rebase => Key::GitHistoryRebase,
        Action::SwitchRebase => Key::GitHistorySwitchRebase,
        Action::PullMerge => Key::GitHistoryPullMerge,
        Action::PullRebase => Key::GitHistoryPullRebase,
        Action::PushTag => Key::GitHistoryPushTag,
        Action::DeleteTag => Key::GitHistoryDeleteTag,
        Action::Copy => Key::GitHistoryCopy,
        Action::Parents => Key::GitHistoryParents,
        Action::Children => Key::GitHistoryChildren,
        Action::Snapshot => Key::GitHistorySnapshot,
        Action::Browser => Key::GitHistoryBrowser,
        Action::Create => Key::GitNewBranch,
        Action::Tag => Key::GitHistoryTag,
        Action::Patch => Key::GitHistoryPatch,
        Action::Detach => Key::GitHistoryDetach,
        Action::Pick => Key::GitHistoryPick,
        Action::Revert => Key::GitHistoryRevert,
        Action::Reset => Key::GitHistoryReset,
        Action::Undo => Key::GitHistoryUndo,
        Action::Message => Key::GitHistoryMessage,
        Action::Continue => Key::GitHistoryContinue,
        Action::Abort => Key::GitHistoryAbort,
    }
}

fn menu_item<'a>(app: View<'a>, action: Action, message: Message) -> Element<'a, Message> {
    context_menu::action(key(action), app.i18n.text(key(action)))
        .on_press(message)
        .into()
}

pub(super) fn branch_items<'a>(
    app: View<'a>,
    branch: &crate::domain::repository::Branch,
    target: &BranchKey,
) -> Vec<Element<'a, Message>> {
    let mut actions = vec![Action::Tip];
    let context = app.state.git_actions.history.menu_context.as_ref();
    if context.is_some_and(|context| {
        context
            .unfinished
            .as_ref()
            .is_some_and(|operation| !operation.is_empty())
    }) {
        actions.extend([Action::Continue, Action::Abort]);
    } else if context.is_some_and(|context| context.unfinished.is_none()) {
        if !branch.current {
            actions.extend([Action::Merge, Action::Rebase]);
            match branch.kind {
                ReferenceKind::Local => actions.push(Action::SwitchRebase),
                ReferenceKind::Remote => actions.extend([Action::PullMerge, Action::PullRebase]),
                ReferenceKind::Tag => {}
            }
        }
        if branch.kind == ReferenceKind::Tag {
            actions.extend([Action::PushTag, Action::DeleteTag]);
        }
    }
    actions
        .into_iter()
        .map(|action| {
            menu_item(
                app,
                action,
                event(Event::Choose(Target::Branch(target.clone()), action)),
            )
        })
        .collect()
}

pub(super) fn menu<'a>(app: View<'a>, base: Element<'a, Message>) -> Element<'a, Message> {
    if app.state.git_actions.history.commit_menu_loading {
        return base;
    }
    let Some((target, point)) = &app.state.git_actions.history.menu else {
        return base;
    };
    let Some(repo) = app
        .repository
        .as_ref()
        .and_then(|repos| repos.get(&target.root))
    else {
        return base;
    };
    let Some(commit) = app
        .repository
        .as_ref()
        .and_then(|repos| repos.commit(target))
        .or_else(|| {
            app.state
                .git_actions
                .history
                .revealed
                .as_ref()
                .filter(|entry| entry.key() == *target)
                .map(|entry| &entry.commit)
        })
    else {
        return base;
    };
    context_menu::overlay(
        base,
        *point,
        Size::new(365. + context_menu::ICON_COLUMN_WIDTH, 620.),
        move || {
            let mut navigation = vec![Action::Children, Action::Snapshot];
            if !commit.parents.is_empty() {
                navigation.insert(0, Action::Parents);
            }
            if repo
                .remote
                .as_ref()
                .is_some_and(|remote| remote.provider.is_some())
            {
                navigation.push(Action::Browser);
            }
            let context = app.state.git_actions.history.menu_context.as_ref();
            let mut transfer = vec![Action::Copy];
            let mut references = Vec::new();
            let mut history = Vec::new();
            if let Some(context) = context {
                match context.unfinished.as_deref() {
                    Some(operation) if !operation.is_empty() => {
                        history.extend([Action::Continue, Action::Abort]);
                    }
                    Some(_) => {}
                    None => {
                        references.extend([Action::Create, Action::Tag]);
                        history.extend([
                            Action::Detach,
                            Action::Pick,
                            Action::Revert,
                            Action::Rebase,
                            Action::Reset,
                        ]);
                        if commit.parents.len() <= 1 {
                            transfer.push(Action::Patch);
                        }
                        if context.unpublished {
                            history.extend([Action::Undo, Action::Message]);
                        }
                    }
                }
            }
            let mut content = Column::new();
            for (index, group) in [navigation, transfer, references, history]
                .into_iter()
                .enumerate()
            {
                if group.is_empty() {
                    continue;
                }
                if index > 0 {
                    content = content.push(context_menu::separator());
                }
                for action in group {
                    content = content.push(
                        container(menu_item(
                            app,
                            action,
                            event(Event::Choose(Target::Commit(target.clone()), action)),
                        ))
                        .id(format!("git-history-action-{action:?}")),
                    );
                }
            }
            let mut branches = repo
                .branches
                .iter()
                .filter(|branch| {
                    context.is_some_and(|context| context.refs.contains(&branch.reference))
                })
                .peekable();
            if branches.peek().is_some() {
                content = content.push(context_menu::separator());
            }
            for branch in branches {
                let key = BranchKey {
                    root: target.root.clone(),
                    reference: branch.reference.clone(),
                };
                content = content.push(
                    context_menu::row(context_menu::icon_label(
                        Some(if branch.kind == ReferenceKind::Tag {
                            Icon::Tag
                        } else {
                            Icon::Branch
                        }),
                        branch.name.clone(),
                    ))
                    .on_press(Message::Git(
                        crate::modules::git::actions::Message::Menu(key, *point),
                    )),
                );
            }
            content = content
                .push(context_menu::separator())
                .push(crate::ui::explain::submenu(
                    app.i18n,
                    crate::domain::explain::Target::Commit(target.clone()),
                    Message::Explain,
                ));
            scrollable(content)
                .height(Length::Shrink)
                .style(theme::scroll)
                .into()
        },
        close(),
    )
}

fn update(input: &Input, change: impl FnOnce(&mut Input)) -> Message {
    let mut input = input.clone();
    change(&mut input);
    event(Event::Input(input))
}

fn reset_key(reset: Reset) -> Key {
    match reset {
        Reset::Soft => Key::GitHistorySoft,
        Reset::Mixed => Key::GitHistoryMixed,
        Reset::Hard => Key::GitHistoryHard,
        Reset::Keep => Key::GitHistoryKeep,
    }
}

fn form<'a>(app: View<'a>, dialog: &'a Dialog) -> (Column<'a, Message>, bool) {
    let action = dialog.action;
    let input = &dialog.input;
    let mut fields = Column::new().spacing(10);
    let mut enabled = true;
    if let Some(description) = match action {
        Action::Detach => Some(Key::GitHistoryDetached),
        Action::Rebase | Action::SwitchRebase | Action::PullRebase | Action::Message => {
            Some(Key::GitHistoryRewrite)
        }
        Action::Reset => Some(Key::GitHistoryResetHint),
        _ => None,
    } {
        fields = fields.push(text(app.i18n.text(description)).size(14));
    }
    if action == Action::Reset {
        fields = fields.push(Column::with_children(
            [Reset::Soft, Reset::Mixed, Reset::Hard, Reset::Keep]
                .into_iter()
                .map(|mode| {
                    button(text(app.i18n.text(reset_key(mode))).size(14))
                        .width(Length::Fill)
                        .padding(8)
                        .style(theme::action(input.reset == mode))
                        .on_press(update(input, |input| {
                            input.reset = mode;
                            input.name.clear();
                        }))
                        .into()
                }),
        ));
        if input.reset == Reset::Hard {
            fields = fields.push(text(app.i18n.text(Key::GitHistoryTypeBranch)).size(14));
        }
    }
    if let Some(label) = match action {
        Action::Tag => Some(Key::GitHistoryTagName),
        Action::Create => Some(Key::GitBranchName),
        Action::Reset if input.reset == Reset::Hard => Some(Key::GitBranchName),
        _ => None,
    } {
        let label = app.i18n.text(label);
        fields = fields.push(text(label.clone()).size(13)).push(
            text_input(&label, &input.name)
                .on_input(move |name| update(input, |input| input.name = name))
                .padding(10)
                .style(theme::input),
        );
        enabled &= !input.name.trim().is_empty();
        if action == Action::Reset {
            enabled &=
                dialog.context.branch.strip_prefix("refs/heads/") == Some(input.name.as_str());
        }
    }
    if action == Action::Create {
        fields = fields.push(
            toggler(input.checkout)
                .label(app.i18n.text(Key::GitSwitchAfterCreate))
                .style(theme::toggle)
                .on_toggle(move |checkout| update(input, |input| input.checkout = checkout)),
        );
    }
    if matches!(action, Action::Tag | Action::Message) {
        let label = app.i18n.text(if action == Action::Tag {
            Key::GitHistoryAnnotation
        } else {
            Key::GitHistoryCommitMessage
        });
        fields = fields.push(text(label).size(13)).push(
            text_editor(&dialog.editor)
                .on_action(|action| event(Event::Edit(action)))
                .padding(10)
                .height(100.)
                .size(14),
        );
        enabled &= action == Action::Tag || !input.message.trim().is_empty();
    }
    if action == Action::PushTag {
        let remotes = app
            .repository
            .as_ref()
            .and_then(|repo| repo.get(dialog.target.root()))
            .map(|repo| repo.remotes.clone())
            .unwrap_or_default();
        fields = fields
            .push(text(app.i18n.text(Key::GitRemote)).size(13))
            .push(
                pick_list(remotes, Some(input.name.clone()), move |name| {
                    update(input, |input| input.name = name)
                })
                .padding(10)
                .style(theme::select)
                .menu_style(theme::select_menu)
                .width(Length::Fill),
            );
        enabled &= !input.name.is_empty();
    }
    if action == Action::Patch {
        fields = fields.push(
            button(text(app.i18n.text(Key::GitHistoryDestination)))
                .on_press(event(Event::Browse))
                .style(theme::secondary),
        );
        if let Some(path) = &input.path {
            fields = fields.push(text(path.display().to_string()).size(13));
        }
        enabled &= input.path.is_some();
    }
    if matches!(action, Action::Pick | Action::Revert) && dialog.context.parents.len() > 1 {
        fields = fields.push(text(app.i18n.text(Key::GitHistoryMainParent)).size(14));
        for (index, id) in dialog.context.parents.iter().enumerate() {
            let parent = index + 1;
            fields = fields.push(
                button(text(id).size(13))
                    .style(theme::action(input.parent == Some(parent)))
                    .on_press(update(input, |input| input.parent = Some(parent))),
            );
        }
        enabled &= input.parent.is_some();
    }
    (fields, enabled)
}

fn results<'a>(app: View<'a>, output: &'a Output) -> Column<'a, Message> {
    let mut body = Column::new().spacing(8);
    let result = |label, message| {
        context_menu::row(ellipsis::text(label, context_menu::TEXT_SIZE)).on_press(message)
    };
    match output {
        Output::Navigate(ids) => {
            for id in ids {
                body = body.push(result(id.clone(), event(Event::Navigate(id.clone()))));
            }
            if ids.is_empty() {
                body = body.push(text(app.i18n.text(Key::GitHistoryEmpty)));
            }
        }
        Output::Snapshot(paths) => {
            for path in paths {
                body = body.push(result(path.clone(), event(Event::ReadFile(path.clone()))));
            }
            if paths.is_empty() {
                body = body.push(text(app.i18n.text(Key::GitHistoryEmpty)));
            }
        }
        _ => {}
    }
    body
}

pub(super) fn overlay<'a>(app: View<'a>, base: Element<'a, Message>) -> Element<'a, Message> {
    let Some(dialog) = &app.state.git_actions.history.dialog else {
        return base;
    };
    let pending = app.state.git_actions.pending;
    let mut body = column![
        text(app.i18n.text(key(dialog.action)))
            .size(22)
            .font(theme::semibold()),
        text(
            dialog
                .target
                .root()
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
        )
        .size(13),
        text(
            dialog
                .target
                .revision()
                .strip_prefix("refs/heads/")
                .or_else(|| dialog.target.revision().strip_prefix("refs/remotes/"))
                .or_else(|| dialog.target.revision().strip_prefix("refs/tags/"))
                .unwrap_or(dialog.target.revision())
        )
        .size(13),
    ]
    .spacing(12);
    if !dialog.context.branch.is_empty() {
        body = body.push(text(dialog.context.branch.trim_start_matches("refs/heads/")).size(13));
    }
    let mut footer = row![
        button(text(app.i18n.text(Key::Cancel)))
            .style(theme::secondary)
            .on_press_maybe((!pending).then(close))
    ]
    .spacing(8);
    if let Some(output) = &dialog.output {
        body = body
            .push(container(scrollable(results(app, output)).style(theme::scroll)).max_height(420));
    } else if dialog.context.unfinished.is_some()
        && !dialog.action.reads()
        && !matches!(dialog.action, Action::Abort | Action::Continue)
    {
        body = body.push(text(app.i18n.text(Key::GitHistoryUnfinished)));
        if dialog
            .context
            .unfinished
            .as_ref()
            .is_some_and(|operation| !operation.is_empty())
        {
            for action in [Action::Continue, Action::Abort] {
                footer = footer.push(
                    button(text(app.i18n.text(key(action))))
                        .style(theme::secondary)
                        .on_press_maybe(
                            (!pending)
                                .then_some(event(Event::Choose(dialog.target.clone(), action))),
                        ),
                );
            }
        }
    } else if !dialog.action.reads() {
        let (fields, enabled) = form(app, dialog);
        body = body.push(fields);
        footer = footer.push(
            button(text(app.i18n.text(key(dialog.action))))
                .style(theme::primary)
                .on_press_maybe((enabled && !pending).then_some(event(Event::Submit))),
        );
    }
    if pending {
        body = body.push(text(app.i18n.text(Key::GitWorking)).size(13));
    }
    if let Some(error) = dialog.error {
        body = body.push(
            text(app.i18n.text(error_key(error)))
                .size(13)
                .style(theme::RED.text()),
        );
    }
    modal::overlay(
        base,
        body.push(footer).into(),
        if dialog.output.is_some() { 800. } else { 560. },
        close(),
    )
}
