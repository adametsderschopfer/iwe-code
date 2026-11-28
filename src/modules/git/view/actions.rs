use super::{Event as Message, View};
use crate::modules::git::actions::{self as git_actions, Action, Message as GitMessage};
use crate::ui::modal;
use crate::ui::theme::iced;
use crate::ui::tooltip;
use crate::{
    i18n::{FluentArgs, Key},
    ui::{context_menu, ellipsis, theme},
};
use iced::{
    Element, Length, Size,
    widget::{column, container, text},
};

pub(super) fn label(app: View<'_>, action: Action, branch: &str) -> String {
    if action == Action::Create {
        let mut args = FluentArgs::new();
        args.set("branch", branch);
        return app.i18n.format(Key::GitNewBranchFrom, &args);
    }
    app.i18n.text(action_key(action))
}

pub(crate) fn overlay<'a>(app: View<'a>, base: Element<'a, Message>) -> Element<'a, Message> {
    let base = super::history::menu(app, base);
    let base = if let Some((key, branch, point)) = &app.state.git_actions.menu {
        context_menu::overlay(
            base,
            *point,
            Size::new(300. + context_menu::ICON_COLUMN_WIDTH, 350.),
            move || {
                let item = |action| {
                    let enabled = git_actions::allowed(action, branch)
                        && !app.state.git_actions.pending
                        && app
                            .state
                            .git_actions
                            .history
                            .menu_context
                            .as_ref()
                            .is_some_and(|context| context.unfinished.is_none());
                    container(
                        context_menu::action(action_key(action), label(app, action, &branch.name))
                            .on_press_maybe(
                                enabled
                                    .then(|| Message::Git(GitMessage::Choose(action, key.clone()))),
                            ),
                    )
                    .id(format!("git-action-{action:?}"))
                };
                let mut content = column![
                    item(Action::Checkout),
                    item(Action::Create),
                    item(Action::Update),
                    item(Action::Push),
                    item(Action::Rename),
                    context_menu::separator(),
                ];
                for action in super::history::branch_items(app, branch, key) {
                    content = content.push(action);
                }
                iced::widget::scrollable(
                    content
                        .push(context_menu::separator())
                        .push(crate::ui::explain::submenu(
                            app.i18n,
                            crate::domain::explain::Target::Branch(key.clone()),
                            Message::Explain,
                        ))
                        .push(context_menu::separator())
                        .push(item(Action::Delete)),
                )
                .height(Length::Shrink)
                .style(theme::scroll)
                .into()
            },
            Message::Git(GitMessage::Close),
        )
    } else {
        base
    };
    let base = super::history::overlay(app, super::bulk::overlay(app, base));
    let Some(dialog) = &app.state.git_actions.dialog else {
        return base;
    };
    let mut content = modal::content(app.i18n.text(action_key(dialog.action))).push(tooltip::view(
        ellipsis::text(&dialog.branch.name, 14.),
        text(&dialog.branch.name).size(12),
        tooltip::Position::Bottom,
    ));
    if app
        .repository
        .as_ref()
        .is_some_and(|repos| repos.repositories.len() > 1)
    {
        content = content.push(
            text(
                dialog
                    .root
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned(),
            )
            .size(13)
            .style(theme::MUTED.text()),
        );
    }
    if matches!(
        dialog.action,
        Action::Create | Action::Rename | Action::Push
    ) {
        content = content
            .push(text(app.i18n.text(Key::GitBranchName)).size(13))
            .push(
                theme::text_field(&app.i18n.text(Key::GitBranchName), &dialog.name)
                    .id("git-branch-name")
                    .style(theme::validated_input(
                        dialog.error == Some(crate::domain::git::Error::InvalidName),
                    ))
                    .on_input(|name| Message::Git(GitMessage::Name(name)))
                    .on_submit(Message::Git(GitMessage::Submit)),
            );
    } else if dialog.action == Action::Delete {
        content = content.push(
            text(app.i18n.text(
                if dialog.branch.kind == crate::domain::repository::ReferenceKind::Remote {
                    Key::GitDeleteRemoteDescription
                } else {
                    Key::GitDeleteDescription
                },
            ))
            .size(14),
        );
    }
    if dialog.action == Action::Create {
        content = content.push(
            theme::switch(dialog.checkout)
                .label(app.i18n.text(Key::GitSwitchAfterCreate))
                .on_toggle(|value| Message::Git(GitMessage::Checkout(value))),
        );
    }
    if dialog.action == Action::Push {
        let remotes = app
            .repository
            .as_ref()
            .and_then(|repos| repos.get(&dialog.root))
            .map(|repo| repo.remotes.clone())
            .unwrap_or_default();
        content = content
            .push(text(app.i18n.text(Key::GitRemote)).size(13))
            .push(
                theme::Select::Standard
                    .view(remotes, Some(dialog.remote.clone()), |remote| {
                        Message::Git(GitMessage::Remote(remote))
                    })
                    .width(Length::Fill),
            );
    }
    if let Some(error) = dialog.error {
        content = content.push(
            text(app.i18n.text(git_actions::error_key(error)))
                .size(13)
                .style(theme::RED.text()),
        );
    }
    if app.state.git_actions.pending {
        content = content.push(
            text(app.i18n.text(Key::GitWorking))
                .size(13)
                .style(theme::MUTED.text()),
        );
    }
    content = content.push(
        modal::actions()
            .push(
                theme::text_button(app.i18n.text(Key::Cancel)).on_press_maybe(
                    (!app.state.git_actions.pending).then_some(Message::Git(GitMessage::Close)),
                ),
            )
            .push(
                container(
                    theme::text_button(app.i18n.text(action_key(dialog.action)))
                        .style(if dialog.action == Action::Delete {
                            theme::danger
                        } else {
                            theme::primary
                        })
                        .on_press_maybe(
                            (!app.state.git_actions.pending
                                && (matches!(dialog.action, Action::Delete | Action::Checkout)
                                    || !dialog.name.trim().is_empty())
                                && (dialog.action != Action::Push || !dialog.remote.is_empty()))
                            .then_some(Message::Git(GitMessage::Submit)),
                        ),
                )
                .id("git-confirm"),
            ),
    );
    crate::ui::modal::overlay(base, content.into(), 560., Message::Git(GitMessage::Close))
}

fn action_key(action: Action) -> Key {
    match action {
        Action::Checkout => Key::GitCheckout,
        Action::Create => Key::GitNewBranch,
        Action::Update => Key::GitUpdate,
        Action::Push => Key::GitPush,
        Action::Rename => Key::GitRename,
        Action::Delete => Key::GitDelete,
    }
}
