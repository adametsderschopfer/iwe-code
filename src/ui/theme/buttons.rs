//! Build labeled actions at the standard or compact size.

use crate::ui::theme::iced;
use iced::widget::{Button, button, text};

pub(crate) fn text_button<'a, Message: Clone + 'a>(
    label: impl text::IntoFragment<'a>,
) -> Button<'a, Message> {
    labeled_button(label, 14., [8, 14])
}

pub(crate) fn compact_button<'a, Message: Clone + 'a>(
    label: impl text::IntoFragment<'a>,
) -> Button<'a, Message> {
    labeled_button(label, 12., [4, 8])
}

fn labeled_button<'a, Message: Clone + 'a>(
    label: impl text::IntoFragment<'a>,
    size: f32,
    padding: [u16; 2],
) -> Button<'a, Message> {
    button(text(label).font(super::font()).size(size))
        .padding(padding)
        .style(super::secondary)
}
