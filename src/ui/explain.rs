//! Share the Codex explanation action across workspace and bundled module views.

use super::{context_menu, modal, theme};
use crate::{
    app::{App, Message, explain::Event},
    domain::explain::{Agent, Target},
    i18n::{I18n, Key},
};
use iced::{
    Element, Length, Size,
    widget::{button, column, row, text},
};

pub(crate) fn submenu<'a, M: Clone + 'a>(
    i18n: &I18n,
    target: Target,
    choose: impl Fn(Target, Agent) -> M + 'a,
) -> Element<'a, M> {
    button(text(i18n.text(Key::Explain)).size(14))
        .width(Length::Fill)
        .padding([10, 12])
        .style(theme::secondary)
        .on_press(choose(target, Agent::Codex))
        .into()
}

pub(crate) fn choose(target: Target, agent: Agent) -> Message {
    Message::Explain(Event::Choose(target, agent))
}

pub(crate) fn overlay<'a>(app: &'a App, base: Element<'a, Message>) -> Element<'a, Message> {
    let base = if let Some((target, point)) = &app.explain.menu {
        context_menu::overlay(
            base,
            *point,
            Size::new(240., 58.),
            move || submenu(&app.i18n, target.clone(), choose),
            Message::Explain(Event::CloseMenu),
        )
    } else {
        base
    };
    let Some(request) = &app.explain.setup else {
        return base;
    };
    let action = |key, event| {
        button(text(app.i18n.text(key)).size(14))
            .padding([8, 12])
            .style(theme::secondary)
            .on_press_maybe((!request.busy).then_some(Message::Explain(event)))
    };
    let body = if request.busy {
        column![text(app.i18n.text(Key::ExplainOpening)).size(14)]
    } else {
        column![
            text(app.i18n.text(Key::ExplainSetup)).size(14),
            row![
                action(Key::ExplainLocate, Event::Locate),
                action(Key::ExplainInstall, Event::Install),
                action(Key::ExplainRetry, Event::Retry),
            ]
            .spacing(8)
            .wrap(),
        ]
        .spacing(16)
    };
    modal::overlay(
        base,
        column![
            text("Codex").font(theme::semibold()).size(22),
            body,
            button(text(app.i18n.text(Key::Cancel)).size(14))
                .padding([8, 12])
                .style(theme::secondary)
                .on_press(Message::Explain(Event::Cancel)),
        ]
        .spacing(16)
        .into(),
        480.,
        Message::Explain(Event::Cancel),
    )
}
