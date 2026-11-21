//! Render editable source buffers and unsaved-change decisions.

mod completion;
mod definition;
mod diagnostics;
mod gutter;
mod menu;
mod scrollbar;
mod search;
mod syntax;

const TEXT_PADDING: f32 = 12.;

use crate::ui::modal;
use crate::{
    app::{
        App, Message,
        editor::{Buffer, Event},
    },
    domain::shortcuts::Command,
    i18n::Key,
    ui::theme,
};
use iced::{
    Element, Length,
    widget::{column, container, row, space, text, text_editor},
};

pub(crate) fn view<'a>(app: &'a App, buffer: &'a Buffer) -> Element<'a, Message> {
    let id = app.preview().id;
    let line_height = f32::from(app.workspace.preferences.code_line_height) / 100.;
    let font = theme::selected_code_font(app.workspace.preferences.code_font);
    let bindings = &app.workspace.preferences.shortcuts;
    let blocked = app.modules.docker.has_dialog()
        || app.editor.menu.as_ref().is_some_and(|menu| menu.tab == id)
        || app
            .editor
            .rename
            .as_ref()
            .is_some_and(|rename| rename.tab == id)
        || app.editor.pending.is_some()
        || app.auth_dialog.is_some()
        || app.line_navigation.is_some()
        || app.space_draft.is_some()
        || app.project_add.is_some()
        || app.modules.git.git_actions.has_dialog()
        || app.modules.deployment.has_dialog();
    let extension = app
        .preview()
        .path
        .as_deref()
        .map(crate::ui::code::syntax_for)
        .unwrap_or_default();
    let editor = text_editor(&buffer.content)
        .id(format!("text-editor-{id}"))
        .on_action(move |action| Message::Editor(Event::Action(id, action)))
        .font(font)
        .size(app.workspace.preferences.code_size as f32)
        .line_height(iced::widget::text::LineHeight::Relative(line_height))
        .height(Length::Fill)
        .padding(TEXT_PADDING)
        .wheel_scroll_scale(theme::CODE_WHEEL_SCROLL_SCALE)
        .wrapping(iced::widget::text::Wrapping::None)
        .highlight_with::<syntax::Highlighter>(extension.to_owned(), |tone, theme| {
            iced::advanced::text::highlighter::Format {
                color: Some(tone.resolve(theme)),
                font: None,
            }
        })
        .key_binding(move |key| {
            use text_editor::{Binding, Status};
            if blocked || !matches!(key.status, Status::Focused { .. }) {
                return None;
            }
            if key.key == iced::keyboard::Key::Named(iced::keyboard::key::Named::ContextMenu)
                || (key.modifiers.shift()
                    && key.key == iced::keyboard::Key::Named(iced::keyboard::key::Named::F10))
            {
                let left = if app.file_panel_visible {
                    app.workspace.preferences.file_panel_width
                } else {
                    0.
                } + if app.workspace.preferences.show_spaces_sidebar {
                    app.workspace.preferences.spaces_sidebar_width
                } else {
                    0.
                };
                return Some(Binding::Custom(crate::app::editor::menu::message(
                    crate::app::editor::menu::Event::Open(
                        id,
                        iced::Point::new(left + 64., 110.),
                        None,
                    ),
                )));
            }
            if crate::ui::shortcuts::zoom_delta(&key.key, key.physical_key, key.modifiers).is_some()
            {
                return None;
            }
            if let Some(suggestions) = app
                .editor
                .suggestions
                .as_ref()
                .filter(|value| value.tab == id && key.modifiers.is_empty())
            {
                use iced::keyboard::{Key, key::Named};
                let event = match key.key {
                    Key::Named(Named::ArrowDown) => Some(Event::SelectCompletion(true)),
                    Key::Named(Named::ArrowUp) => Some(Event::SelectCompletion(false)),
                    Key::Named(Named::Enter | Named::Tab) => {
                        Some(Event::AcceptCompletion(suggestions.selected))
                    }
                    Key::Named(Named::Escape) => Some(Event::DismissCompletion),
                    _ => None,
                };
                if let Some(event) = event {
                    return Some(Binding::Custom(Message::Editor(event)));
                }
            }
            if let iced::keyboard::Key::Named(named) = &key.key {
                use iced::keyboard::key::Named;
                use text_editor::Motion;
                let motion = match named {
                    Named::ArrowLeft => Some(Motion::Left),
                    Named::ArrowRight => Some(Motion::Right),
                    Named::ArrowUp => Some(Motion::Up),
                    Named::ArrowDown => Some(Motion::Down),
                    Named::Home => Some(Motion::Home),
                    Named::End => Some(Motion::End),
                    Named::PageUp => Some(Motion::PageUp),
                    Named::PageDown => Some(Motion::PageDown),
                    _ => None,
                };
                if let Some(motion) = motion {
                    let motion = if key.modifiers.macos_command() {
                        match motion {
                            Motion::Left => Motion::Home,
                            Motion::Right => Motion::End,
                            Motion::Up => Motion::DocumentStart,
                            Motion::Down => Motion::DocumentEnd,
                            _ => motion,
                        }
                    } else if key.modifiers.jump() {
                        motion.widen()
                    } else {
                        motion
                    };
                    return Some(if key.modifiers.shift() {
                        Binding::Select(motion)
                    } else {
                        Binding::Move(motion)
                    });
                }
            }
            if key.modifiers.control()
                && key.key == iced::keyboard::Key::Named(iced::keyboard::key::Named::Space)
            {
                return Some(Binding::Custom(Message::Editor(Event::Complete(id))));
            }
            if key.modifiers.command()
                && !key.modifiers.alt()
                && key.key.to_latin(key.physical_key) == Some('z')
            {
                return Some(Binding::Custom(Message::Editor(Event::Undo(
                    id,
                    key.modifiers.shift(),
                ))));
            }
            if buffer.content.cursor().selection.is_none() {
                use iced::keyboard::{Key, key::Named};
                let backward = key.key == Key::Named(Named::Backspace);
                let forward = key.key == Key::Named(Named::Delete);
                if backward || forward {
                    let motion = if key.modifiers.macos_command() {
                        Some(if backward {
                            text_editor::Motion::Home
                        } else {
                            text_editor::Motion::End
                        })
                    } else if key.modifiers.jump() {
                        Some(if backward {
                            text_editor::Motion::WordLeft
                        } else {
                            text_editor::Motion::WordRight
                        })
                    } else {
                        None
                    };
                    if let Some(motion) = motion {
                        return Some(Binding::Sequence(vec![
                            Binding::Select(motion),
                            if backward {
                                Binding::Backspace
                            } else {
                                Binding::Delete
                            },
                        ]));
                    }
                }
            }
            if key.key == iced::keyboard::Key::Named(iced::keyboard::key::Named::Tab)
                && !key.modifiers.macos_command()
            {
                return Some(Binding::Sequence(vec![Binding::Insert(' '); 4]));
            }
            for command in Command::ALL {
                if !bindings.conflict(command)
                    && crate::ui::shortcuts::matches(
                        &bindings.binding(command),
                        &key.key,
                        key.physical_key,
                        key.modifiers,
                    )
                {
                    return match command {
                        Command::Copy => Some(Binding::Copy),
                        Command::SelectAll => Some(Binding::SelectAll),
                        _ => crate::app::shortcuts::command_message(command).map(Binding::Custom),
                    };
                }
            }
            Binding::from_key_press(key)
        })
        .style(theme::code_editor);
    let editor: Element<'_, Message> = if let Some(search) = app.file_search() {
        iced::widget::stack![
            editor,
            iced::widget::canvas(self::search::Highlights {
                content: &buffer.content,
                search
            })
            .width(Length::Fill)
            .height(Length::Fill)
        ]
        .into()
    } else {
        iced::widget::stack![editor].into()
    };
    let editor: Element<'_, Message> = if let Some(report) =
        app.editor.diagnostics.as_ref().filter(|report| {
            !report.diagnostics.is_empty()
                && report.tab == id
                && report.text == buffer.document.text
        }) {
        iced::widget::stack![
            editor,
            iced::widget::canvas(diagnostics::Underlines {
                content: &buffer.content,
                diagnostics: &report.diagnostics,
            })
            .width(Length::Fill)
            .height(Length::Fill)
        ]
        .into()
    } else {
        iced::widget::stack![editor].into()
    };
    let editor = crate::ui::context_menu::positioned(editor, move |point, local| {
        let hit = buffer
            .content
            .with_editor(|editor| {
                editor
                    .buffer()
                    .hit(local.x - TEXT_PADDING, local.y - TEXT_PADDING)
            })
            .map(|cursor| text_editor::Position {
                line: cursor.line,
                column: cursor.index,
            });
        crate::app::editor::menu::message(crate::app::editor::menu::Event::Open(id, point, hit))
    });
    let editor = completion::overlay(app, editor);
    let editor = definition::overlay(app, editor);
    let cursor = buffer.content.cursor().position;
    let size = app.workspace.preferences.code_size as f32;
    let mut body = row![];
    let marks = if app.module_enabled(crate::modules::ModuleId::Git)
        && buffer.marks.after == buffer.document.text
    {
        buffer.marks.hunks.as_slice()
    } else {
        &[]
    };
    let numbers = app.workspace.preferences.line_numbers;
    if numbers || !marks.is_empty() {
        let digits = buffer.content.line_count().max(1).ilog10() + 1;
        body = body.push(
            container(
                iced::widget::canvas(gutter::Gutter {
                    content: &buffer.content,
                    tab: id,
                    marks,
                    numbers,
                    size,
                    line_height,
                    font,
                })
                .width(if numbers {
                    size * 0.65 * digits.max(3) as f32 + gutter::MARKER_LANE + 8.
                } else {
                    gutter::MARKER_LANE
                })
                .height(Length::Fill),
            )
            .id("editor-line-numbers"),
        );
    }
    body = body
        .push(editor)
        .push(
            container(
                iced::widget::canvas(scrollbar::Scrollbar {
                    content: &buffer.content,
                    tab: id,
                    row_height: size * line_height,
                })
                .width(theme::CODE_SCROLLBAR_WIDTH)
                .height(Length::Fill),
            )
            .id("editor-scrollbar"),
        )
        .height(Length::Fill);
    column![
        body,
        container(
            text(format!("{}:{}", cursor.line + 1, cursor.column + 1))
                .size(11)
                .style(theme::MUTED.text())
        )
        .padding([4, 12])
    ]
    .height(Length::Fill)
    .into()
}

pub(crate) fn overlay<'a>(app: &'a App, base: Element<'a, Message>) -> Element<'a, Message> {
    let base = menu::overlay(app, base);
    let base = if let Some(buffer) = app.editor_buffer()
        && app.module_enabled(crate::modules::ModuleId::Git)
        && buffer.document.text == buffer.marks.after
        && let Some((index, point)) = buffer.marks.open
        && let Some(hunk) = buffer.marks.hunks.get(index)
    {
        use crate::ui::context_menu;
        context_menu::overlay(
            base,
            point,
            iced::Size::new(490., 220.),
            || {
                let mut lines = column![];
                let mut skip = 40 * buffer.marks.page;
                let mut left = 40;
                for (source, range, color, prefix) in [
                    (
                        &buffer.marks.before,
                        hunk.old.clone(),
                        theme::GIT_REMOVED,
                        "−",
                    ),
                    (&buffer.marks.after, hunk.new.clone(), theme::GIT_ADDED, "+"),
                ] {
                    let count = range.len();
                    let start = skip.min(count);
                    skip -= start;
                    let take = (count - start).min(left);
                    left -= take;
                    for line in source.lines().skip(range.start + start).take(take) {
                        lines = lines.push(
                            text(format!("{prefix} {line}"))
                                .size(12)
                                .font(iced::Font::MONOSPACE)
                                .style(color.text())
                                .wrapping(iced::widget::text::Wrapping::None),
                        );
                    }
                }
                let lines = iced::widget::scrollable(lines)
                    .style(theme::scroll)
                    .direction(iced::widget::scrollable::Direction::Both {
                        vertical: Default::default(),
                        horizontal: Default::default(),
                    })
                    .height(176);
                let pages = (hunk.old.len() + hunk.new.len()).div_ceil(40);
                let mut popup = column![lines];
                if pages > 1 {
                    let page = buffer.marks.page;
                    let message = |next| {
                        Message::Editor(Event::Marks(crate::app::editor::marks::Event::Page(next)))
                    };
                    popup = popup.push(row![
                        theme::compact_button(app.i18n.text(Key::PreviousFilePart))
                            .on_press_maybe((page > 0).then(|| message(false)))
                            .style(theme::action(false)),
                        space().width(Length::Fill),
                        text(format!("{} / {pages}", page + 1)).size(12),
                        space().width(Length::Fill),
                        theme::compact_button(app.i18n.text(Key::NextFilePart))
                            .on_press_maybe((page + 1 < pages).then(|| message(true)))
                            .style(theme::action(false))
                    ]);
                }
                popup.into()
            },
            Message::Editor(Event::Marks(crate::app::editor::marks::Event::Close)),
        )
    } else {
        base
    };
    if app.editor.pending.is_none()
        || app.editor.saving_all
        || app
            .editor
            .pending
            .as_deref()
            .and_then(crate::app::files::operations::request)
            .is_some()
    {
        return base;
    }
    let busy = app
        .tabs
        .files
        .iter()
        .any(|tab| tab.editor.as_ref().is_some_and(|buffer| buffer.saving));
    let mut files = modal::content(app.i18n.text(Key::EditorUnsavedTitle));
    for tab in app.tabs.files.iter().filter(|tab| {
        app.editor_pending_tab(tab.id) && tab.editor.as_ref().is_some_and(Buffer::protected)
    }) {
        files = files.push(text(&tab.title).size(13));
        if let Some(error) = tab.editor.as_ref().and_then(|buffer| buffer.error) {
            files = files.push(text(app.i18n.text(error)).size(12).style(theme::RED.text()));
        }
    }
    if busy {
        files = files.push(text(app.i18n.text(Key::EditorSaving)).size(12));
    }
    let action = |key, event, enabled: bool| {
        theme::text_button(app.i18n.text(key))
            .on_press_maybe(enabled.then_some(Message::Editor(event)))
            .style(match key {
                Key::EditorDiscard => theme::danger,
                Key::EditorSaveAll => theme::primary,
                _ => theme::secondary,
            })
    };
    files = files.push(
        modal::actions()
            .push(action(Key::Cancel, Event::Cancel, true))
            .push(action(Key::EditorDiscard, Event::Discard, !busy))
            .push(action(Key::EditorSaveAll, Event::SaveAll, !busy)),
    );
    crate::ui::modal::overlay(base, files.into(), 560., Message::Editor(Event::Cancel))
}
