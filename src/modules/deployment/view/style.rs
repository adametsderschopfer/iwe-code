//! Use the warning tone for deployment notices.

use crate::ui::theme;
use iced::{Border, Theme, widget::container};

pub(super) fn warning(theme: &Theme) -> container::Style {
    let color = theme::YELLOW.resolve(theme);
    container::Style {
        background: Some(iced::Color { a: 0.10, ..color }.into()),
        text_color: Some(theme::TEXT.resolve(theme)),
        border: Border {
            color: iced::Color { a: 0.35, ..color },
            width: 1.,
            radius: theme::CONTROL_RADIUS.into(),
        },
        ..Default::default()
    }
}
