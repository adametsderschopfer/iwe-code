use crate::ui::theme;
use iced::{
    Element, Length,
    widget::{container, opaque, responsive, stack},
};

pub(crate) fn overlay<'a, Message: Clone + 'a>(
    base: Element<'a, Message>,
    content: Element<'a, Message>,
    width: f32,
    dismiss: Message,
) -> Element<'a, Message> {
    super::context_menu::dismiss_on_escape(
        stack![base, backdrop(content, width, theme::dialog)],
        dismiss,
    )
}

/// Supply the available dialog height to forms with a scrollable body and fixed actions.
pub(crate) fn bounded_overlay<'a, Message: Clone + 'a>(
    base: Element<'a, Message>,
    content: impl Fn(f32) -> Element<'a, Message> + 'a,
    width: f32,
    dismiss: Message,
    style: impl Fn(&iced::Theme) -> container::Style + Copy + 'a,
) -> Element<'a, Message> {
    let layer = responsive(move |size| {
        let available = (size.height - 2. * (WINDOW_INSET + DIALOG_INSET)).max(0.);
        backdrop(content(available), width, style)
    });
    super::context_menu::dismiss_on_escape(stack![base, layer], dismiss)
}

const WINDOW_INSET: f32 = 24.;
const DIALOG_INSET: f32 = 32.;

fn backdrop<'a, Message: 'a>(
    content: Element<'a, Message>,
    width: f32,
    style: impl Fn(&iced::Theme) -> container::Style + 'a,
) -> Element<'a, Message> {
    let dialog = container(content)
        .width(width)
        .padding(DIALOG_INSET)
        .style(style);
    let backdrop = container(dialog)
        .center(Length::Fill)
        .padding(WINDOW_INSET)
        .style(|_| container::Style {
            background: Some(iced::Color::from_rgba(0., 0., 0., 0.5).into()),
            ..Default::default()
        });
    opaque(backdrop)
}
