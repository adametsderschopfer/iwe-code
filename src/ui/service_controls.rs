//! Share localized actions, fields, and resource rows across bundled modules.

use super::theme;
use crate::i18n::{I18n, Key};
use crate::ui::theme::iced;
use iced::{
    Element, Length,
    widget::{Button, button, row, text},
};
pub(crate) fn action<'a, M: Clone + 'a>(
    i: &I18n,
    key: Key,
    event: impl Into<Option<M>>,
) -> Button<'a, M> {
    theme::compact_button(i.text(key)).on_press_maybe(event.into())
}
pub(crate) fn field<'a, M: Clone + 'a>(
    i: &I18n,
    key: Key,
    value: &str,
    change: impl Fn(String) -> M + 'a,
) -> Element<'a, M> {
    theme::compact_field(&i.text(key), value)
        .on_input(change)
        .into()
}
pub(crate) fn entry<'a, M: Clone + 'a>(label: String, selected: bool, event: M) -> Element<'a, M> {
    button(super::ellipsis::text(label, 13.))
        .width(Length::Fill)
        .on_press(event)
        .padding([5, 8])
        .style(theme::tree_item(selected))
        .into()
}
pub(crate) fn heading<'a, M: 'a>(value: String) -> Element<'a, M> {
    text(value).size(14).font(theme::semibold()).into()
}
pub(crate) fn choice<'a, M: Clone + 'a>(i: &I18n, title: Key, value: &str) -> Element<'a, M> {
    row![
        text(i.text(title)).size(12).style(theme::MUTED.text()),
        text(value.to_owned()).size(13)
    ]
    .spacing(8)
    .into()
}
