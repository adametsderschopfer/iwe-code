//! Present a compact Go to Line dialog above the source preview.

use crate::ui::modal;
use crate::ui::theme::iced;
use crate::{
    app::{App, Message, line_navigation::Event},
    i18n::Key,
    ui::theme,
};
use iced::{Element, widget::text};

pub(super) fn overlay<'a>(app: &'a App, base: Element<'a, Message>) -> Element<'a, Message> {
    let Some(dialog) = &app.line_navigation else {
        return base;
    };
    let input = theme::text_field(&app.i18n.text(Key::LineNumber), &dialog.value)
        .id("go-to-line-number")
        .style(theme::validated_input(matches!(
            dialog.error,
            Some(Key::InvalidLineNumber | Key::LineNumberNotFound)
        )));
    let input = if dialog.loading {
        input
    } else {
        input
            .on_input(|value| Message::GoToLine(Event::Changed(value)))
            .on_submit(Message::GoToLine(Event::Submit))
    };
    let mut content = modal::content(app.i18n.text(Key::GoToLine))
        .push(
            text(app.i18n.text(Key::GoToLineDescription))
                .size(13)
                .style(theme::MUTED.text()),
        )
        .push(input);
    if let Some(error) = dialog.error {
        content = content.push(text(app.i18n.text(error)).size(13).style(theme::RED.text()));
    }
    content = content.push(
        modal::actions()
            .push(
                theme::text_button(app.i18n.text(Key::Cancel))
                    .on_press(Message::GoToLine(Event::Cancel)),
            )
            .push(
                theme::text_button(app.i18n.text(if dialog.loading {
                    Key::Loading
                } else {
                    Key::GoToLine
                }))
                .on_press_maybe((!dialog.loading).then_some(Message::GoToLine(Event::Submit)))
                .style(theme::primary),
            ),
    );
    super::modal::overlay(base, content.into(), 420., Message::GoToLine(Event::Cancel))
}
