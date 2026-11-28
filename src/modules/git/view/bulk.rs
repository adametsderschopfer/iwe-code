//! Review branch batches before sending repository operations.
use super::{Event as Message, View};
use crate::modules::git::actions::{
    self as git_actions, Message as GitMessage,
    bulk::{Action, Event},
};
use crate::ui::modal;

use crate::ui::theme::iced;
use crate::{
    domain::repository::ReferenceKind,
    i18n::{FluentArgs, Key},
    ui::{context_menu, ellipsis, theme, tooltip},
};
use iced::{
    Alignment, Element, Length, Size,
    widget::{Column, column, container, row, scrollable, text},
};
use std::collections::BTreeSet;

pub(super) fn count(app: View<'_>, count: usize) -> String {
    let mut args = FluentArgs::new();
    args.set("count", count);
    app.i18n.format(Key::BranchesSelected, &args)
}

pub(crate) fn overlay<'a>(app: View<'a>, base: Element<'a, Message>) -> Element<'a, Message> {
    let base = if let Some(point) = app.state.git_actions.bulk.menu {
        context_menu::overlay(
            base,
            point,
            Size::new(300. + context_menu::ICON_COLUMN_WIDTH, 110.),
            move || {
                let mut content = Column::new().spacing(2);
                for action in [Action::Push, Action::Pull, Action::Delete] {
                    if action == Action::Delete {
                        content = content.push(context_menu::separator());
                    }
                    content = content.push(
                        container(
                            context_menu::action(action.label(), app.i18n.text(action.label()))
                                .on_press_maybe(app.bulk_allowed(action).then_some(Message::Git(
                                    GitMessage::Bulk(Event::Choose(action)),
                                ))),
                        )
                        .id(format!("git-bulk-{action:?}")),
                    );
                }
                content.into()
            },
            Message::Git(GitMessage::Close),
        )
    } else {
        base
    };
    let Some(dialog) = &app.state.git_actions.bulk.dialog else {
        return base;
    };
    let mut content = modal::content(app.i18n.text(dialog.action.label()))
        .push(text(count(app, dialog.targets.len())).size(14));
    let description = match dialog.action {
        Action::Delete
            if dialog
                .targets
                .iter()
                .any(|target| target.branch.kind == ReferenceKind::Remote) =>
        {
            Key::GitBulkDeleteRemoteDescription
        }
        Action::Delete => Key::GitBulkDeleteDescription,
        Action::Push => Key::GitBulkPushDescription,
        Action::Pull => Key::GitBulkPullDescription,
    };
    content = content.push(text(app.i18n.text(description)).size(14));
    let mut targets = Column::new().spacing(12);
    for target in &dialog.targets {
        let name = target
            .key
            .root
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        let mut entry = column![
            tooltip::view(
                ellipsis::text(&target.branch.name, 14.),
                text(&target.branch.name).size(12),
                tooltip::Position::Top
            ),
            text(name).size(12).style(theme::MUTED.text()),
        ]
        .spacing(4);
        if let Some(error) = target.error {
            entry = entry.push(
                text(app.i18n.text(git_actions::error_key(error)))
                    .size(13)
                    .style(theme::RED.text()),
            );
        }
        targets = targets.push(entry);
    }
    let mut review = column![targets].spacing(16).width(Length::Fill);
    let mut review_height = dialog.targets.len() as f32 * 78.;
    if dialog.action == Action::Push {
        let roots: BTreeSet<_> = dialog
            .targets
            .iter()
            .map(|target| target.key.root.clone())
            .collect();
        for root in roots {
            let remotes = app
                .repository
                .as_ref()
                .and_then(|repos| repos.get(&root))
                .map(|repo| repo.remotes.clone())
                .unwrap_or_default();
            let selected = dialog
                .targets
                .iter()
                .find(|target| target.key.root == root)
                .map(|target| target.remote.clone());
            let name = root
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned();
            review_height += 56.;
            review = review.push(
                row![
                    column![
                        text(name).size(13),
                        text(app.i18n.text(Key::GitRemote))
                            .size(12)
                            .style(theme::MUTED.text())
                    ]
                    .width(Length::Fill)
                    .spacing(4),
                    theme::Select::Standard.view(remotes, selected, move |remote| Message::Git(
                        GitMessage::Bulk(Event::Remote(root.clone(), remote))
                    )),
                ]
                .spacing(12)
                .align_y(Alignment::Center),
            );
        }
    }
    content = content.push(
        scrollable(review)
            .height(review_height.clamp(78., 220.))
            .style(theme::scroll),
    );
    if app.state.git_actions.pending {
        content = content.push(
            text(app.i18n.text(Key::GitWorking))
                .size(13)
                .style(theme::MUTED.text()),
        );
    }
    let can_submit = !app.state.git_actions.pending
        && !dialog.targets.is_empty()
        && (dialog.action != Action::Push
            || dialog
                .targets
                .iter()
                .all(|target| !target.remote.is_empty()));
    content = content.push(
        modal::actions()
            .push(
                theme::text_button(app.i18n.text(Key::Cancel)).on_press_maybe(
                    (!app.state.git_actions.pending).then_some(Message::Git(GitMessage::Close)),
                ),
            )
            .push(
                container(
                    theme::text_button(app.i18n.text(dialog.action.label()))
                        .style(if dialog.action == Action::Delete {
                            theme::danger
                        } else {
                            theme::primary
                        })
                        .on_press_maybe(
                            can_submit.then_some(Message::Git(GitMessage::Bulk(Event::Submit))),
                        ),
                )
                .id("git-bulk-confirm"),
            ),
    );
    crate::ui::modal::overlay(base, content.into(), 620., Message::Git(GitMessage::Close))
}
