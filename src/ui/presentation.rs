use crate::domain::Failure;
use crate::{
    i18n::{I18n, Key},
    ui::{
        icons::{Icon, sized_icon},
        theme, tooltip,
    },
};
use iced::{
    Alignment, Element, Length,
    widget::{Button, Container, button, container, space, text},
};

/// Center toolbar icons inside their click targets.
#[derive(Clone, Copy)]
pub(crate) enum IconButton {
    Header,
    Tool,
    Compact,
    Small,
    Inline(f32),
}

impl IconButton {
    pub(crate) fn button<'a, M: Clone + 'a>(
        self,
        symbol: Icon,
        message: Option<M>,
        selected: bool,
    ) -> Button<'a, M> {
        let (glyph, target) = match self {
            Self::Header => (20., theme::WORKSPACE_HEADER_BUTTON_SIZE),
            Self::Tool => (20., 30.),
            Self::Compact => (16., 24.),
            Self::Small => (14., 22.),
            Self::Inline(target) => (20., target),
        };
        let color = if message.is_none() {
            theme::FAINT
        } else if selected {
            theme::TEXT
        } else {
            theme::MUTED
        };
        button(container(sized_icon(symbol, color, glyph)).center(target))
            .padding(0)
            .on_press_maybe(message)
            .style(theme::icon_action(selected))
    }

    pub(crate) fn view<'a, M: Clone + 'a>(
        self,
        i18n: &I18n,
        symbol: Icon,
        label: Key,
        message: Option<M>,
    ) -> Element<'a, M> {
        icon_hint(self.button(symbol, message, false), i18n, label)
    }
}

fn icon_hint<'a, M: 'a>(
    control: impl Into<Element<'a, M>>,
    i18n: &I18n,
    label: Key,
) -> Element<'a, M> {
    tooltip::view(
        control,
        text(i18n.text(label)).size(12),
        tooltip::Position::Bottom,
    )
    .into()
}

pub(crate) fn icon_button<'a, Message: Clone + 'a>(
    i18n: &I18n,
    symbol: Icon,
    label: Key,
    message: Message,
    active: bool,
) -> Element<'a, Message> {
    icon_hint(
        IconButton::Header.button(symbol, Some(message), active),
        i18n,
        label,
    )
}

pub(crate) fn divider<'a, Message: 'a>() -> Element<'a, Message> {
    container(space())
        .height(1)
        .width(Length::Fill)
        .style(theme::separator)
        .into()
}

pub(crate) fn vertical_divider<'a, Message: 'a>() -> Container<'a, Message> {
    container(space())
        .width(1)
        .height(Length::Fill)
        .style(theme::separator)
}

pub(crate) fn center_message<'a, Message: 'a>(
    i18n: &crate::i18n::I18n,
    key: Key,
) -> Element<'a, Message> {
    container(
        text(i18n.text(key))
            .size(14)
            .style(theme::MUTED.text())
            .align_x(Alignment::Center),
    )
    .padding(20)
    .center(Length::Fill)
    .into()
}

pub(crate) fn short(value: &str, limit: usize) -> String {
    if value.chars().count() <= limit {
        value.to_owned()
    } else {
        format!("{}…", value.chars().take(limit).collect::<String>())
    }
}

pub(crate) fn error_key(error: Failure) -> Key {
    match error {
        Failure::MissingTool => Key::GitMissing,
        Failure::NotRepository => Key::NotRepository,
        Failure::Read => Key::ReadFailed,
        Failure::TooLarge => Key::TooLarge,
        Failure::Binary => Key::Binary,
        Failure::Access => Key::AccessRequired,
        Failure::Unavailable => Key::Unavailable,
        Failure::Storage => Key::StorageSaveFailed,
        Failure::NewerVersion => Key::StorageNewerVersion,
        Failure::Credentials => Key::CredentialsUnavailable,
        Failure::AuthNotConfigured => Key::AuthNotConfigured,
        Failure::AuthCancelled => Key::Cancelled,
        Failure::AuthExpired => Key::AuthExpired,
        Failure::AuthDenied => Key::AuthDenied,
        Failure::AuthPortBusy => Key::AuthPortBusy,
        Failure::InvalidToken => Key::InvalidToken,
        Failure::PermissionDenied => Key::PermissionDenied,
        Failure::RateLimited => Key::RateLimited,
    }
}
