//! Render terminal search controls with the shared compact search field.

use super::{Event, icon_control};
use crate::ui::theme::iced;
use crate::{
    i18n::{FluentArgs, I18n, Key},
    modules::terminal::{
        search::{Action, Search},
        state::search_id,
    },
    ui::{
        context_menu,
        icons::{self, Icon},
        theme,
    },
};
use iced::{
    Alignment, Element, Length, keyboard,
    widget::{container, opaque, row, text},
};

pub(super) fn shortcut(
    key: &keyboard::Key,
    physical: keyboard::key::Physical,
    modifiers: keyboard::Modifiers,
) -> bool {
    let required = if cfg!(target_os = "macos") {
        keyboard::Modifiers::LOGO
    } else {
        keyboard::Modifiers::CTRL | keyboard::Modifiers::SHIFT
    };
    modifiers == required
        && (physical == keyboard::key::Physical::Code(keyboard::key::Code::KeyF)
            || matches!(key, keyboard::Key::Character(letter) if letter.eq_ignore_ascii_case("f")))
}

pub(super) fn bar<'a>(
    i18n: &I18n,
    request: u64,
    search: &'a Search,
    interactive: bool,
) -> Element<'a, Event> {
    let (current, total) = search.count();
    let mut args = FluentArgs::new();
    args.set("current", current.to_string());
    args.set("total", total.to_string());
    let input = theme::compact_field(&i18n.text(Key::TerminalFind), &search.query)
        .id(search_id(request))
        .on_input(move |value| Event::Search(request, Action::Query(value)));
    let controls = row![
        icons::icon(Icon::Search, theme::MUTED),
        input,
        text(i18n.format(Key::TerminalMatchCount, &args))
            .size(12)
            .style(theme::MUTED.text()),
        icon_control(
            i18n,
            Icon::Up,
            Key::FindPrevious,
            format!("terminal-find-previous-{request}"),
            Event::Search(request, Action::Move(false))
        ),
        icon_control(
            i18n,
            Icon::Down,
            Key::FindNext,
            format!("terminal-find-next-{request}"),
            Event::Search(request, Action::Move(true))
        ),
        icon_control(
            i18n,
            Icon::Close,
            Key::FindClose,
            format!("terminal-find-close-{request}"),
            Event::Search(request, Action::Close)
        ),
    ]
    .spacing(4)
    .align_y(Alignment::Center);
    let panel = context_menu::on_focused_key(
        container(controls)
            .id("terminal-search-panel")
            .max_width(360)
            .padding([4, 8])
            .width(Length::Fill)
            .style(theme::search_panel),
        move |event| {
            let keyboard::Event::KeyPressed {
                key,
                physical_key,
                modifiers,
                ..
            } = event
            else {
                return None;
            };
            interactive.then_some(())?;
            if shortcut(key, *physical_key, *modifiers) {
                return Some(Event::Search(request, Action::Open));
            }
            let action = match key {
                keyboard::Key::Named(keyboard::key::Named::Escape) => Action::Close,
                keyboard::Key::Named(keyboard::key::Named::Enter) => {
                    Action::Move(!modifiers.shift())
                }
                _ => return None,
            };
            Some(Event::Search(request, action))
        },
    );
    container(opaque(panel))
        .padding([6, 8])
        .align_right(Length::Fill)
        .into()
}
