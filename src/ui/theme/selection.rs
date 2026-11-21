//! Build form and toolbar selectors with the shared menu and control styles.

use iced::widget::{self, Checkbox, PickList, Toggler};
use std::borrow::Borrow;

#[derive(Clone, Copy)]
pub(crate) enum Select {
    Standard,
    Compact,
}

impl Select {
    pub(crate) fn view<'a, T, L, V, Message: Clone + 'a>(
        self,
        options: L,
        selected: Option<V>,
        on_selected: impl Fn(T) -> Message + 'a,
    ) -> PickList<'a, T, L, V, Message>
    where
        T: ToString + PartialEq + Clone + 'a,
        L: Borrow<[T]> + 'a,
        V: Borrow<T> + 'a,
    {
        let (size, padding) = match self {
            Self::Standard => (13, [8, 12]),
            Self::Compact => (12, [4, 8]),
        };
        widget::pick_list(options, selected, on_selected)
            .font(super::font())
            .text_size(size)
            .padding(padding)
            .style(super::select)
            .menu_style(super::select_menu)
    }
}

pub(crate) fn switch<'a, Message: 'a>(checked: bool) -> Toggler<'a, Message> {
    widget::toggler(checked).style(super::toggle_style)
}

pub(crate) fn checkbox<'a, Message: 'a>(checked: bool) -> Checkbox<'a, Message> {
    widget::checkbox(checked).style(super::checkbox_style)
}
