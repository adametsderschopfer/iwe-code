//! Resolve protocol colors while preserving readable defaults in both themes.

use crate::domain::theme::ColorRole;
use crate::ui::theme::iced;
use iced::{Color, Theme};

pub(crate) fn background(theme: &Theme) -> Color {
    theme.color(ColorRole::TerminalBackground)
}

pub(crate) fn foreground(theme: &Theme) -> Color {
    theme.color(ColorRole::TerminalText)
}

pub(crate) fn color(color: vt100::Color, default: Color, theme: &Theme) -> Color {
    match color {
        vt100::Color::Default => default,
        vt100::Color::Rgb(r, g, b) => Color::from_rgb8(r, g, b),
        vt100::Color::Idx(index @ 0..=15) => theme.color(ColorRole::ANSI[usize::from(index)]),
        vt100::Color::Idx(index @ 16..=231) => {
            let index = index - 16;
            let component = |value| if value == 0 { 0 } else { 55 + value * 40 };
            Color::from_rgb8(
                component(index / 36),
                component((index / 6) % 6),
                component(index % 6),
            )
        }
        vt100::Color::Idx(index) => {
            let value = 8 + (index - 232) * 10;
            Color::from_rgb8(value, value, value)
        }
    }
}

pub(super) const CELL_WIDTH: f32 = 0.6;
pub(super) const PADDING: f32 = 12.;
pub(super) const TAB_BAR_HEIGHT: f32 = 36.;
pub(super) const TAB_GAP: f32 = 6.;
pub(super) const TAB_CONTENT_GAP: f32 = 6.;
