//! Present available Docker operations, their results and confirmations.

mod form;
mod style;

use super::{Action, Error, Event, Kind, PAGE, State};
use crate::{
    i18n::{I18n, Key},
    ui::{
        context_menu, ellipsis,
        icons::{Icon, sized_icon},
        modal, theme, tooltip,
    },
};
use iced::{
    Alignment, Element, Length,
    widget::{
        button, column, container, pick_list, responsive, row, scrollable, space, text, text_input,
    },
};

fn error_key(error: Error) -> Key {
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
const HEADER: f32 = 34.;
const NAVIGATION_WIDTH: f32 = 42.;
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

fn tool<'a>(i18n: &I18n, symbol: Icon, label: Key, message: Option<Event>) -> Element<'a, Event> {
    tooltip::view(
        button(sized_icon(symbol, theme::MUTED, 16.))
            .on_press_maybe(message)
            .padding(4)
            .style(style::button(theme::icon_action(false))),
        text(i18n.text(label)).size(12),
        tooltip::Position::Bottom,
    )
    .style(style::surface(theme::tooltip))
    .into()
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
        container(space())
            .width(1)
            .height(18)
            .style(theme::separator),
    ]
    .spacing(GUTTER)
    .align_y(Alignment::Center);
    if width >= 520. {
        heading = heading.push(caption(i18n.text(Key::DockerConnection)));
    }
    heading
        .push(
            pick_list(
                state.contexts.as_slice(),
                state.context.as_ref(),
                Event::Context,
            )
            .placeholder(i18n.text(Key::DockerConnection))
            .width((width - 185.).clamp(55., 188.))
            .text_size(12)
            .padding([3, 8])
            .style(style::select)
            .menu_style(style::menu),
        )
        .push(tool(
            i18n,
            Icon::Plus,
            Key::DockerAddContext,
            ready.then_some(Event::Act(Action::AddContext)),
        ))
        .push(tool(
            i18n,
            Icon::Refresh,
            Key::DockerRefresh,
            ready.then_some(Event::Connections),
        ))
        .push(space().width(Length::Fill))
        .push(tool(i18n, Icon::Close, Key::DockerClose, Some(Event::Hide)))
        .into()
}

fn navigation<'a>(state: &State, i18n: &I18n) -> Element<'a, Event> {
    let mut links = column![].spacing(2).padding(3);
    for kind in Kind::ALL {
        let selected = kind == state.kind;
        let symbol = sized_icon(
            glyph(kind),
            if selected { theme::BLUE } else { theme::MUTED },
            16.,
        );
        let link = button(container(symbol).center_x(Length::Fill))
            .width(Length::Fill)
            .padding([7, 0])
            .on_press_maybe(state.busy.is_none().then_some(Event::Kind(kind)))
            .style(style::button(theme::tree_item(selected)));
        links = links.push(
            tooltip::view(
                link,
                text(i18n.text(kind.key())).size(12),
                tooltip::Position::Right,
            )
            .style(style::surface(theme::tooltip)),
        );
    }
    container(scrollable(links).height(Length::Fill).style(style::scroll))
        .id("docker-navigation")
        .width(NAVIGATION_WIDTH)
        .height(Length::Fill)
        .style(theme::surface)
        .into()
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
    let mut header = row![
        text(i18n.text(state.kind.key()))
            .font(theme::semibold())
            .size(14),
        container(text(state.filtered.len().to_string()).size(11))
            .padding([1, 6])
            .style(style::surface(theme::badge)),
        space().width(Length::Fill),
    ]
    .align_y(Alignment::Center)
    .spacing(5);
    let command = |action: Action, symbol| -> Element<'a, Event> {
        let available = ready && state.context.is_some() && state.connection_error.is_none();
        let event = available.then_some(Event::Act(action));
        if width < 440. {
            tool(i18n, symbol, action.key(), event)
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
            .style(style::button(theme::secondary))
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
    header = header.push(tool(
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
    let search = text_input(&i18n.text(Key::DockerFilter), &state.query)
        .id("docker-filter")
        .on_input(Event::Search)
        .size(12)
        .padding([5, 8])
        .style(style::input);
    let mut content = column![container(search).padding([6., GUTTER]).width(Length::Fill)];
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
    for &index in state.filtered.iter().skip(state.page * PAGE).take(PAGE) {
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
            .style(style::button(theme::tree_item(
                state.selected.as_ref() == Some(id),
            )));
        rows = rows.push(context_menu::trigger(row, move |point| {
            Event::Menu(id.clone(), point)
        }));
    }
    if state.filtered.is_empty() && ready && state.error.is_none() {
        let message = if state.query.trim().is_empty() {
            Key::DockerEmptyList
        } else {
            Key::DockerEmpty
        };
        content = content.push(
            container(text(i18n.text(message)).size(13).style(theme::MUTED.text()))
                .center(Length::Fill),
        );
    } else {
        content = content.push(scrollable(rows).height(Length::Fill).style(style::scroll));
    }
    if state.filtered.len() > PAGE {
        content = content.push(context_menu::separator()).push(pagination(
            state.page,
            state.filtered.len().div_ceil(PAGE),
            i18n,
            Event::Page,
        ));
    }
    section(
        "docker-inventory",
        header.into(),
        content.into(),
        Length::Fill,
    )
}

fn pagination<'a>(
    page: usize,
    pages: usize,
    i18n: &I18n,
    event: fn(bool) -> Event,
) -> Element<'a, Event> {
    row![
        space().width(Length::Fill),
        tool(
            i18n,
            Icon::Back,
            Key::PreviousFilePart,
            (page > 0).then(|| event(false))
        ),
        text(format!("{} / {}", page + 1, pages)).size(12),
        tool(
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
            container(tool(
                i18n,
                Icon::Back,
                Key::DockerBack,
                state.busy.is_none().then_some(Event::BackToList),
            ))
            .id("docker-back"),
        );
    }
    header = header.push(caption(i18n.text(Key::DockerDetails)));
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
            summary = summary.push(
                row![
                    text(i18n.text(key))
                        .size(12)
                        .style(theme::MUTED.text())
                        .width(Length::FillPortion(1)),
                    text(value)
                        .size(12)
                        .wrapping(text::Wrapping::WordOrGlyph)
                        .width(Length::FillPortion(2)),
                ]
                .spacing(GUTTER),
            );
        }
    }
    let mut buttons = row![].spacing(4);
    for action in Action::actions(state.kind, item, terminal) {
        let label = text(i18n.text(action.key())).size(12);
        let label = if action == Action::Remove {
            label.style(theme::RED.text())
        } else {
            label
        };
        buttons = buttons.push(
            button(label)
                .on_press_maybe(state.busy.is_none().then_some(Event::Act(action)))
                .padding([3, 6])
                .style(style::button(theme::secondary)),
        );
    }
    let mut details = column![
        summary.padding(GUTTER),
        context_menu::separator(),
        column![caption(i18n.text(Key::DockerActions)), buttons.wrap()]
            .spacing(5)
            .padding(GUTTER)
    ];
    if let Some(output) = &state.detail {
        let mut toolbar = row![caption(i18n.text(Key::DockerOutput))].align_y(Alignment::Center);
        if output.offsets.len() > 1 {
            toolbar = toolbar.push(pagination(
                state.output_page,
                output.offsets.len(),
                i18n,
                Event::OutputPage,
            ));
        }
        let value = output.page(state.output_page).unwrap_or_default();
        let output = if value.is_empty() {
            text(i18n.text(Key::DockerEmpty))
        } else {
            text(value)
        };
        details = details.push(context_menu::separator()).push(
            column![
                toolbar,
                output
                    .font(iced::Font::MONOSPACE)
                    .size(12)
                    .wrapping(text::Wrapping::WordOrGlyph)
            ]
            .padding(GUTTER)
            .spacing(5),
        );
    }
    section(
        "docker-properties",
        header.into(),
        scrollable(details)
            .height(Length::Fill)
            .style(style::scroll)
            .into(),
        Length::Fill,
    )
}

pub(crate) fn panel<'a>(state: &'a State, i18n: &'a I18n, terminal: bool) -> Element<'a, Event> {
    responsive(move |size| {
        let compact = size.width < COMPACT_WIDTH;
        let selected = state.selected();
        let divider = || {
            container(space())
                .width(1)
                .height(Length::Fill)
                .style(theme::separator)
        };
        let mut body = row![navigation(state, i18n), divider()].height(Length::Fill);
        if selected.is_none() || !compact {
            let width = size.width - NAVIGATION_WIDTH - 1.;
            body = body.push(inventory(
                state,
                i18n,
                if selected.is_some() {
                    (width - 1.) / 2.
                } else {
                    width
                },
            ));
        }
        if let Some(item) = selected {
            if !compact {
                body = body.push(divider());
            }
            body = body.push(properties(state, item, i18n, terminal, compact));
        }
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
        } else if let Some(key) = state.busy {
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
    if let Some(draft) = &state.draft {
        return modal::bounded_overlay(
            base,
            move |height| form::contents(state, draft, i18n, map, height),
            516.,
            map(Event::Dismiss),
            style::surface(theme::dialog),
        );
    }
    if let (Some(point), Some(item)) = (state.menu, state.selected()) {
        let actions = Action::actions(state.kind, item, terminal);
        return context_menu::overlay_with(
            base,
            point,
            move |available| {
                let rows = iced::widget::Column::with_children(actions.iter().map(|action| {
                    context_menu::row(context_menu::label(action.key(), i18n.text(action.key())))
                        .style(style::button(context_menu::action_style(
                            action.key(),
                            false,
                        )))
                        .on_press(map(Event::Act(*action)))
                        .into()
                }));
                iced::widget::opaque(
                    container(rows)
                        .padding(context_menu::SURFACE_PADDING)
                        .width((210. + context_menu::ICON_COLUMN_WIDTH).min(available))
                        .style(style::surface(theme::tooltip)),
                )
            },
            map(Event::Dismiss),
        );
    }
    base
}
