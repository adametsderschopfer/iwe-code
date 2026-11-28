//! Keep Docker operation fields together and their submit controls within reach.

use super::{GUTTER, caption, error_key};
use crate::ui::modal;
use crate::ui::theme::iced;
use crate::{
    i18n::{I18n, Key},
    modules::docker::{Action, Draft, Event, Kind, State, container::Field},
    ui::{
        icons::{Icon, sized_icon},
        theme,
    },
};
use iced::{
    Alignment, Element, Length,
    widget::{Column, column, container, row, scrollable, text},
};

fn field<'a, M: Clone + 'a>(
    i18n: &I18n,
    key: Key,
    value: &str,
    change: impl Fn(String) -> M + 'a,
) -> Element<'a, M> {
    theme::labeled_field(
        i18n.text(key),
        theme::text_field("", value).on_input(change),
    )
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
        body = body.push(context);
    }
    if let Some(key) = input {
        let mut fields = column![field(i18n, key, &draft.value, move |v| map(Event::Value(
            v
        )))];
        if matches!(action, Action::Up | Action::Build) {
            fields = fields.push(
                theme::compact_button(i18n.text(Key::DeploymentChooseFile))
                    .on_press(map(Event::Browse)),
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
        body = body.push(fields.spacing(GUTTER));
    }
    if action == Action::Create && state.kind == Kind::Containers {
        let fields = Column::with_children(Field::ALL.into_iter().map(|kind| {
            field(i18n, kind.key(), draft.container.get(kind), move |v| {
                map(Event::ContainerField(kind, v))
            })
        }));
        body = body
            .push(caption(i18n.text(Key::DockerRuntime)))
            .push(fields.spacing(GUTTER));
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
    modal::content(i18n.text(action.key()))
        .push(
            container(
                scrollable(body.padding(iced::Padding::default().right(7)))
                    .height(Length::Shrink)
                    .style(theme::scroll),
            )
            .max_height((height - 102.).max(1.))
            .width(Length::Fill),
        )
        .push(
            modal::actions()
                .push(
                    theme::text_button(i18n.text(Key::Cancel))
                        .on_press_maybe(ready.then(|| map(Event::Dismiss))),
                )
                .push(
                    container(
                        theme::text_button(i18n.text(state.busy.unwrap_or(action.key())))
                            .on_press_maybe((valid && ready).then(|| {
                                map(if action == Action::Terminal {
                                    Event::Shell(state.request)
                                } else {
                                    Event::Submit
                                })
                            }))
                            .style(if action == Action::Remove {
                                theme::danger
                            } else {
                                theme::primary
                            }),
                    )
                    .id("docker-submit"),
                ),
        )
        .into()
}
