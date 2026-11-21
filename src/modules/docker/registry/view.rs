use super::{Event, Form, Mode, State};
use crate::{
    domain::images::{Registry, repository, tag},
    i18n::{I18n, Key},
    ui::{
        context_menu,
        service_controls::{action, choice, divider, entry, field, heading, menu, select},
        theme,
    },
};
use iced::{
    Alignment, Element, Length,
    widget::{checkbox, column, container, pick_list, row, scrollable, text},
};

pub(crate) fn panel<'a>(
    s: &'a State,
    profiles: &'a [Registry],
    engine: Option<&'a str>,
    i: &'a I18n,
) -> Element<'a, Event> {
    let ready = !s.busy;
    let mut top = row![
        heading(i.text(Key::Registries)),
        pick_list(profiles, s.selected.as_ref(), Event::Select)
            .placeholder(i.text(Key::DockerConnection))
            .text_size(12)
            .style(select)
            .menu_style(menu),
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
    if s.busy {
        body = body.push(
            text(i.text(Key::Loading))
                .size(12)
                .style(theme::MUTED.text()),
        )
    }
    if let Some(form) = &s.form {
        let mut form_view = column![].spacing(7).padding(8);
        match form {
            Form::Edit {
                name,
                endpoint,
                issuer,
                login,
                ..
            } => {
                form_view = form_view
                    .push(heading(i.text(Key::RegistryConnection)))
                    .push(field(i, Key::DockerName, name, Event::Name))
                    .push(field(i, Key::RegistryEndpoint, endpoint, Event::Endpoint))
                    .push(
                        checkbox(*login)
                            .label(i.text(Key::RegistryCredentials))
                            .on_toggle(Event::Login),
                    );
                if *login {
                    form_view = form_view.push(field(i, Key::RegistryIssuer, issuer, Event::Issuer))
                }
                form_view = form_view.push(action(i, Key::RegistrySave, Some(Event::Save)));
            }
            Form::Remove => {
                if let Some(reg) = &s.selected {
                    form_view = form_view
                        .push(choice(i, Key::RegistryRemove, &reg.endpoint))
                        .push(action(i, Key::DockerRemove, Some(Event::Confirm)))
                }
            }
            Form::Transfer { context, target } => {
                if let Mode::Publish(image) = &s.mode {
                    form_view = form_view.push(choice(i, Key::RegistrySource, &image.id))
                }
                form_view = form_view
                    .push(choice(i, Key::DockerConnection, context))
                    .push(choice(i, Key::RegistryTarget, &target.to_string()))
                    .push(action(
                        i,
                        if matches!(s.mode, Mode::Publish(_)) {
                            Key::RegistryPublish
                        } else {
                            Key::DockerPull
                        },
                        ready.then_some(Event::Confirm),
                    ));
            }
        }
        return body
            .push(scrollable(form_view.push(action(
                i,
                Key::Cancel,
                Some(Event::Cancel),
            ))))
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
        return body.push(scrollable(form)).into();
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
    listing = listing.push(scrollable(rows).height(Length::Fill)).push(
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
        if engine.is_some() {
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
            .push(divider())
            .push(container(scrollable(details)).width(Length::FillPortion(1)))
    }
    body.push(panes).into()
}
