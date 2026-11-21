//! Compose dialogs from shared surfaces, titles, actions, and keyboard handling.

use crate::ui::theme;
use iced::{
    Alignment, Element, Length,
    widget::{
        Column, Row, Text, column, container, mouse_area, opaque, responsive, row, space, stack,
        text,
    },
};

pub(crate) const CONTENT_GAP: f32 = 14.;
const TITLE_SIZE: f32 = 20.;
const WINDOW_INSET: f32 = 24.;
const PANEL_INSET: f32 = 24.;

pub(crate) fn title<'a>(label: impl Into<String>) -> Text<'a> {
    text(label.into()).size(TITLE_SIZE).font(theme::semibold())
}

pub(crate) fn clipped_title<'a, Message: 'a>(label: String) -> Element<'a, Message> {
    super::ellipsis::text_with_font(label, TITLE_SIZE, theme::semibold())
}

pub(crate) fn content<'a, Message: 'a>(label: impl Into<String>) -> Column<'a, Message> {
    column![title(label)].spacing(CONTENT_GAP)
}

pub(crate) fn header<'a, Message: 'a>(
    heading: impl Into<Element<'a, Message>>,
) -> Row<'a, Message> {
    row![container(heading).width(Length::Fill)]
        .spacing(8)
        .align_y(Alignment::Center)
}

pub(crate) fn actions<'a, Message: 'a>() -> Row<'a, Message> {
    row![space().width(Length::Fill)]
        .spacing(8)
        .align_y(Alignment::Center)
}

fn panel<'a, Message: 'a>(
    content: impl Into<Element<'a, Message>>,
    width: f32,
) -> Element<'a, Message> {
    container(content)
        .width(Length::Fill)
        .max_width(width)
        .padding(PANEL_INSET)
        .style(theme::dialog)
        .into()
}

fn layer<'a, Message: 'a>(
    base: Element<'a, Message>,
    foreground: Element<'a, Message>,
) -> Element<'a, Message> {
    let backdrop = container(foreground)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(theme::backdrop);
    stack![base, super::focus::modal_scope(opaque(backdrop))].into()
}

fn centered_panel<'a, Message: 'a>(
    content: Element<'a, Message>,
    width: f32,
) -> Element<'a, Message> {
    container(panel(content, width))
        .center(Length::Fill)
        .padding(WINDOW_INSET)
        .into()
}

pub(crate) fn overlay<'a, Message: Clone + 'a>(
    base: Element<'a, Message>,
    content: Element<'a, Message>,
    width: f32,
    dismiss: Message,
) -> Element<'a, Message> {
    super::context_menu::dismiss_on_escape(layer(base, centered_panel(content, width)), dismiss)
}

/// Position the panel below the window header and dismiss it on a backdrop press.
pub(crate) fn top_overlay<'a, Message: Clone + 'a>(
    base: Element<'a, Message>,
    content: Element<'a, Message>,
    width: f32,
    top: f32,
    dismiss: Message,
) -> Element<'a, Message> {
    let area = container(opaque(panel(content, width)))
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(Alignment::Center)
        .padding(iced::Padding {
            top,
            ..Default::default()
        });
    let outside = mouse_area(space().width(Length::Fill).height(Length::Fill)).on_press(dismiss);
    layer(base, stack![outside, area].into())
}

/// Supply the available dialog height to forms with a scrollable body and fixed actions.
pub(crate) fn bounded_overlay<'a, Message: Clone + 'a>(
    base: Element<'a, Message>,
    content: impl Fn(f32) -> Element<'a, Message> + 'a,
    width: f32,
    dismiss: Message,
) -> Element<'a, Message> {
    let foreground = responsive(move |size| {
        let available = (size.height - 2. * (WINDOW_INSET + PANEL_INSET)).max(0.);
        centered_panel(content(available), width)
    });
    super::context_menu::dismiss_on_escape(layer(base, foreground.into()), dismiss)
}

#[cfg(test)]
mod tests;
