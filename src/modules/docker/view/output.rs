use super::{GUTTER, error_key, pagination};
use crate::ui::presentation::IconButton;
use crate::ui::theme::iced;
use crate::{
    i18n::{I18n, Key},
    modules::docker::{
        Action, Event,
        output::{Content, Dialog, SCROLL_ID},
    },
    ui::{context_menu, ellipsis, icons::Icon, modal, theme},
};
use iced::{
    Alignment, Element, Length,
    widget::{column, container, row, scrollable, text},
};

pub(super) fn contents<'a>(dialog: &'a Dialog, i18n: &'a I18n) -> Element<'a, Event> {
    let ready = !matches!(dialog.content, Content::Loading { .. });
    let heading = row![
        modal::title(i18n.text(dialog.request.action.key())),
        ellipsis::text(&dialog.request.name, 14.),
    ]
    .spacing(GUTTER)
    .align_y(Alignment::Center);
    let header = modal::header(heading)
        .push(
            container(IconButton::Compact.view(
                i18n,
                Icon::Refresh,
                Key::DockerRefresh,
                ready.then_some(Event::Refresh),
            ))
            .id("docker-output-refresh"),
        )
        .push(
            container(IconButton::Compact.view(
                i18n,
                Icon::Close,
                Key::Dismiss,
                Some(Event::Dismiss),
            ))
            .id("docker-output-close"),
        );
    let mut contents = column![header, context_menu::separator()].spacing(GUTTER);
    let status = match &dialog.content {
        Content::Loading { .. } => Key::Loading,
        Content::Failed(error) => error_key(*error),
        Content::Ready { output, page } if !output.text.is_empty() => {
            contents = contents.push(
                scrollable(
                    text(output.page(*page).unwrap_or_default())
                        .font(iced::Font::MONOSPACE)
                        .size(13)
                        .wrapping(text::Wrapping::WordOrGlyph)
                        .width(Length::Fill),
                )
                .id(SCROLL_ID)
                .style(theme::scroll)
                .height(Length::Fill),
            );
            if output.offsets.len() > 1 {
                contents = contents.push(context_menu::separator()).push(pagination(
                    *page,
                    output.offsets.len(),
                    i18n,
                    Event::OutputPage,
                ));
            }
            return contents.into();
        }
        Content::Ready { .. } if dialog.request.action == Action::Logs => Key::DockerNoLogs,
        Content::Ready { .. } => Key::DockerNoDetails,
    };
    contents
        .push(container(text(i18n.text(status)).size(13)).center(Length::Fill))
        .into()
}
