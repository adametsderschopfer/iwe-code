//! Keep Docker operation fields together and their submit controls within reach.

use super::{GUTTER, caption, error_key, style};
use crate::{
    i18n::{I18n, Key},
    modules::docker::{Action, Draft, Event, Kind, State, container::Field},
    ui::{
        context_menu,
        icons::{Icon, sized_icon},
        theme,
    },
};
use iced::{
    Alignment, Element, Length,
    widget::{Column, button, column, container, row, scrollable, space, text, text_input},
};

fn field<'a, M: Clone + 'a>(
    i18n: &I18n,
    key: Key,
    value: &str,
    change: impl Fn(String) -> M + 'a,
) -> Element<'a, M> {
    column![
        text(i18n.text(key)).size(12).style(theme::MUTED.text()),
        text_input("", value)
            .on_input(change)
            .padding([6, 8])
            .size(13)
            .style(style::input)
    ]
    .spacing(3)
    .into()
}

fn group<'a, M: 'a>(title: String, fields: Column<'a, M>) -> Element<'a, M> {
    container(column![
        container(caption(title)).padding([5, 9]),
        context_menu::separator(),
        fields.spacing(GUTTER).padding(9)
    ])
    .padding(1)
    .width(Length::Fill)
    .style(style::frame)
    .into()
}

pub(super) fn contents<'a, M: Clone + 'a>(
    state: &'a State,
    draft: &'a Draft,
    i18n: &'a I18n,
    map: fn(Event) -> M,
    height: f32,
) -> Element<'a, M> {
    let action = draft.action;
    let target = state.find(draft.target.as_deref());
    let mut body = column![].spacing(GUTTER);
    let input = match action {
        Action::Create if state.kind == Kind::Containers => Some(Key::DockerImage),
        Action::Pull | Action::Tag => Some(Key::DockerImage),
        Action::Create | Action::AddContext => Some(Key::DockerName),
        Action::Terminal => Some(Key::DockerShell),
        Action::Connect | Action::Disconnect => Some(Key::DockerContainer),
        Action::Up => Some(Key::DockerComposeFile),
        Action::Build => Some(Key::Dockerfile),
        _ => None,
    };
    if action != Action::AddContext
        && let Some(connection) = &state.context
    {
        let mut context = column![
            row![
                sized_icon(Icon::Server, theme::MUTED, 15.),
                text(i18n.text(Key::DockerConnection))
                    .size(12)
                    .style(theme::MUTED.text()),
                text(connection)
                    .size(13)
                    .wrapping(text::Wrapping::WordOrGlyph)
            ]
            .spacing(6)
            .align_y(Alignment::Center)
        ]
        .spacing(4);
        if action != state.kind.create()
            && action != Action::Build
            && let Some(item) = target
        {
            context = context.push(
                text(&item.name)
                    .size(14)
                    .font(theme::semibold())
                    .wrapping(text::Wrapping::WordOrGlyph),
            );
        }
        body = body.push(
            container(context)
                .padding(GUTTER)
                .width(Length::Fill)
                .style(style::frame),
        );
    }
    if let Some(key) = input {
        let mut fields = column![field(i18n, key, &draft.value, move |v| map(Event::Value(
            v
        )))];
        if matches!(action, Action::Up | Action::Build) {
            fields = fields.push(
                button(text(i18n.text(Key::DeploymentChooseFile)).size(12))
                    .on_press(map(Event::Browse))
                    .style(style::button(theme::secondary)),
            );
        }
        let extra = match (action, state.kind) {
            (Action::AddContext, _) => Some(Key::DockerEndpoint),
            (Action::Build, _) => Some(Key::DockerImage),
            (Action::Create, Kind::Volumes | Kind::Networks) => Some(Key::DockerDriver),
            (Action::Create, Kind::Containers) => Some(Key::DockerOptionalName),
            _ => None,
        };
        if let Some(key) = extra {
            fields = fields.push(field(i18n, key, &draft.extra, move |v| {
                map(Event::Extra(v))
            }));
        }
        body = body.push(group(
            i18n.text(if action == Action::AddContext {
                Key::DockerConnection
            } else {
                Key::Settings
            }),
            fields,
        ));
    }
    if action == Action::Create && state.kind == Kind::Containers {
        let fields = Column::with_children(Field::ALL.into_iter().map(|kind| {
            field(i18n, kind.key(), draft.container.get(kind), move |v| {
                map(Event::ContainerField(kind, v))
            })
        }));
        body = body.push(group(i18n.text(Key::DockerRuntime), fields));
    }
    if action.confirm() {
        body = body.push(
            row![
                sized_icon(Icon::Warning, theme::YELLOW, 16.),
                text(i18n.text(
                    if action == Action::Remove
                        && state.kind == Kind::Containers
                        && target.is_some_and(super::super::Item::requires_force_removal)
                    {
                        Key::DockerConfirmStopRemove
                    } else {
                        Key::DockerConfirm
                    }
                ))
                .size(13)
            ]
            .spacing(6)
            .align_y(Alignment::Center),
        );
    }
    if let Some(error) = state.error {
        body = body.push(
            text(i18n.text(error_key(error)))
                .size(13)
                .style(theme::RED.text()),
        );
    }
    let ready = state.busy.is_none();
    let valid = draft.arguments(state.kind, target).is_some();
    column![
        text(i18n.text(action.key()))
            .font(theme::semibold())
            .size(19),
        context_menu::separator(),
        container(
            scrollable(body.padding(iced::Padding::default().right(7)))
                .height(Length::Shrink)
                .style(style::scroll)
        )
        .max_height((height - 102.).max(1.))
        .width(Length::Fill),
        context_menu::separator(),
        row![
            button(text(i18n.text(Key::Cancel)).size(13))
                .on_press_maybe(ready.then(|| map(Event::Dismiss)))
                .style(style::button(theme::action(false))),
            space().width(Length::Fill),
            container(
                button(text(i18n.text(state.busy.unwrap_or(action.key()))).size(13))
                    .on_press_maybe((valid && ready).then(|| map(Event::Submit)))
                    .style(style::button(theme::primary))
            )
            .id("docker-submit")
        ]
        .spacing(GUTTER)
        .align_y(Alignment::Center)
    ]
    .spacing(GUTTER)
    .into()
}
