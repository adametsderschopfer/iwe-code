//! Use the workspace palette and interaction states with square Docker surfaces.

use crate::ui::theme;
use iced::{
    Theme,
    widget::{
        button::{Status as ButtonStatus, Style as ButtonStyle},
        container, scrollable, text_input,
    },
};

pub(super) fn surface(
    style: fn(&Theme) -> container::Style,
) -> impl Fn(&Theme) -> container::Style + Copy {
    move |theme| {
        let mut surface = style(theme);
        surface.border.radius = Default::default();
        surface
    }
}

pub(in crate::modules::docker) fn button(
    style: impl Fn(&Theme, ButtonStatus) -> ButtonStyle,
) -> impl Fn(&Theme, ButtonStatus) -> ButtonStyle {
    move |theme, status| {
        let mut button = style(theme, status);
        button.border.radius = Default::default();
        button
    }
}

pub(super) fn input(theme: &Theme, status: text_input::Status) -> text_input::Style {
    let mut style = theme::input(theme, status);
    style.border.radius = Default::default();
    style
}

pub(super) use crate::ui::service_controls::{menu, select};

pub(super) fn scroll(theme: &Theme, status: scrollable::Status) -> scrollable::Style {
    let mut style = theme::scroll(theme, status);
    style.horizontal_rail.scroller.border.radius = Default::default();
    style.vertical_rail.scroller.border.radius = Default::default();
    style.auto_scroll.border.radius = Default::default();
    style
}
