//! Render terminal tab controls and position their shared context menu.

use super::{Event, TerminalState, icon_control, style};
use crate::{
    i18n::{I18n, Key},
    modules::terminal::state::{Tab, TitleAction, TitleControl},
    ui::{context_menu, icons::Icon, theme},
};
use iced::{
    Alignment, Element, Length, Point, Size, keyboard,
    widget::{button, column, container, mouse_area, row, scrollable, space, text},
};

pub(super) fn strip<'a>(terminal: &'a TerminalState, i18n: &'a I18n) -> Element<'a, Event> {
    let mut tabs = row![
        text(i18n.text(Key::Terminal))
            .size(13)
            .font(theme::semibold())
    ]
    .spacing(style::TAB_GAP)
    .align_y(Alignment::Center);
    for tab in &terminal.tabs {
        tabs = tabs.push(control(tab, i18n, terminal.active == Some(tab.request)));
    }
    tabs = tabs.push(icon_control(
        i18n,
        Icon::Plus,
        Key::TerminalNewTab,
        "terminal-add-tab".into(),
        Event::Start,
    ));
    let strip = scrollable(tabs)
        .id("terminal-tabs")
        .direction(scrollable::Direction::Horizontal(
            scrollable::Scrollbar::new(),
        ))
        .style(theme::scroll)
        .width(Length::Fill);
    context_menu::window_anchor(strip.into(), |event, point| match event {
        Event::Title(id, TitleAction::Menu(_)) => Event::Title(id, TitleAction::Menu(point)),
        other => other,
    })
}

fn control<'a>(tab: &'a Tab, i18n: &'a I18n, selected: bool) -> Element<'a, Event> {
    let request = tab.request;
    let label: Element<'a, Event> = if let TitleControl::Editing(draft) = &tab.title.control {
        let field = theme::compact_field(&tab.title.automatic(i18n), draft)
            .id(format!("terminal-rename-{request}"))
            .width(175)
            .on_input(move |text| Event::Title(request, TitleAction::Edit(text)))
            .on_submit(Event::Title(request, TitleAction::Save));
        context_menu::on_focused_key(field, move |event| {
            matches!(
                event,
                keyboard::Event::KeyPressed {
                    key: keyboard::Key::Named(keyboard::key::Named::Escape),
                    ..
                }
            )
            .then_some(Event::Title(request, TitleAction::Dismiss))
        })
    } else {
        button(
            row![
                text(tab.title.label(i18n))
                    .size(12)
                    .wrapping(text::Wrapping::None),
                icon_control(
                    i18n,
                    Icon::Close,
                    Key::TerminalCloseTab,
                    format!("terminal-close-tab-{request}"),
                    Event::Close(request),
                ),
            ]
            .spacing(style::TAB_CONTENT_GAP)
            .height(Length::Fill)
            .align_y(Alignment::Center),
        )
        .padding(iced::Padding {
            left: 10.,
            right: 4.,
            top: 2.,
            bottom: 2.,
        })
        .height(style::TAB_BAR_HEIGHT - 2.)
        .style(theme::action(false))
        .on_press(Event::Select(request))
        .into()
    };
    let line =
        container(space().height(2).width(Length::Fill)).style(theme::tab_indicator(selected));
    context_menu::trigger(
        mouse_area(
            container(column![label, line].width(Length::Shrink))
                .id(format!("terminal-tab-{request}")),
        )
        .on_middle_press(Event::Close(request)),
        move |point| Event::Title(request, TitleAction::Menu(point)),
    )
}

pub(super) fn menu(terminal: &TerminalState) -> Option<(u64, Point)> {
    terminal
        .tabs
        .iter()
        .find_map(|tab| match tab.title.control {
            TitleControl::Menu(point) => Some((tab.request, point)),
            _ => None,
        })
}

pub(crate) fn tab_menu<'a, M: Clone + 'a>(
    terminal: &TerminalState,
    i18n: &'a I18n,
    base: Element<'a, M>,
    map: fn(Event) -> M,
) -> Element<'a, M> {
    let Some((id, point)) = menu(terminal).filter(|_| terminal.open) else {
        return base;
    };
    context_menu::overlay(
        base,
        point,
        Size::new(180., 40.),
        move || {
            let key = Key::TerminalRenameTab;
            container(
                context_menu::action(key, i18n.text(key))
                    .on_press(map(Event::Title(id, TitleAction::Rename))),
            )
            .id("terminal-rename-action")
            .into()
        },
        map(Event::Title(id, TitleAction::Dismiss)),
    )
}
