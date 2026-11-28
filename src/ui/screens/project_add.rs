use super::modal;
use crate::ui::theme::iced;
use crate::{
    app::{
        App, Message,
        project_add::{Error, Message as Event},
    },
    i18n::Key,
    ui::{
        icons::{Icon, icon},
        theme,
    },
};
use iced::{
    Alignment, Element, Length,
    widget::{button, column, container, row, text},
};

pub(super) fn overlay<'a>(app: &'a App, base: Element<'a, Message>) -> Element<'a, Message> {
    let Some(draft) = &app.project_add else {
        return base;
    };
    let available = !draft.pending && !draft.picking;
    let mut content = modal::content(app.i18n.text(if draft.cloning {
        Key::CloneRepository
    } else {
        Key::AddProject
    }));
    if draft.cloning {
        let input = theme::text_field(&app.i18n.text(Key::RepositoryUrlPlaceholder), &draft.remote)
            .id("clone-url")
            .style(theme::validated_input(
                draft.error == Some(Error::InvalidRemote),
            ));
        let destination =
            theme::text_field(&app.i18n.text(Key::CloneDestination), &draft.destination)
                .id("clone-destination")
                .style(theme::validated_input(matches!(
                    draft.error,
                    Some(Error::Destination | Error::Exists)
                )));
        content = content
            .push(theme::labeled_field(
                app.i18n.text(Key::RepositoryUrl),
                if available {
                    input.on_input(|value| Message::ProjectAdd(Event::Url(value)))
                } else {
                    input
                },
            ))
            .push(
                theme::labeled_field(
                    app.i18n.text(Key::CloneDestination),
                    if available {
                        destination.on_input(|value| Message::ProjectAdd(Event::Destination(value)))
                    } else {
                        destination
                    },
                )
                .push(
                    container(
                        theme::text_button(app.i18n.text(Key::ChooseCloneParent)).on_press_maybe(
                            available.then_some(Message::ProjectAdd(Event::PickParent)),
                        ),
                    )
                    .id("clone-pick-parent"),
                ),
            );
        if let Some(error) = draft.error {
            content = content.push(
                text(app.i18n.text(error_key(error)))
                    .size(13)
                    .style(theme::RED.text()),
            );
        }
        if draft.pending {
            content = content.push(
                text(app.i18n.text(Key::CloningRepository))
                    .size(14)
                    .style(theme::MUTED.text()),
            );
        }
    } else {
        content = content.push(
            text(app.i18n.text(Key::AddProjectSource))
                .size(14)
                .style(theme::MUTED.text()),
        );
        let choices = [
            (
                "add-project-directories",
                Key::AddProjectFolder,
                Icon::Folder,
                Event::Directories,
            ),
            (
                "add-project-remote",
                Key::CloneRepository,
                Icon::Branch,
                Event::Remote,
            ),
        ]
        .map(|(id, key, symbol, event)| {
            container(
                button(
                    container(
                        column![
                            icon(symbol, theme::MUTED),
                            text(app.i18n.text(key)).size(14).align_x(Alignment::Center),
                        ]
                        .spacing(6)
                        .align_x(Alignment::Center),
                    )
                    .center(Length::Fill),
                )
                .on_press(Message::ProjectAdd(event))
                .padding(12)
                .width(Length::Fill)
                .height(80)
                .style(theme::secondary),
            )
            .id(id)
            .into()
        });
        content = content.push(row(choices).spacing(12));
    }
    let mut actions = modal::actions().push(
        container(
            theme::text_button(app.i18n.text(Key::Cancel))
                .on_press_maybe((!draft.pending).then_some(Message::ProjectAdd(Event::Cancel))),
        )
        .id("add-project-cancel"),
    );
    if draft.cloning {
        actions = actions.push(
            container(
                theme::text_button(app.i18n.text(Key::CloneRepository))
                    .on_press_maybe(
                        (available
                            && !draft.remote.trim().is_empty()
                            && !draft.destination.trim().is_empty())
                        .then_some(Message::ProjectAdd(Event::Clone)),
                    )
                    .style(theme::primary),
            )
            .id("clone-submit"),
        );
    }
    modal::overlay(
        base,
        content.push(actions).into(),
        600.,
        Message::ProjectAdd(Event::Cancel),
    )
}

fn error_key(error: Error) -> Key {
    match error {
        Error::InvalidRemote => Key::CloneInvalidRemote,
        Error::Destination => Key::CloneInvalidDestination,
        Error::Exists => Key::CloneDestinationExists,
        Error::Failed => Key::CloneFailed,
    }
}
