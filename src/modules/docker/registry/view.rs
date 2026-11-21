use super::{Editor, Event, Form, Mode, State};
use crate::{
    domain::images::{Registry, repository, tag},
    i18n::{I18n, Key},
    ui::{
        context_menu, modal,
        presentation::vertical_divider,
        service_controls::{action, choice, entry, field, heading},
        theme,
    },
};
use iced::{
    Alignment, Element, Length,
    widget::{column, container, row, scrollable, space, text},
};

pub(in crate::modules::docker) fn edit_overlay<'a, M: Clone + 'a>(
    editor: &'a Editor,
    i: &'a I18n,
    base: Element<'a, M>,
    map: impl Fn(Event) -> M + Copy + 'a,
) -> Element<'a, M> {
    modal::bounded_overlay(
        base,
        move |height| {
            let input = |key, value: &str, change: fn(String) -> Event| {
                theme::labeled_field(
                    i.text(key),
                    theme::text_field("", value).on_input(move |v| map(change(v))),
                )
            };
            let mut fields = column![
                input(Key::DockerName, &editor.name, Event::Name),
                input(Key::RegistryEndpoint, &editor.endpoint, Event::Endpoint),
                theme::checkbox(editor.login)
                    .label(i.text(Key::RegistryCredentials))
                    .on_toggle(move |v| map(Event::Login(v))),
            ]
            .spacing(8);
            if editor.login {
                fields = fields.push(input(Key::RegistryIssuer, &editor.issuer, Event::Issuer));
            }
            if let Some(key) = editor.error {
                fields = fields.push(text(i.text(key)).size(13).style(theme::RED.text()));
            }
            modal::content(i.text(if editor.existing.is_some() {
                Key::RegistryConnection
            } else {
                Key::RegistryAdd
            }))
            .push(
                container(
                    scrollable(fields)
                        .height(Length::Shrink)
                        .style(theme::scroll),
                )
                .width(Length::Fill)
                .max_height((height - 102.).max(1.)),
            )
            .push(
                modal::actions()
                    .push(theme::text_button(i.text(Key::Cancel)).on_press(map(Event::Cancel)))
                    .push(
                        theme::text_button(i.text(Key::RegistrySave))
                            .style(theme::primary)
                            .on_press(map(Event::Save)),
                    ),
            )
            .into()
        },
        516.,
        map(Event::Cancel),
    )
}

pub(crate) fn panel<'a>(
    s: &'a State,
    profiles: &'a [Registry],
    engine: Option<&'a str>,
    i: &'a I18n,
) -> Element<'a, Event> {
    let ready = !s.busy;
    let mut top = row![
        heading(i.text(Key::Registries)),
        theme::Select::Compact
            .view(profiles, s.selected.as_ref(), Event::Select)
            .placeholder(i.text(Key::DockerConnection)),
        action(i, Key::RegistryAdd, Some(Event::Edit(false)))
    ]
    .spacing(6)
    .align_y(Alignment::Center);
    if s.selected.is_some() {
        for (key, event) in [
            (Key::RegistryEdit, Event::Edit(true)),
            (Key::RegistryCheck, Event::Check),
            (Key::DockerRemove, Event::Remove),
        ] {
            top = top.push(action(i, key, ready.then_some(event)))
        }
    }
    let mut body = column![
        container(top.wrap()).padding([4, 8]),
        context_menu::separator()
    ]
    .height(Length::Fill);
    if let Some(key) = s.notice {
        body = body.push(text(i.text(key)).size(12))
    }
    if s.busy
        && (!s.page.names.is_empty()
            || matches!(s.mode, Mode::Publish(_))
            || matches!(s.form, Some(Form::Transfer { .. })))
    {
        body = body.push(
            text(i.text(Key::Loading))
                .size(12)
                .style(theme::MUTED.text()),
        );
    }
    let form_view = match &s.form {
        Some(Form::Remove) => s.selected.as_ref().map(|reg| {
            column![
                choice(i, Key::RegistryRemove, &reg.endpoint),
                action(i, Key::DockerRemove, Some(Event::Confirm)).style(theme::danger),
            ]
        }),
        Some(Form::Transfer { context, target }) => {
            let mut form = column![];
            if let Mode::Publish(image) = &s.mode {
                form = form.push(choice(i, Key::RegistrySource, &image.id));
            }
            Some(
                form.push(choice(i, Key::DockerConnection, context))
                    .push(choice(i, Key::RegistryTarget, &target.to_string()))
                    .push(
                        action(
                            i,
                            if matches!(s.mode, Mode::Publish(_)) {
                                Key::RegistryPublish
                            } else {
                                Key::DockerPull
                            },
                            ready.then_some(Event::Confirm),
                        )
                        .style(theme::primary),
                    ),
            )
        }
        _ => None,
    };
    if let Some(form_view) = form_view {
        let form_view = form_view.spacing(7).padding(8);
        return body
            .push(
                scrollable(form_view.push(action(i, Key::Cancel, Some(Event::Cancel))))
                    .style(theme::scroll),
            )
            .into();
    }
    if let Mode::Publish(image) = &s.mode {
        let form = column![
            heading(i.text(Key::RegistryPublish)),
            choice(i, Key::RegistrySource, &image.id),
            choice(i, Key::DockerConnection, engine.unwrap_or_default()),
            field(i, Key::RegistryRepository, &s.input, Event::RepoInput),
            field(
                i,
                Key::RegistryVersion,
                &s.version_input,
                Event::VersionInput
            ),
            row![
                action(
                    i,
                    Key::RegistryReview,
                    (ready
                        && engine.is_some()
                        && s.selected.is_some()
                        && repository(&s.input)
                        && tag(&s.version_input))
                    .then_some(Event::Review)
                ),
                action(i, Key::Cancel, ready.then_some(Event::Cancel))
            ]
            .spacing(5)
        ]
        .padding(8)
        .spacing(6);
        return body.push(scrollable(form).style(theme::scroll)).into();
    }
    if s.picking() {
        body = body.push(
            row![
                heading(i.text(Key::RegistryPick)),
                space().width(Length::Fill),
                action(i, Key::Cancel, Some(Event::Return))
            ]
            .padding(5),
        )
    }
    if let Some(image) = &s.published {
        body = body.push(
            row![
                choice(i, Key::RegistryPublished, &image.to_string()),
                action(i, Key::RegistryCopy, Some(Event::Copy(image.clone())))
            ]
            .spacing(6),
        )
    }
    if s.selected.is_none() {
        return body.into();
    }
    let search = row![
        field(i, Key::RegistryRepository, &s.input, Event::RepoInput),
        action(
            i,
            Key::RegistryOpen,
            repository(&s.input).then(|| Event::Repository(s.input.clone()))
        ),
        action(i, Key::RegistryCatalog, Some(Event::Catalog))
    ]
    .spacing(4)
    .padding(5);
    let mut listing = column![search, context_menu::separator()].width(Length::FillPortion(1));
    if !s.repo.is_empty() {
        listing = listing.push(
            row![
                field(
                    i,
                    Key::RegistryVersion,
                    &s.version_input,
                    Event::VersionInput
                ),
                action(
                    i,
                    Key::DockerInspect,
                    (!s.version_input.is_empty()).then(|| Event::Version(s.version_input.clone()))
                )
            ]
            .spacing(4)
            .padding(5),
        )
    }
    let rows = column(s.page.names.iter().map(|n| {
        entry(
            n.clone(),
            n == &s.version,
            if s.repo.is_empty() {
                Event::Repository(n.clone())
            } else {
                Event::Version(n.clone())
            },
        )
    }));
    let results: Element<'_, Event> = if s.busy && s.page.names.is_empty() {
        container(
            text(i.text(Key::Loading))
                .size(13)
                .style(theme::MUTED.text()),
        )
        .center(Length::Fill)
        .into()
    } else {
        scrollable(rows)
            .style(theme::scroll)
            .height(Length::Fill)
            .into()
    };
    listing = listing.push(results).push(
        row![
            action(
                i,
                Key::PreviousFilePart,
                (s.history.len() > 1 && ready).then_some(Event::Previous)
            ),
            action(
                i,
                Key::NextFilePart,
                (s.page.next.is_some() && ready).then_some(Event::Next)
            ),
            action(i, Key::DockerRefresh, ready.then_some(Event::Refresh))
        ]
        .spacing(5)
        .padding(4),
    );
    let mut panes = row![listing].height(Length::Fill);
    if let Some(m) = &s.manifest
        && let Some(img) = s.image(false)
    {
        let mut details = column![
            heading(i.text(Key::DockerDetails)),
            text(img.to_string()).size(13),
            choice(i, Key::RegistryDigest, &m.digest),
            choice(i, Key::RegistryPlatforms, &m.platforms.join(", ")),
            action(i, Key::RegistryCopy, Some(Event::Copy(img.clone())))
        ]
        .padding(8)
        .spacing(6);
        if s.picking() {
            if tag(&s.version) {
                details = details.push(action(
                    i,
                    Key::RegistryChooseTag,
                    Some(Event::Choose(false)),
                ))
            }
            details = details.push(action(i, Key::RegistryChoose, Some(Event::Choose(true))))
        } else if engine.is_some() {
            if tag(&s.version) {
                details = details.push(action(
                    i,
                    Key::RegistryPullTag,
                    ready.then_some(Event::Pull(false)),
                ))
            }
            details = details.push(action(
                i,
                Key::RegistryPullDigest,
                ready.then_some(Event::Pull(true)),
            ))
        } else {
            details = details.push(text(i.text(Key::RegistryEngine)).size(12))
        }
        panes = panes
            .push(vertical_divider())
            .push(container(scrollable(details).style(theme::scroll)).width(Length::FillPortion(1)))
    }
    body.push(panes).into()
}
