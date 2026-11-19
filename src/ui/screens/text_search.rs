//! Show source search controls and project results.
mod spinner;
use crate::{
    app::{
        App, Message,
        text_search::{Event, Search},
    },
    i18n::Key,
    ui::{context_menu, icons, theme},
};
use iced::{
    Element, Length,
    widget::{button, column, container, opaque, row, scrollable, stack, text, text_input},
};
fn message(event: Event) -> Message {
    Message::TextSearch(event)
}
fn action(app: &App, key: Key, event: Event) -> Element<'_, Message> {
    button(text(app.i18n.text(key)).size(12))
        .on_press(message(event))
        .style(theme::action(false))
        .padding([5, 8])
        .into()
}
fn compact<'a>(
    app: &App,
    label: &'static str,
    key: Key,
    event: Event,
    active: bool,
) -> Element<'a, Message> {
    let content: Element<'a, Message> = match label {
        "×" => icons::sized_icon(icons::Icon::Close, theme::MUTED, 16.),
        "›" => icons::sized_icon(icons::Icon::Right, theme::MUTED, 16.),
        "⌄" | "↓" => icons::sized_icon(icons::Icon::Down, theme::MUTED, 16.),
        "↑" => icons::sized_icon(icons::Icon::Up, theme::MUTED, 16.),
        _ => text(label).size(14).into(),
    };
    iced::widget::tooltip(
        button(content)
            .on_press(message(event))
            .style(theme::action(active))
            .padding([4, 7]),
        container(text(app.i18n.text(key)).size(12))
            .padding(6)
            .style(theme::search_panel),
        iced::widget::tooltip::Position::Bottom,
    )
    .into()
}
fn controls<'a>(app: &'a App, search: &'a Search, project: bool) -> Element<'a, Message> {
    let mut bar = row![].spacing(4).align_y(iced::Alignment::Center);
    if !project && app.editor_buffer().is_some() {
        bar = bar.push(compact(
            app,
            if search.replace { "⌄" } else { "›" },
            Key::FindReplace,
            Event::ToggleReplace,
            search.replace,
        ));
    }
    bar = bar
        .push(icons::icon(icons::Icon::Search, theme::MUTED))
        .push(
            text_input(
                &if project {
                    String::new()
                } else {
                    app.i18n.text(Key::FindFile)
                },
                &search.query,
            )
            .id(if project {
                "project-search"
            } else {
                "file-search"
            })
            .on_input(move |value| message(Event::Query(project, value)))
            .size(14)
            .padding([7, 8])
            .style(theme::search_input),
        );
    for (label, key, option, active) in [
        ("Cc", Key::FindCase, 0, search.options.case),
        ("W", Key::FindWord, 1, search.options.word),
        (".*", Key::FindRegex, 2, search.options.regex),
    ] {
        bar = bar.push(compact(
            app,
            label,
            key,
            Event::Option(project, option),
            active,
        ));
    }
    if !project {
        bar = bar
            .push(
                text(if search.busy {
                    String::new()
                } else {
                    format!(
                        "{} / {}",
                        if search.results.hits.is_empty() {
                            0
                        } else {
                            search.selected + 1
                        },
                        search.results.hits.len()
                    )
                })
                .size(12)
                .style(theme::MUTED.text()),
            )
            .push(compact(
                app,
                "↑",
                Key::FindPrevious,
                Event::Move(false, false),
                false,
            ))
            .push(compact(
                app,
                "↓",
                Key::FindNext,
                Event::Move(false, true),
                false,
            ))
            .push(compact(
                app,
                "×",
                Key::FindClose,
                Event::Close(false),
                false,
            ));
    }
    if search.busy {
        bar = bar.push(spinner::view());
    }
    let mut content = column![bar].spacing(4);
    if let Some(error) = search.error {
        content = content.push(text(app.i18n.text(error)).size(12));
    }
    if search.results.limited {
        content = content.push(text(app.i18n.text(Key::FindLimited)).size(12));
    }
    if !project && app.editor_buffer().is_some() && search.replace {
        content = content.push(
            row![
                text_input(&app.i18n.text(Key::FindReplace), &search.replacement)
                    .on_input(|value| message(Event::Replacement(value)))
                    .style(theme::search_input)
                    .size(14)
                    .padding([7, 8]),
                action(app, Key::FindReplace, Event::Replace(false)),
                action(app, Key::FindReplaceAll, Event::Replace(true))
            ]
            .spacing(6),
        );
    }
    container(content)
        .padding([4, 8])
        .width(Length::Fill)
        .style(theme::search_panel)
        .into()
}
pub(crate) fn file_panel<'a>(app: &'a App, base: Element<'a, Message>) -> Element<'a, Message> {
    if app.file_search().is_none() && app.text_search.saved.is_none() && !app.text_search.pinned {
        return base;
    }
    let mut content = column![];
    if let Some(saved) = app
        .text_search
        .saved
        .as_ref()
        .filter(|search| app.workspace.directory() == Some(&search.root))
    {
        content = content.push(
            button(
                row![
                    icons::icon(icons::Icon::Search, theme::MUTED),
                    text(saved.query.clone()).size(12)
                ]
                .spacing(8),
            )
            .on_press(message(Event::Restore))
            .style(theme::action(false)),
        );
    }
    if app.text_search.pinned
        && let Some(search) = &app.text_search.project
    {
        return content
            .push(project_view(app, search))
            .height(Length::Fill)
            .into();
    }
    if let Some(search) = app.file_search() {
        content = content.push(context_menu::on_key(
            controls(app, search, false),
            move |event| key(event, false, app),
        ));
    }
    let base: Element<'a, Message> = content.push(base).height(Length::Fill).into();
    if app.file_search().is_none() {
        return base;
    }
    base
}
fn key(event: &iced::keyboard::Event, project: bool, app: &App) -> Option<Message> {
    use iced::keyboard::{Event as K, Key, key::Named};
    match event {
        K::KeyPressed {
            key,
            physical_key,
            modifiers,
            ..
        } if crate::ui::shortcuts::matches(
            &app.workspace
                .preferences
                .shortcuts
                .binding(crate::domain::shortcuts::Command::CloseTab),
            key,
            *physical_key,
            *modifiers,
        ) =>
        {
            Some(Message::CloseActiveTab)
        }
        K::KeyPressed {
            key: Key::Named(Named::Escape),
            ..
        } => Some(message(Event::Close(project))),
        K::KeyPressed {
            key: Key::Named(Named::Enter),
            modifiers,
            ..
        } => Some(message(if project {
            Event::Activate
        } else {
            Event::Move(false, !modifiers.shift())
        })),
        K::KeyPressed {
            key: Key::Named(Named::ArrowDown),
            ..
        } if project => Some(message(Event::Move(true, true))),
        K::KeyPressed {
            key: Key::Named(Named::ArrowUp),
            ..
        } if project => Some(message(Event::Move(true, false))),
        _ => None,
    }
}
pub(crate) fn overlay<'a>(app: &'a App, base: Element<'a, Message>) -> Element<'a, Message> {
    if app.text_search.pinned {
        return base;
    }
    let Some(search) = &app.text_search.project else {
        return base;
    };
    let popup = project_view(app, search);
    let layer = container(popup)
        .center(Length::Fill)
        .style(|_| container::Style {
            background: Some(iced::Color::from_rgba(0., 0., 0., 0.5).into()),
            ..Default::default()
        });
    context_menu::on_key(opaque(stack![base, opaque(layer)]), move |event| {
        key(event, true, app)
    })
}
fn project_view<'a>(app: &'a App, search: &'a Search) -> Element<'a, Message> {
    let pattern = (!search.query.is_empty())
        .then(|| crate::domain::text_search::compile(&search.query, &search.options).ok())
        .flatten();
    let mut results = column![];
    let start = search.selected.saturating_sub(8);
    for (index, hit) in search.results.hits.iter().enumerate().skip(start).take(20) {
        let path = hit
            .path
            .strip_prefix(&search.root)
            .unwrap_or(&hit.path)
            .display();
        results = results.push(
            button(
                row![
                    container(highlighted(&hit.found.text, pattern.as_ref(), app))
                        .width(Length::Fill),
                    text(format!("{}  {}", path, hit.found.line))
                        .size(12)
                        .style(theme::MUTED.text())
                ]
                .spacing(16),
            )
            .width(Length::Fill)
            .padding([2, 8])
            .on_press(message(Event::Select(index)))
            .style(theme::search_result(index == search.selected)),
        );
    }
    let preview = search
        .results
        .hits
        .get(search.selected)
        .map(|hit| {
            format!(
                "{}:{}\n\n{}",
                hit.path.display(),
                hit.found.line,
                search.preview
            )
        })
        .unwrap_or_else(|| app.i18n.text(Key::FindNoResults));
    let body: Element<'a, Message> =
        if search.query.is_empty() || search.results.hits.is_empty() || search.busy {
            container(
                text(if search.query.is_empty() {
                    app.i18n.text(Key::FindEmpty)
                } else if search.busy || search.error.is_some() {
                    String::new()
                } else {
                    app.i18n.text(Key::FindNoResults)
                })
                .size(14)
                .style(theme::MUTED.text()),
            )
            .center_x(Length::Fill)
            .center_y(540)
            .into()
        } else {
            column![
                container(scrollable(results).height(234).style(theme::scroll))
                    .padding(8)
                    .width(Length::Fill)
                    .style(theme::search_panel),
                container(
                    scrollable(highlighted(&preview, pattern.as_ref(), app))
                        .height(256)
                        .style(theme::scroll)
                )
                .padding(12)
                .width(Length::Fill)
                .style(theme::search_preview)
            ]
            .spacing(10)
            .into()
        };
    let popup = container(
        column![
            row![
                text(app.i18n.text(Key::FindFiles))
                    .size(15)
                    .font(theme::semibold()),
                text(if search.query.is_empty() || search.busy {
                    String::new()
                } else {
                    search.results.hits.len().to_string()
                })
                .size(13)
                .style(theme::MUTED.text()),
                iced::widget::space().width(Length::Fill),
                text_input(&app.i18n.text(Key::FindMask), &search.mask)
                    .on_input(|value| message(Event::Mask(value)))
                    .style(theme::search_input)
                    .size(13)
                    .width(250),
                compact(app, "×", Key::FindClose, Event::Close(true), false)
            ]
            .spacing(12)
            .align_y(iced::Alignment::Center),
            controls(app, search, true),
            row![
                button(text(app.i18n.text(Key::FindProjectScope)).size(13))
                    .on_press(message(Event::Folder(String::new())))
                    .style(theme::action(search.folder.is_empty()))
                    .padding([5, 10]),
                text_input(&app.i18n.text(Key::FindFolder), &search.folder)
                    .on_input(|value| message(Event::Folder(value)))
                    .style(theme::search_input)
                    .size(13)
                    .padding([5, 8])
            ]
            .spacing(8)
            .align_y(iced::Alignment::Center),
            body,
            row![
                action(app, Key::FindPin, Event::Pin),
                action(app, Key::FindRefresh, Event::Refresh)
            ]
            .spacing(8)
        ]
        .spacing(10),
    )
    .padding(12)
    .width(Length::Fill)
    .max_width(1000)
    .style(theme::search_dialog);
    context_menu::on_key(popup, move |event| key(event, true, app))
}

fn highlighted<'a>(value: &str, pattern: Option<&regex::Regex>, app: &App) -> Element<'a, Message> {
    let mut spans = Vec::new();
    let mut start = 0;
    if let Some(pattern) = pattern {
        for found in pattern.find_iter(value).take(1000) {
            spans.push(iced::widget::span(value[start..found.start()].to_owned()));
            spans.push(
                iced::widget::span(value[found.range()].to_owned())
                    .background(theme::CODE_SELECTION.resolve(&app.theme())),
            );
            start = found.end();
        }
    }
    spans.push(iced::widget::span(value[start..].to_owned()));
    iced::widget::rich_text::<(), Message, _, _>(spans)
        .font(theme::code_font())
        .size(13)
        .into()
}
