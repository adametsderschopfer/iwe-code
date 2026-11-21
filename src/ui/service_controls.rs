//! Compact controls shared by the Docker registry and Kubernetes resource browsers.
use super::theme;
use crate::i18n::{I18n, Key};
use iced::{
    Element, Length, Theme,
    widget::{button, container, overlay::menu, pick_list, row, text, text_input},
};
pub(crate) fn action<'a, M: Clone + 'a>(i: &I18n, key: Key, event: Option<M>) -> Element<'a, M> {
    button(text(i.text(key)).size(12))
        .padding([4, 8])
        .on_press_maybe(event)
        .style(|t, s| {
            let mut v = theme::secondary(t, s);
            v.border.radius = Default::default();
            v
        })
        .into()
}
pub(crate) fn field<'a, M: Clone + 'a>(
    i: &I18n,
    key: Key,
    value: &str,
    change: impl Fn(String) -> M + 'a,
) -> Element<'a, M> {
    text_input(&i.text(key), value)
        .on_input(change)
        .size(13)
        .padding([4, 7])
        .style(|t, s| {
            let mut v = theme::input(t, s);
            v.border.radius = Default::default();
            v
        })
        .into()
}
pub(crate) fn entry<'a, M: Clone + 'a>(label: String, selected: bool, event: M) -> Element<'a, M> {
    button(super::ellipsis::text(label, 13.))
        .width(Length::Fill)
        .on_press(event)
        .padding([5, 8])
        .style(move |t, s| {
            let mut v = theme::tree_item(selected)(t, s);
            v.border.radius = Default::default();
            v
        })
        .into()
}
pub(crate) fn heading<'a, M: 'a>(value: String) -> Element<'a, M> {
    text(value).size(14).font(theme::semibold()).into()
}
pub(crate) fn divider<'a, M: 'a>() -> Element<'a, M> {
    container(iced::widget::space())
        .width(1)
        .height(Length::Fill)
        .style(theme::separator)
        .into()
}
pub(crate) fn choice<'a, M: Clone + 'a>(i: &I18n, title: Key, value: &str) -> Element<'a, M> {
    row![
        text(i.text(title)).size(12).style(theme::MUTED.text()),
        text(value.to_owned()).size(13)
    ]
    .spacing(8)
    .into()
}

pub(crate) fn select(theme: &Theme, status: pick_list::Status) -> pick_list::Style {
    let mut style = theme::select(theme, status);
    style.border.radius = Default::default();
    style
}

pub(crate) fn menu(theme: &Theme) -> menu::Style {
    let mut style = theme::select_menu(theme);
    style.border.radius = Default::default();
    style
}
