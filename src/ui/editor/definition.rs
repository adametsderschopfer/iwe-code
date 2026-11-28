//! Let users choose among multiple definition locations.

use crate::ui::theme::iced;
use crate::{
    app::{App, Message, editor::Event},
    ui::{context_menu, theme},
};
use iced::{
    Element, Length,
    widget::{button, column, container, scrollable, text},
};

pub(super) fn overlay<'a>(app: &'a App, base: Element<'a, Message>) -> Element<'a, Message> {
    if app.editor.definitions.is_empty() {
        return base;
    }
    context_menu::overlay(
        base,
        iced::Point::new(24., 48.),
        iced::Size::new(560., 320.),
        move || {
            let mut rows = column![].spacing(2);
            for (index, definition) in app.editor.definitions.iter().enumerate() {
                let path = app
                    .workspace
                    .directory()
                    .and_then(|root| definition.path.strip_prefix(root).ok())
                    .unwrap_or(&definition.path);
                rows = rows.push(
                    button(
                        text(format!(
                            "{}:{}",
                            path.display(),
                            definition.position.line + 1
                        ))
                        .font(theme::selected_code_font(
                            app.workspace.preferences.code_font,
                        ))
                        .size(13),
                    )
                    .padding([8, 12])
                    .width(Length::Fill)
                    .style(theme::action(false))
                    .on_press(Message::Editor(Event::OpenDefinition(index))),
                );
            }
            container(scrollable(rows).style(theme::scroll).height(Length::Shrink))
                .max_height(320)
                .into()
        },
        Message::Editor(Event::DismissDefinitions),
    )
}
