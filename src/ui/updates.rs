//! Present update actions without exposing transport or installer details.

use crate::{
    app::{
        App, Message,
        updates::{Event, State},
    },
    i18n::{FluentArgs, Key},
    ui::{icons::Icon, theme},
};
use iced::{Element, widget::container};

pub(crate) fn sidebar(app: &App) -> Option<Element<'_, Message>> {
    let (label, action) = match &app.updates {
        State::Available(_) => (Key::DownloadUpdate, Some(Event::Download)),
        State::Downloading => (Key::DownloadingUpdate, None),
        State::Ready(_) => (Key::InstallUpdate, Some(Event::Install)),
        State::Saving(_) | State::Installing(_) => (Key::InstallingUpdate, None),
        _ => return None,
    };
    Some(crate::ui::screens::workspace::navigation_button(
        app,
        Icon::Update,
        label,
        action.map(Message::Update),
        false,
        "sidebar-update",
    ))
}

pub(crate) fn settings(app: &App) -> Element<'_, Message> {
    let (label, action) = match &app.updates {
        State::Idle | State::Current | State::Failed(_) => (Key::CheckUpdates, Some(Event::Check)),
        State::Checking => (Key::CheckingUpdates, None),
        State::Available(_) => (Key::DownloadUpdate, Some(Event::Download)),
        State::Downloading => (Key::DownloadingUpdate, None),
        State::Ready(_) => (Key::InstallUpdate, Some(Event::Install)),
        State::Saving(_) | State::Installing(_) => (Key::InstallingUpdate, None),
    };
    let description = match &app.updates {
        State::Current => app.i18n.text(Key::UpToDate),
        State::Available(release) => version(app, &release.version),
        State::Ready(prepared) | State::Saving(prepared) | State::Installing(prepared) => {
            version(app, &prepared.version)
        }
        State::Failed(error) => app.i18n.text(crate::app::updates::error_key(*error)),
        _ => app.i18n.text(Key::UpdatesDescription),
    };
    crate::ui::screens::settings::setting(
        app.i18n.text(Key::Updates),
        description,
        container(
            theme::text_button(app.i18n.text(label))
                .on_press_maybe(action.map(Message::Update))
                .style(theme::primary),
        )
        .id("settings-update")
        .into(),
    )
}

fn version(app: &App, version: &str) -> String {
    let mut args = FluentArgs::new();
    args.set("version", version);
    app.i18n.format(Key::UpdateAvailable, &args)
}

pub(crate) fn overlay<'a>(app: &'a App, base: Element<'a, Message>) -> Element<'a, Message> {
    use iced::widget::{column, scrollable, text};

    let Some(release) = app.whats_new.as_ref().filter(|_| app.loaded) else {
        return base;
    };
    let mut args = FluentArgs::new();
    args.set("version", release.version.as_str());
    let title = app.i18n.format(Key::UpdateWhatsNew, &args);
    let dismiss = Message::Update(Event::DismissNotes);
    crate::ui::modal::bounded_overlay(
        base,
        move |available| {
            let mut changes = column![].spacing(10);
            let mut notes = release
                .notes(app.i18n.language(), std::env::consts::OS)
                .peekable();
            if notes.peek().is_none() {
                changes = changes.push(text(app.i18n.text(Key::UpdateNoNotes)).size(14));
            }
            for note in notes {
                changes = changes
                    .push(iced::widget::row![text("•").size(14), text(note).size(14)].spacing(10));
            }
            crate::ui::modal::content(title.clone())
                .push(
                    container(scrollable(changes).style(theme::scroll))
                        .max_height((available - 100.).max(0.)),
                )
                .push(
                    crate::ui::modal::actions()
                        .push(
                            theme::text_button(app.i18n.text(Key::UpdateReleasePage))
                                .on_press(Message::OpenUrl(release.url())),
                        )
                        .push(
                            theme::text_button(app.i18n.text(Key::UpdateContinue))
                                .style(theme::primary)
                                .on_press(Message::Update(Event::DismissNotes)),
                        ),
                )
                .into()
        },
        580.,
        dismiss,
    )
}
