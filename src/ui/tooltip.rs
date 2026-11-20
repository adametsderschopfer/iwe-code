//! Keep hover hints separated from their controls throughout the interface.

use crate::i18n::{I18n, Key};
pub(crate) use iced::widget::tooltip::Position;
use iced::{
    Element, Font,
    widget::{Tooltip, column, text},
};

pub(crate) fn view<'a, Message: 'a>(
    content: impl Into<Element<'a, Message>>,
    hint: impl Into<Element<'a, Message>>,
    position: Position,
) -> Tooltip<'a, Message> {
    iced::widget::tooltip(content, hint, position)
        .gap(8)
        .style(super::theme::tooltip)
}

/// Keep the copy action, success state, and full clipboard value in a single hover hint.
pub(crate) fn copy<'a, Message: 'a>(
    content: impl Into<Element<'a, Message>>,
    i18n: &I18n,
    action: Key,
    value: impl text::IntoFragment<'a>,
    copied: bool,
    position: Position,
) -> Tooltip<'a, Message> {
    view(
        content,
        column![
            text(i18n.text(if copied {
                Key::CopiedToClipboard
            } else {
                action
            }))
            .size(12),
            text(value)
                .size(11)
                .font(Font::MONOSPACE)
                .wrapping(text::Wrapping::WordOrGlyph),
        ]
        .spacing(4),
        position,
    )
    .style(if copied {
        super::theme::success_tooltip
    } else {
        super::theme::tooltip
    })
}

#[cfg(test)]
mod tests;
