//! Render editable source buffers and unsaved-change decisions.

mod completion;
mod definition;
mod diagnostics;
mod gutter;
mod scrollbar;
mod search;
mod syntax;

const TEXT_PADDING: f32 = 12.;

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
    widget::{button, column, container, row, space, text, text_editor},
};

pub(crate) fn view<'a>(app: &'a App, buffer: &'a Buffer) -> Element<'a, Message> {
    let id = app.preview().id;
    let line_height = f32::from(app.workspace.preferences.code_line_height) / 100.;
    let font = theme::selected_code_font(app.workspace.preferences.code_font);
    let bindings = &app.workspace.preferences.shortcuts;
    let blocked = app.editor.pending.is_some()
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
            if key.key == iced::keyboard::Key::Named(iced::keyboard::key::Named::Tab) {
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
        .style(|style_theme, _| text_editor::Style {
            background: theme::code_surface(style_theme)
                .background
                .unwrap_or(iced::Color::TRANSPARENT.into()),
            border: iced::Border::default(),
            placeholder: theme::MUTED.resolve(style_theme),
            value: theme::syntax::TEXT.resolve(style_theme),
            selection: theme::CODE_SELECTION.resolve(style_theme),
        });
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
    let editor = if let Some(report) = app
        .editor
        .diagnostics
        .as_ref()
        .filter(|report| report.tab == id && report.text == buffer.document.text)
    {
        iced::widget::stack![
            editor,
            iced::widget::canvas(diagnostics::Underlines {
                content: &buffer.content,
                diagnostics: &report.diagnostics,
                text: &report.text
            })
            .width(Length::Fill)
            .height(Length::Fill)
        ]
        .into()
    } else {
        iced::widget::stack![editor].into()
    };
    let editor = completion::overlay(app, editor);
    let editor = definition::overlay(app, editor);
    let cursor = buffer.content.cursor().position;
    let size = app.workspace.preferences.code_size as f32;
    let mut body = row![];
    if app.workspace.preferences.line_numbers {
        let digits = buffer.content.line_count().max(1).ilog10() + 1;
        body = body.push(
            container(
                iced::widget::canvas(gutter::Gutter {
                    content: &buffer.content,
                    size,
                    line_height,
                    font,
                })
                .width(size * 0.65 * digits.max(3) as f32 + 16.)
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
    let mut files = column![text(app.i18n.text(Key::EditorUnsavedTitle)).size(20)].spacing(12);
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
        button(text(app.i18n.text(key)).size(13))
            .on_press_maybe(enabled.then_some(Message::Editor(event)))
            .style(theme::action(false))
    };
    files = files.push(
        row![
            action(Key::Cancel, Event::Cancel, true),
            space().width(Length::Fill),
            action(Key::EditorDiscard, Event::Discard, !busy),
            action(Key::EditorSaveAll, Event::SaveAll, !busy)
        ]
        .spacing(8),
    );
    crate::ui::modal::overlay(base, files.into(), 560., Message::Editor(Event::Cancel))
}
