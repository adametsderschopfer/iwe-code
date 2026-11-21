//! Use the workspace palette and interaction states with square Docker surfaces.

use crate::ui::theme;
use iced::{
    Border, Theme,
    widget::{
        button::{Status as ButtonStatus, Style as ButtonStyle},
        container,
        overlay::menu,
        pick_list, scrollable, text_input,
    },
};

pub(super) fn frame(theme: &Theme) -> container::Style {
    container::Style {
        border: Border {
            color: theme::LINE.resolve(theme),
            width: 1.,
            ..Default::default()
        },
        ..theme::surface(theme)
    }
}

pub(super) fn surface(
    style: fn(&Theme) -> container::Style,
) -> impl Fn(&Theme) -> container::Style + Copy {
    move |theme| {
        let mut surface = style(theme);
        surface.border.radius = Default::default();
        surface
    }
}

pub(super) fn button(
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

pub(super) fn select(theme: &Theme, status: pick_list::Status) -> pick_list::Style {
    let mut style = theme::select(theme, status);
    style.border.radius = Default::default();
    style
}

pub(super) fn menu(theme: &Theme) -> menu::Style {
    let mut style = theme::select_menu(theme);
    style.border.radius = Default::default();
    style
}

pub(super) fn scroll(theme: &Theme, status: scrollable::Status) -> scrollable::Style {
    let mut style = theme::scroll(theme, status);
    style.horizontal_rail.scroller.border.radius = Default::default();
    style.vertical_rail.scroller.border.radius = Default::default();
    style.auto_scroll.border.radius = Default::default();
    style
}
