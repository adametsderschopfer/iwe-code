//! Present update actions without exposing transport or installer details.

use crate::{
    app::{
        App, Message,
        updates::{Event, State},
    },
    i18n::{FluentArgs, Key},
    ui::{icons::Icon, modal, presentation::icon_button, theme},
};
use iced::{
    Alignment, Element,
    widget::{column, container, row, scrollable, text},
};

pub(crate) fn header_button(app: &App) -> Option<Element<'_, Message>> {
    app.updates.version()?;
    Some(
        container(icon_button(
            &app.i18n,
            Icon::Update,
            Key::UpdateDetails,
            Message::Update(Event::OpenDialog),
            false,
        ))
        .id("header-update")
        .into(),
    )
}

pub(crate) fn settings(app: &App) -> Element<'_, Message> {
    let (label, action) = match &app.updates {
        State::Idle | State::Current | State::Failed(_) => (Key::CheckUpdates, Some(Event::Check)),
        State::Checking => (Key::CheckingUpdates, None),
        _ => (Key::UpdateDetails, Some(Event::OpenDialog)),
    };
    let description = match &app.updates {
        State::Current => app.i18n.text(Key::UpToDate),
        State::Failed(error) => app.i18n.text(crate::app::updates::error_key(*error)),
        _ => app.updates.version().map_or_else(
            || app.i18n.text(Key::UpdatesDescription),
            |number| version(app, number),
        ),
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
    if app.update_prompt_visible()
        && let Some(version) = app.updates.version()
    {
        return prompt(app, base, version);
    }

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

fn prompt<'a>(app: &'a App, base: Element<'a, Message>, version: &'a str) -> Element<'a, Message> {
    modal::bounded_overlay(
        base,
        move |available| {
            let (label, action) = match &app.updates {
                State::Available(_) => (Key::DownloadUpdate, Some(Event::Download)),
                State::Downloading(_) => (Key::DownloadingUpdate, None),
                State::Ready(_) => (Key::InstallUpdate, Some(Event::Install)),
                _ => (Key::InstallingUpdate, None),
            };
            let versions = row![
                text(env!("CARGO_PKG_VERSION"))
                    .size(17)
                    .style(theme::MUTED.text()),
                text("→").size(17).style(theme::MUTED.text()),
                text(version)
                    .size(22)
                    .font(theme::semibold())
                    .style(theme::TEXT.text()),
            ]
            .spacing(10)
            .align_y(Alignment::Center);
            let mut notes = column![].spacing(9);
            if app.update_dialog.notes.is_empty() {
                notes = notes.push(text(app.i18n.text(Key::UpdateNoNotes)).size(14));
            }
            for note in &app.update_dialog.notes {
                notes = notes.push(row![text("•").size(14), text(note).size(14)].spacing(10));
            }
            let show_checkbox = matches!(app.updates, State::Available(_));
            let error = app.update_dialog.download_error;
            let reserved = 114.
                + if show_checkbox { 38. } else { 0. }
                + if error.is_some() { 55. } else { 0. };
            let mut content = modal::content(app.i18n.text(Key::UpdatePrompt))
                .push(versions)
                .push(
                    container(scrollable(notes).style(theme::scroll))
                        .max_height((available - reserved).max(0.)),
                );
            if show_checkbox {
                content = content.push(
                    theme::checkbox(app.update_dialog.install_after_download)
                        .label(app.i18n.text(Key::UpdateInstallAfterDownload))
                        .on_toggle(|checked| Message::Update(Event::InstallAfterDownload(checked))),
                );
            }
            if let Some(key) = error {
                content = content.push(text(app.i18n.text(key)).size(13));
            }
            let installing = matches!(app.updates, State::Saving(_) | State::Installing(_));
            content
                .push(
                    modal::actions()
                        .push(
                            theme::text_button(app.i18n.text(Key::UpdateLater)).on_press_maybe(
                                (!installing).then_some(Message::Update(Event::DismissDialog)),
                            ),
                        )
                        .push(
                            theme::text_button(app.i18n.text(label))
                                .style(theme::primary)
                                .on_press_maybe(action.map(Message::Update)),
                        ),
                )
                .into()
        },
        580.,
        Message::Update(Event::DismissDialog),
    )
}
