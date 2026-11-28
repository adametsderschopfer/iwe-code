//! Use the application theme as the default for Iced widget types.

pub(crate) use super::Theme;
pub(crate) use ::iced::*;

pub(crate) type Element<'a, Message, T = Theme, R = Renderer> = ::iced::Element<'a, Message, T, R>;

pub(crate) mod widget {
    use super::{Renderer, Theme};
    pub(crate) use ::iced::widget::*;

    pub(crate) type Button<'a, M> = ::iced::widget::Button<'a, M, Theme>;
    pub(crate) type Checkbox<'a, M> = ::iced::widget::Checkbox<'a, M, Theme>;
    pub(crate) type Column<'a, M> = ::iced::widget::Column<'a, M, Theme>;
    pub(crate) type Container<'a, M> = ::iced::widget::Container<'a, M, Theme>;
    pub(crate) type PickList<'a, T, L, V, M> = ::iced::widget::PickList<'a, T, L, V, M, Theme>;
    pub(crate) type Row<'a, M> = ::iced::widget::Row<'a, M, Theme>;
    pub(crate) type Text<'a> = ::iced::widget::Text<'a, Theme>;
    pub(crate) type TextEditor<'a, H, M> = ::iced::widget::TextEditor<'a, H, M, Theme>;
    pub(crate) type TextInput<'a, M> = ::iced::widget::TextInput<'a, M, Theme>;
    pub(crate) type Toggler<'a, M> = ::iced::widget::Toggler<'a, M, Theme>;
    pub(crate) type Tooltip<'a, M> = ::iced::widget::Tooltip<'a, M, Theme>;

    pub(crate) fn text<'a>(value: impl text::IntoFragment<'a>) -> Text<'a> {
        let widget = ::iced::widget::text::<Theme, Renderer>(value);
        match super::super::runtime::interface_override() {
            Some(font) => widget.font(font),
            None => widget,
        }
    }
}
