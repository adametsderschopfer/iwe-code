//! Present available Docker operations, their results and confirmations.

mod form;
mod output;

use super::{Action, Error, Event, Kind, PAGE, State};
use crate::{
    i18n::{I18n, Key},
    ui::{
        context_menu, ellipsis,
        icons::{Icon, sized_icon},
        modal,
        presentation::{IconButton, vertical_divider},
        theme, tool_rail,
    },
};
use iced::{
    Alignment, Element, Length,
    widget::{button, column, container, responsive, row, scrollable, space, text},
};

pub(super) fn error_key(error: Error) -> Key {
    match error {
        Error::Stopped => Key::DockerStopped,
        Error::Unavailable => Key::DockerOffline,
        Error::Missing => Key::DockerMissing,
        Error::Timeout => Key::DockerTimeout,
        Error::Large => Key::DockerLarge,
        Error::Invalid => Key::DockerInvalid,
        Error::InUse => Key::DockerInUse,
        Error::Denied => Key::DockerDenied,
        Error::NotFound => Key::DockerNotFound,
        Error::ComposeMissing => Key::DockerComposeMissing,
        Error::Refused => Key::DockerRefused,
    }
}

const GUTTER: f32 = 8.;
const HEADER: f32 = theme::PANEL_HEADER_HEIGHT;

const COMPACT_WIDTH: f32 = 720.;

fn caption<'a>(value: String) -> iced::widget::Text<'a> {
    text(value)
        .size(12)
        .font(theme::semibold())
        .style(theme::MUTED.text())
}

fn glyph(kind: Kind) -> Icon {
    match kind {
        Kind::Containers => Icon::Cube,
        Kind::Images => Icon::Duplicate,
        Kind::Networks => Icon::Network,
        Kind::Volumes => Icon::Volume,
        Kind::Compose => Icon::Server,
    }
}

fn section<'a>(
    id: &'static str,
    header: Element<'a, Event>,
    contents: Element<'a, Event>,
    width: Length,
) -> Element<'a, Event> {
    container(column![
        container(header)
            .padding([0., GUTTER])
            .height(HEADER)
            .align_y(Alignment::Center),
        context_menu::separator(),
        contents,
    ])
    .id(id)
    .width(width)
    .height(Length::Fill)
    .style(theme::surface)
    .into()
}

fn heading<'a>(state: &'a State, i18n: &I18n, width: f32) -> Element<'a, Event> {
    let ready = state.busy.is_none();
    let mut heading = row![
        sized_icon(Icon::Docker, theme::BLUE, 21.),
        text(i18n.text(Key::Docker))
            .font(theme::semibold())
            .size(15),
        vertical_divider().height(18),
    ]
    .spacing(GUTTER)
    .align_y(Alignment::Center);
    if width >= 520. {
        heading = heading.push(caption(i18n.text(Key::DockerConnection)));
    }
    heading
        .push(
            theme::Select::Compact
                .view(
                    state.contexts.as_slice(),
                    state.context.as_ref(),
                    Event::Context,
                )
                .placeholder(i18n.text(Key::DockerConnection))
                .width((width - 185.).clamp(55., 188.)),
        )
        .push(IconButton::Compact.view(
            i18n,
            Icon::Plus,
            Key::DockerAddContext,
            ready.then_some(Event::Act(Action::AddContext)),
        ))
        .push(IconButton::Compact.view(
            i18n,
            Icon::Refresh,
            Key::DockerRefresh,
            ready.then_some(Event::Connections),
        ))
        .push(space().width(Length::Fill))
        .push(IconButton::Compact.view(i18n, Icon::Close, Key::DockerClose, Some(Event::Hide)))
        .into()
}

fn navigation<'a>(state: &State, i18n: &I18n) -> Element<'a, Event> {
    let mut links = column![];
    for (kind, id) in Kind::ALL.into_iter().zip([
        "docker-kind-containers",
        "docker-kind-images",
        "docker-kind-networks",
        "docker-kind-volumes",
        "docker-kind-compose",
    ]) {
        links = links.push(tool_rail::item(
            id,
            glyph(kind),
            i18n.text(kind.key()),
            !state.registry_open && kind == state.kind,
            (state.busy.is_none() || state.loading_inventory()).then_some(Event::Kind(kind)),
        ));
    }
    if super::registry::VISIBLE {
        links = links.push(tool_rail::item(
            "docker-registries",
            Icon::Tag,
            i18n.text(Key::Registries),
            state.registry_open,
            Some(Event::Registry(super::registry::Event::Open)),
        ));
    }
    tool_rail::view("docker-navigation", "docker-navigation-scroll", links)
}

fn status_key(kind: Kind) -> Key {
    match kind {
        Kind::Containers | Kind::Compose => Key::DockerStatus,
        Kind::Images => Key::DockerSize,
        Kind::Networks | Kind::Volumes => Key::DockerScope,
    }
}

fn inventory<'a>(state: &'a State, i18n: &'a I18n, width: f32) -> Element<'a, Event> {
    let ready = state.busy.is_none();
    let create = state.kind.create();
    let search = theme::compact_field(&i18n.text(Key::DockerFilter), &state.query)
        .id("docker-filter")
        .on_input(Event::Search);
    let mut header = row![
        text(i18n.text(state.kind.key()))
            .font(theme::semibold())
            .size(14),
        container(text(state.filtered.len().to_string()).size(11))
            .padding([1, 6])
            .style(theme::badge),
        search.width(Length::Fill),
    ]
    .align_y(Alignment::Center)
    .spacing(5);
    let command = |action: Action, symbol| -> Element<'a, Event> {
        let available = ready && state.context.is_some() && state.connection_error.is_none();
        let event = available.then_some(Event::Act(action));
        if width < 440. {
            IconButton::Compact.view(i18n, symbol, action.key(), event)
        } else {
            button(
                row![
                    sized_icon(symbol, theme::MUTED, 14.),
                    text(i18n.text(action.key())).size(12)
                ]
                .align_y(Alignment::Center)
                .spacing(4),
            )
            .on_press_maybe(event)
            .padding([3, 6])
            .style(theme::secondary)
            .into()
        }
    };
    let symbol = if create == Action::Pull {
        Icon::Fetch
    } else {
        Icon::Plus
    };
    header = header.push(command(create, symbol));
    if state.kind == Kind::Images {
        header = header.push(command(Action::Build, Icon::Cube));
    }
    header = header.push(IconButton::Compact.view(
        i18n,
        Icon::Refresh,
        Key::DockerRefresh,
        ready.then_some(Event::Refresh),
    ));
    if let Some(error) = state.connection_error {
        return section(
            "docker-inventory",
            header.into(),
            container(
                text(i18n.text(error_key(error)))
                    .size(13)
                    .style(theme::MUTED.text()),
            )
            .padding(GUTTER)
            .center(Length::Fill)
            .into(),
            Length::Fill,
        );
    }
    let mut content = column![];
    if !state.filtered.is_empty() {
        content = content
            .push(
                container(
                    row![
                        caption(i18n.text(Key::DockerName)).width(Length::FillPortion(3)),
                        caption(i18n.text(status_key(state.kind))).width(Length::FillPortion(2)),
                    ]
                    .spacing(GUTTER),
                )
                .width(Length::Fill)
                .padding([3, 12])
                .style(theme::sidebar),
            )
            .push(context_menu::separator());
    }
    let mut rows = column![].spacing(1).padding([2, 4]);
    let shown = (state.extra_batches + 1) * PAGE;
    for &index in state.filtered.iter().take(shown) {
        let item = &state.items[index];
        let id = &item.id;
        let color = if item.state == "running" {
            theme::GREEN
        } else {
            theme::MUTED
        };
        let label = row![
            container(ellipsis::text(item.name.as_str(), 13.)).width(Length::FillPortion(3)),
            container(ellipsis::text(item.status.as_str(), 12.))
                .width(Length::FillPortion(2))
                .style(move |theme| container::Style {
                    text_color: Some(color.resolve(theme)),
                    ..Default::default()
                })
        ]
        .spacing(GUTTER)
        .align_y(Alignment::Center);
        let row = button(label)
            .width(Length::Fill)
            .padding([4, 8])
            .on_press_maybe(ready.then(|| Event::Select(id.clone())))
            .style(theme::tree_item(state.selected.as_ref() == Some(id)));
        let resource = context_menu::trigger(row, move |point| Event::Menu(id.clone(), point));
        let mut resource = container(resource).width(Length::Fill);
        if state.selected.as_ref() == Some(id) {
            resource = resource.id("docker-selected-row");
        }
        rows = rows.push(resource);
    }
    let initial_load =
        state.busy == Some(Key::Loading) && (state.loading_inventory() || state.context.is_none());
    if state.filtered.len() > shown {
        rows = rows.push(load_more(shown, Event::MoreRows(state.kind, shown)));
    }
    let loading = state.loading_inventory() || initial_load;
    let delayed = loading && slow_request(state);
    if state.filtered.is_empty() && loading {
        content = content.push(loading_footer(i18n, delayed));
    } else if state.filtered.is_empty() && ready && state.error.is_none() {
        let key = if state.query.trim().is_empty() {
            Key::DockerEmptyList
        } else {
            Key::DockerEmpty
        };
        content = content.push(
            container(text(i18n.text(key)).size(13).style(theme::MUTED.text()))
                .center(Length::Fill),
        );
    } else {
        if delayed {
            rows = rows.push(loading_label(i18n));
        }
        content = content.push(context_menu::window_anchor(
            scrollable(rows)
                .id("docker-rows")
                .height(Length::Fill)
                .style(theme::scroll)
                .into(),
            |event, point| match event {
                Event::Menu(id, _) => Event::Menu(id, point),
                event => event,
            },
        ));
    }
    section(
        "docker-inventory",
        header.into(),
        content.into(),
        Length::Fill,
    )
}

fn load_more<'a>(shown: usize, event: Event) -> Element<'a, Event> {
    iced::widget::sensor(space().width(Length::Fill).height(1))
        .key(shown)
        .anticipate(100.)
        .on_show(move |_| event.clone())
        .into()
}

fn slow_request(state: &State) -> bool {
    state
        .started
        .is_some_and(|start| start.elapsed() >= std::time::Duration::from_secs(1))
}

fn loading_label<'a>(i18n: &I18n) -> Element<'a, Event> {
    container(
        text(i18n.text(Key::Loading))
            .size(13)
            .style(theme::MUTED.text()),
    )
    .padding(GUTTER)
    .center_x(Length::Fill)
    .into()
}

fn loading_footer<'a>(i18n: &I18n, visible: bool) -> Element<'a, Event> {
    let label = visible.then(|| loading_label(i18n));
    container(column(label)).align_bottom(Length::Fill).into()
}

fn pagination<'a>(
    page: usize,
    pages: usize,
    i18n: &I18n,
    event: fn(bool) -> Event,
) -> Element<'a, Event> {
    row![
        space().width(Length::Fill),
        IconButton::Compact.view(
            i18n,
            Icon::Back,
            Key::PreviousFilePart,
            (page > 0).then(|| event(false))
        ),
        text(format!("{} / {}", page + 1, pages)).size(12),
        IconButton::Compact.view(
            i18n,
            Icon::Right,
            Key::NextFilePart,
            (page + 1 < pages).then(|| event(true))
        ),
    ]
    .spacing(4)
    .padding([2, 5])
    .align_y(Alignment::Center)
    .into()
}

fn running_list<'a>(state: &'a State, i18n: &I18n) -> Element<'a, Event> {
    let mut contents = column![].height(Length::Fill);
    let shown = (state.running_batches + 1) * PAGE;
    if let Some(rows) = &state.running
        && !rows.is_empty()
    {
        let mut buttons =
            iced::widget::Column::with_children(rows.iter().take(shown).map(|item| {
                button(ellipsis::text(&item.name, 13.))
                    .width(Length::Fill)
                    .padding([5, 8])
                    .on_press_maybe(
                        state
                            .busy
                            .is_none()
                            .then(|| Event::OpenContainer(item.id.clone())),
                    )
                    .style(theme::tree_item(false))
                    .into()
            }));
        if rows.len() > shown {
            buttons = buttons.push(load_more(shown, Event::MoreRunningRows(shown)));
        }
        if state.busy == Some(Key::Loading) && slow_request(state) {
            buttons = buttons.push(loading_label(i18n));
        }
        contents = contents.push(
            scrollable(buttons)
                .height(Length::Fill)
                .style(theme::scroll),
        );
    } else if state.running.is_none() && state.busy == Some(Key::Loading) {
        contents = contents.push(loading_footer(i18n, slow_request(state)));
    } else if state.error.is_none() {
        let key = Key::DockerNoRunningContainers;
        contents = contents.push(
            container(text(i18n.text(key)).size(13).style(theme::MUTED.text()))
                .padding(GUTTER)
                .center(Length::Fill),
        );
    }
    contents.into()
}

fn properties<'a>(
    state: &'a State,
    item: &'a super::Item,
    i18n: &I18n,
    terminal: bool,
    compact: bool,
) -> Element<'a, Event> {
    let mut header = row![].spacing(6).align_y(Alignment::Center);
    if compact {
        header = header.push(
            container(IconButton::Compact.view(
                i18n,
                Icon::Back,
                Key::DockerBack,
                state.busy.is_none().then_some(Event::BackToList),
            ))
            .id("docker-back"),
        );
    }
    if state.kind == Kind::Images {
        for (linked, key) in [(false, Key::DockerDetails), (true, Key::DockerContainers)] {
            header = header.push(
                theme::compact_button(i18n.text(key))
                    .on_press_maybe(
                        (linked != state.show_running && (!linked || state.busy.is_none()))
                            .then_some(Event::ShowRunning(linked)),
                    )
                    .style(theme::action(linked == state.show_running)),
            );
        }
        if state.show_running {
            return section(
                "docker-properties",
                header.into(),
                running_list(state, i18n),
                Length::Fill,
            );
        }
    } else {
        header = header.push(caption(i18n.text(Key::DockerDetails)));
    }
    let detail_key = match state.kind {
        Kind::Containers => Key::DockerImageLabel,
        Kind::Images => Key::DockerId,
        Kind::Networks | Kind::Volumes => Key::DockerDriverLabel,
        Kind::Compose => Key::DockerComposeFiles,
    };
    let mut summary = column![
        text(&item.name)
            .font(theme::semibold())
            .size(14)
            .wrapping(text::Wrapping::WordOrGlyph)
    ]
    .spacing(4);
    for (key, value) in [
        (detail_key, &item.detail),
        (status_key(state.kind), &item.status),
    ] {
        if !value.is_empty() {
            let detail: Element<'a, Event> = if key == Key::DockerImageLabel {
                container(
                    button(
                        text(value)
                            .size(12)
                            .style(theme::BLUE.text())
                            .wrapping(text::Wrapping::WordOrGlyph),
                    )
                    .padding(0)
                    .on_press_maybe(state.busy.is_none().then_some(Event::OpenImage))
                    .style(theme::action(false)),
                )
                .id("docker-open-image")
                .into()
            } else {
                text(value)
                    .size(12)
                    .wrapping(text::Wrapping::WordOrGlyph)
                    .into()
            };
            summary = summary.push(
                row![
                    text(i18n.text(key))
                        .size(12)
                        .style(theme::MUTED.text())
                        .width(Length::FillPortion(1)),
                    container(detail).width(Length::FillPortion(2)),
                ]
                .spacing(GUTTER),
            );
        }
    }
    let mut buttons = row![].spacing(4);
    if super::registry::VISIBLE && state.kind == Kind::Images {
        buttons = buttons.push(crate::ui::service_controls::action(
            i18n,
            Key::RegistryPublish,
            state.busy.is_none().then_some(Event::PublishImage),
        ));
    }
    if state.kind == Kind::Compose {
        buttons = buttons.push(
            theme::compact_button(i18n.text(Key::DockerGraph)).on_press_maybe(
                state
                    .busy
                    .is_none()
                    .then(|| Event::ProjectGraph(item.id.clone())),
            ),
        );
    }
    for action in Action::actions(state.kind, item, terminal) {
        buttons = buttons.push(
            theme::compact_button(i18n.text(action.key()))
                .on_press_maybe(state.busy.is_none().then_some(Event::Act(action)))
                .style(if action == Action::Remove {
                    theme::danger
                } else {
                    theme::secondary
                }),
        );
    }
    let details = column![
        summary.padding(GUTTER),
        context_menu::separator(),
        column![caption(i18n.text(Key::DockerActions)), buttons.wrap()]
            .spacing(5)
            .padding(GUTTER)
    ];
    section(
        "docker-properties",
        header.into(),
        scrollable(details)
            .height(Length::Fill)
            .style(theme::scroll)
            .into(),
        Length::Fill,
    )
}

pub(crate) fn panel<'a>(state: &'a State, i18n: &'a I18n, terminal: bool) -> Element<'a, Event> {
    responsive(move |size| {
        let compact = size.width < COMPACT_WIDTH;
        let selected = state.selected();
        let mut body = row![navigation(state, i18n), vertical_divider()].height(Length::Fill);
        let body = if state.registry_open {
            body.push(
                crate::ui::focus::form(super::registry::panel(
                    &state.registry,
                    &state.profiles,
                    state
                        .context
                        .as_deref()
                        .filter(|_| state.connection_error.is_none()),
                    i18n,
                ))
                .map(Event::Registry),
            )
            .into()
        } else {
            let available = (size.width - tool_rail::WIDTH - 1.).max(0.);
            let split = selected.is_some() && !compact;
            let limits = 188. ..=(available - 280.).max(188.);
            let ratio = if state.kind == Kind::Containers {
                0.30
            } else {
                0.50
            };
            let properties_width = state
                .properties_width
                .unwrap_or((available - 1.) * ratio)
                .clamp(*limits.start(), *limits.end());
            if selected.is_none() || !compact {
                let width = if split {
                    available - properties_width - 1.
                } else {
                    available
                };
                body = body.push(inventory(state, i18n, width));
            }
            if let Some(item) = selected {
                let properties = properties(state, item, i18n, terminal, compact);
                if split {
                    body = body
                        .push(vertical_divider())
                        .push(container(properties).width(properties_width));
                } else {
                    body = body.push(properties);
                }
            }
            crate::ui::resize::right(
                body.into(),
                split.then_some(properties_width),
                size.width - properties_width - 1.,
                limits,
                |width, _| Event::ResizeProperties(width),
            )
        };
        let mut panel = column![
            container(heading(state, i18n, size.width))
                .width(Length::Fill)
                .height(HEADER)
                .padding([0., GUTTER])
                .align_y(Alignment::Center),
            context_menu::separator(),
            body,
        ];
        let status = if let Some(error) = state.error {
            Some((Icon::Warning, theme::RED, i18n.text(error_key(error))))
        } else if let Some(key) = state.busy.filter(|&key| {
            key != Key::Loading
                || (!state.loading_inventory() && !state.show_running && state.context.is_some())
        }) {
            let mut args = crate::i18n::FluentArgs::new();
            args.set("operation", i18n.text(key));
            args.set(
                "seconds",
                state.started.map_or(0, |start| start.elapsed().as_secs()) as i64,
            );
            Some((
                Icon::Refresh,
                theme::BLUE,
                i18n.format(Key::DockerProgress, &args),
            ))
        } else {
            state
                .completed
                .map(|action| (Icon::Check, theme::GREEN, i18n.text(action.key())))
        };
        if let Some((symbol, color, label)) = status {
            panel = panel.push(context_menu::separator()).push(
                row![sized_icon(symbol, color, 14.), text(label).size(12)]
                    .spacing(6)
                    .align_y(Alignment::Center)
                    .padding([3., GUTTER]),
            );
        }
        container(panel)
            .width(Length::Fill)
            .height(Length::Fill)
            .style(theme::surface)
            .into()
    })
    .into()
}

pub(crate) fn overlay<'a, M: Clone + 'a>(
    state: &'a State,
    i18n: &'a I18n,
    terminal: bool,
    base: Element<'a, M>,
    map: fn(Event) -> M,
) -> Element<'a, M> {
    if !state.open {
        return base;
    }
    if state.registry_open
        && let Some(editor) = state.registry.editor()
    {
        return super::registry::edit_overlay(editor, i18n, base, move |event| {
            map(Event::Registry(event))
        });
    }
    if let Some(dialog) = &state.output {
        return modal::bounded_overlay(
            base,
            move |height| {
                container(output::contents(dialog, i18n).map(map))
                    .id("docker-output-dialog")
                    .height(height.min(500.))
                    .into()
            },
            850.,
            map(Event::Dismiss),
        );
    }
    if let Some(project) = &state.project_graph {
        return modal::bounded_overlay(
            base,
            move |height| {
                container(super::graph::panel(project, state.busy.is_some(), i18n).map(map))
                    .id("docker-project-graph")
                    .height(height.min(500.))
                    .into()
            },
            850.,
            map(Event::Dismiss),
        );
    }
    if let Some(draft) = &state.draft {
        return modal::bounded_overlay(
            base,
            move |height| form::contents(state, draft, i18n, map, height),
            516.,
            map(Event::Dismiss),
        );
    }
    if let Some((id, point)) = &state.menu
        && let Some(item) = state.find(Some(id))
    {
        let actions = Action::actions(state.kind, item, terminal);
        return context_menu::overlay_with(
            base,
            *point,
            move |available| {
                let mut rows = column![];
                if state.kind == Kind::Compose {
                    let key = Key::DockerGraph;
                    rows = rows
                        .push(
                            container(
                                context_menu::row(context_menu::label(key, i18n.text(key)))
                                    .style(context_menu::action_style(key, false))
                                    .on_press(map(Event::ProjectGraph(id.clone()))),
                            )
                            .id("docker-menu-project-graph"),
                        )
                        .push(context_menu::separator());
                }
                let rows = rows.extend(actions.iter().map(|action| {
                    let row = context_menu::row(context_menu::label(
                        action.key(),
                        i18n.text(action.key()),
                    ))
                    .style(context_menu::action_style(action.key(), false))
                    .on_press(map(Event::Act(*action)));
                    if *action == Action::Remove {
                        container(row).id("docker-menu-remove").into()
                    } else {
                        row.into()
                    }
                }));
                context_menu::surface(
                    scrollable(rows)
                        .height(Length::Shrink)
                        .style(theme::scroll)
                        .into(),
                    (210. + context_menu::ICON_COLUMN_WIDTH).min(available),
                )
            },
            map(Event::Dismiss),
        );
    }
    base
}
