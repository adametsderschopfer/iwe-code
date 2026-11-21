use super::*;
use crate::{
    i18n::{I18n, Language},
    ui::code::Document,
};
use std::rc::Rc;

fn app() -> App {
    let mut app = App::new(Rc::new(I18n::new(Language::English)));
    app.workspace.create_space(
        "Work".into(),
        vec![PathBuf::from("project"), PathBuf::from("other")],
    );
    app.loaded = true;
    let _ = app.open_file(PathBuf::from("project/file.txt"));
    let id = app.preview().id;
    let _ = app.finish_preview(id, Ok(Document::new("original\r\n".into(), "txt")));
    app
}

fn edit(app: &mut App, value: &str) {
    let id = app.preview().id;
    let _ = app.update_editor(Event::Action(id, text_editor::Action::SelectAll));
    let _ = app.update_editor(Event::Action(
        id,
        text_editor::Action::Edit(text_editor::Edit::Paste(Arc::new(value.into()))),
    ));
}

#[test]
fn markdown_renders_a_typed_link_before_saving_and_returns_to_the_editor() {
    use iced::widget::Id;
    const DRAFT: &str = "[Unsaved link](https://example.org/draft)";

    fn screen(app: &App) -> iced_test::Simulator<'_, Message> {
        iced_test::Simulator::with_size(iced::Settings::default(), (1600., 900.), app.view())
    }

    let mut app = app();
    let _ = app.open_file(PathBuf::from("project/notes.md"));
    let id = app.preview().id;
    let _ = app.finish_preview(id, Ok(Document::new(String::new(), "md")));
    let editor = Id::from(format!("text-editor-{id}"));
    let formatted = Id::new("markdown-preview-pane");

    let mut ui = screen(&app);
    let source = ui.find(editor.clone()).unwrap().bounds();
    assert!(ui.find(formatted.clone()).is_err());
    ui.click(Id::new("markdown-split")).unwrap();
    for message in ui.into_messages() {
        let _ = app.update(message);
    }

    let mut ui = screen(&app);
    let left = ui.click(editor.clone()).unwrap().bounds();
    let right = ui
        .find(formatted.clone())
        .unwrap()
        .visible_bounds()
        .unwrap();
    let header = ui.find(Id::new("file-tabs-header")).unwrap().bounds();
    assert!(left.x + left.width <= right.x);
    assert!((2. * right.width - header.width).abs() <= 2.);
    ui.typewrite(DRAFT);
    for message in ui.into_messages() {
        let _ = app.update(message);
    }
    let buffer = app.editor_buffer().unwrap();
    assert_eq!(buffer.document.text.as_str(), DRAFT);
    assert!(buffer.document.saved.is_empty());
    let cursor = buffer.content.cursor();

    for (next, read_only) in [("markdown-preview", false), ("markdown-source", true)] {
        let mut ui = screen(&app);
        let pane = ui
            .find(formatted.clone())
            .unwrap()
            .visible_bounds()
            .unwrap();
        assert_eq!(ui.find(editor.clone()).is_err(), read_only);
        if read_only {
            assert!((pane.width - header.width).abs() <= 1.);
        }
        // Click the first link on the first line of the formatted document.
        ui.point_at((pane.x + 28., pane.y + 25.));
        ui.simulate(iced_test::simulator::click());
        if read_only {
            ui.typewrite("x");
        }
        ui.click(Id::new(next)).unwrap();
        let mut opened = None;
        for message in ui.into_messages() {
            match message {
                Message::OpenUrl(url) => opened = Some(url),
                action => {
                    let _ = app.update(action);
                }
            }
        }
        assert_eq!(opened.as_deref(), Some("https://example.org/draft"));
    }

    let buffer = app.editor_buffer().unwrap();
    assert_eq!(buffer.content.cursor(), cursor);
    assert_eq!(buffer.document.text.as_str(), DRAFT);
    let mut ui = screen(&app);
    assert!(ui.find(formatted).is_err());
    assert_eq!(ui.click(editor).unwrap().bounds(), source);
    ui.typewrite("!");
    for message in ui.into_messages() {
        let _ = app.update(message);
    }
    let buffer = app.editor_buffer().unwrap();
    assert!(buffer.document.text.contains('!'));
    assert!(buffer.document.saved.is_empty());
}

#[tokio::test]
async fn diagnostics_stop_when_the_last_local_language_document_closes() {
    let mut app = app();
    assert_eq!(app.diagnostics_subscription().units(), 0);
    let path = PathBuf::from("project/source.rs");
    let _ = app.open_file(path.clone());
    let tab = app.preview().id;
    let _ = app.finish_preview(tab, Ok(Document::new("fn main() {}".into(), "rs")));
    assert_eq!(app.diagnostics_subscription().units(), 1);
    app.editor
        .languages
        .lock()
        .await
        .maintain(
            std::path::Path::new("project"),
            vec![(path, Arc::new("fn main() {}".into()))],
        )
        .await;
    let _ = app.update(Message::CloseActiveTab);
    assert_eq!(app.diagnostics_subscription().units(), 1);

    let cleanup = app.update_editor(Event::RefreshDiagnostics);
    let mut actions = iced_test::runtime::task::into_stream(cleanup).unwrap();
    while let Some(action) = iced::futures::StreamExt::next(&mut actions).await {
        if let iced_test::runtime::Action::Output(message) = action {
            let _ = app.update(message);
        }
    }
    assert_eq!(app.diagnostics_subscription().units(), 0);
    assert!(!app.editor.diagnostics_busy);
}

#[test]
fn diagnostic_responses_release_busy_state_while_editor_actions_are_blocked() {
    for settings in [false, true] {
        let mut app = app();
        if settings {
            app.navigation = crate::app::Navigation::Settings;
        } else {
            app.files.operations.busy = true;
        }
        app.editor.diagnostics_busy = true;
        let report = diagnostics::Report {
            generation: app.generation,
            tab: app.preview().id,
            text: app.editor_buffer().unwrap().document.text.clone(),
            diagnostics: Vec::new(),
            failed: false,
        };
        let _ = app.update_editor(Event::Diagnostics(report.clone()));
        assert!(!app.editor.diagnostics_busy);
        assert!(app.editor.diagnostics.is_some());

        app.editor.diagnostics = None;
        app.editor.diagnostics_busy = true;
        let _ = app.update_editor(Event::Diagnostics(diagnostics::Report {
            generation: report.generation.wrapping_add(1),
            ..report
        }));
        assert!(!app.editor.diagnostics_busy);
        assert!(app.editor.diagnostics.is_none());

        app.editor.diagnostics_busy = true;
        let _ = app.update_editor(Event::LanguagesMaintained);
        assert!(!app.editor.diagnostics_busy);
    }
}

#[test]
fn language_overlays_preserve_editor_widget_state() {
    use crate::domain::language::{Diagnostic, Position, Range};
    use iced::advanced::widget::{Tree, tree};
    fn first_stateful_leaf(tree: &mut Tree) -> &mut Tree {
        if tree.children.is_empty() && tree.tag != tree::Tag::stateless() {
            return tree;
        }
        first_stateful_leaf(tree.children.first_mut().unwrap())
    }
    let mut app = app();
    app.workspace.preferences.line_numbers = false;
    let mut tree = {
        let view = crate::ui::editor::view(&app, app.editor_buffer().unwrap());
        Tree::new(&view)
    };
    let leaf = first_stateful_leaf(&mut tree);
    let tag = leaf.tag;
    // Detect replacement of the allocation that owns focus and drag selection.
    leaf.state = tree::State::new(927_u64);
    app.editor.suggestions = Some(completion::Suggestions {
        opened: std::time::Instant::now(),
        revision: 0,
        tab: app.preview().id,
        snapshot: app.editor_buffer().unwrap().document.text.clone(),
        cursor: text_editor::Position { line: 0, column: 0 },
        items: vec![serde_json::json!({"label": "original"})],
        selected: 0,
    });
    for visible in [true, false] {
        if !visible {
            app.editor.suggestions = None;
        }
        app.editor.diagnostics = visible.then(|| diagnostics::Report {
            generation: app.generation,
            tab: app.preview().id,
            text: app.editor_buffer().unwrap().document.text.clone(),
            diagnostics: vec![Diagnostic {
                range: Range {
                    start: Position {
                        line: 0,
                        character: 0,
                    },
                    end: Position {
                        line: 0,
                        character: 1,
                    },
                },
                severity: Some(1),
                message: "Test diagnostic".into(),
            }],
            failed: false,
        });
        let view = crate::ui::editor::view(&app, app.editor_buffer().unwrap());
        tree.diff(&view);
        let leaf = first_stateful_leaf(&mut tree);
        assert_eq!(leaf.tag, tag);
        assert_eq!(*leaf.state.downcast_ref::<u64>(), 927);
    }
}

#[test]
fn tab_accepts_the_selected_completion_instead_of_inserting_spaces() {
    use iced::{
        Event as Input,
        keyboard::{self, Key as KeyboardKey, Modifiers, key},
    };
    let mut app = app();
    let id = app.preview().id;
    app.editor.suggestions = Some(completion::Suggestions {
        opened: std::time::Instant::now(),
        revision: 0,
        tab: id,
        snapshot: app.editor_buffer().unwrap().document.text.clone(),
        cursor: text_editor::Position { line: 0, column: 0 },
        items: vec![
            serde_json::json!({"label":"first"}),
            serde_json::json!({"label":"second"}),
        ],
        selected: 1,
    });
    let mut ui =
        iced_test::Simulator::with_size(iced::Settings::default(), (1080., 720.), app.view());
    ui.click(iced::widget::Id::from(format!("text-editor-{id}")))
        .unwrap();
    ui.simulate([Input::Keyboard(keyboard::Event::KeyPressed {
        key: KeyboardKey::Named(key::Named::Tab),
        modified_key: KeyboardKey::Named(key::Named::Tab),
        physical_key: key::Physical::Code(key::Code::Tab),
        location: keyboard::Location::Standard,
        modifiers: Modifiers::empty(),
        repeat: false,
        text: None,
    })]);
    let messages: Vec<_> = ui.into_messages().collect();
    assert!(
        messages
            .iter()
            .any(|message| matches!(message, Message::Editor(Event::AcceptCompletion(1))))
    );
    assert!(!messages.iter().any(
        |message| matches!(message, Message::Editor(Event::Action(_, action)) if action.is_edit())
    ));
}

#[test]
fn duplicating_a_line_preserves_endings_and_undoes_in_one_step() {
    for (before, after, line) in [
        ("first\nlast", "first\nfirst\nlast", 0),
        ("first\r\nlast", "first\r\nfirst\r\nlast", 0),
        ("first\r\nlast", "first\r\nlast\r\nlast", 1),
        ("слово", "слово\nслово", 0),
        ("", "\n", 0),
    ] {
        let mut app = app();
        edit(&mut app, before);
        let id = app.preview().id;
        app.preview_for_request(id)
            .unwrap()
            .editor
            .as_mut()
            .unwrap()
            .content
            .move_to(text_editor::Cursor {
                position: text_editor::Position { line, column: 0 },
                selection: None,
            });
        let _ = app.update_editor(Event::DuplicateLine);
        assert_eq!(app.editor_buffer().unwrap().document.text.as_str(), after);
        assert_eq!(
            app.editor_buffer().unwrap().content.cursor().position.line,
            line + 1
        );
        let _ = app.update_editor(Event::Undo(id, false));
        assert_eq!(app.editor_buffer().unwrap().document.text.as_str(), before);
    }
}

#[test]
fn formatting_rejects_changed_text_and_undoes_in_one_step() {
    let mut app = app();
    edit(&mut app, "a");
    let id = app.preview().id;
    let report = formatting::Report {
        generation: app.generation,
        tab: id,
        snapshot: Arc::new("a".into()),
        edits: Ok(vec![crate::domain::language::TextEdit {
            range: crate::domain::language::Range {
                start: crate::domain::language::Position {
                    line: 0,
                    character: 0,
                },
                end: crate::domain::language::Position {
                    line: 0,
                    character: 1,
                },
            },
            new_text: "formatted".into(),
        }]),
    };
    edit(&mut app, "changed");
    let _ = app.update_editor(Event::Formatted(report.clone()));
    assert_eq!(
        app.editor_buffer().unwrap().document.text.as_str(),
        "changed"
    );
    edit(&mut app, "a");
    let _ = app.update_editor(Event::Formatted(report));
    assert_eq!(
        app.editor_buffer().unwrap().document.text.as_str(),
        "formatted"
    );
    let _ = app.update_editor(Event::Undo(id, false));
    assert_eq!(app.editor_buffer().unwrap().document.text.as_str(), "a");
}

#[test]
fn modified_backspace_deletes_words_and_line_prefixes() {
    use iced::{
        Event as Input,
        keyboard::{self, Key as KeyboardKey, Modifiers, key},
    };
    let mut cases = vec![
        (Modifiers::empty(), "alpha bet"),
        (
            if cfg!(target_os = "macos") {
                Modifiers::ALT
            } else {
                Modifiers::CTRL
            },
            "alpha ",
        ),
    ];
    if cfg!(target_os = "macos") {
        cases.push((Modifiers::COMMAND, ""));
    }
    for (modifiers, expected) in cases {
        let mut app = app();
        edit(&mut app, "alpha beta");
        let id = app.preview().id;
        let messages = {
            let mut ui = iced_test::Simulator::with_size(
                iced::Settings::default(),
                (1080., 720.),
                app.view(),
            );
            ui.click(iced::widget::Id::from(format!("text-editor-{id}")))
                .unwrap();
            ui.simulate([Input::Keyboard(keyboard::Event::KeyPressed {
                key: KeyboardKey::Named(key::Named::Backspace),
                modified_key: KeyboardKey::Named(key::Named::Backspace),
                physical_key: key::Physical::Code(key::Code::Backspace),
                location: keyboard::Location::Standard,
                modifiers,
                repeat: false,
                text: None,
            })]);
            ui.into_messages().collect::<Vec<_>>()
        };
        for message in messages {
            let _ = app.update(message);
        }
        assert_eq!(
            app.editor_buffer().unwrap().document.text.as_str(),
            expected,
            "{modifiers:?}"
        );
        let _ = app.update_editor(Event::Undo(id, false));
        assert_eq!(
            app.editor_buffer().unwrap().document.text.as_str(),
            "alpha beta"
        );
    }
}

#[test]
fn word_selection_uses_arrow_keys_even_with_an_empty_text_payload() {
    use iced::{
        Event as Input,
        keyboard::{self, Key as KeyboardKey, Modifiers, key},
    };
    let mut app = app();
    edit(&mut app, "alpha beta");
    let id = app.preview().id;
    let messages = {
        let mut ui =
            iced_test::Simulator::with_size(iced::Settings::default(), (1080., 720.), app.view());
        ui.click(iced::widget::Id::from(format!("text-editor-{id}")))
            .unwrap();
        ui.simulate([Input::Keyboard(keyboard::Event::KeyPressed {
            key: KeyboardKey::Named(key::Named::ArrowLeft),
            modified_key: KeyboardKey::Named(key::Named::ArrowLeft),
            physical_key: key::Physical::Code(key::Code::ArrowLeft),
            location: keyboard::Location::Standard,
            modifiers: Modifiers::SHIFT
                | if cfg!(target_os = "macos") {
                    Modifiers::ALT
                } else {
                    Modifiers::CTRL
                },
            repeat: false,
            text: Some("".into()),
        })]);
        ui.into_messages().collect::<Vec<_>>()
    };
    for message in messages {
        let _ = app.update(message);
    }
    assert_eq!(
        app.editor_buffer().unwrap().content.selection().as_deref(),
        Some("beta")
    );
    assert_eq!(
        app.editor_buffer().unwrap().document.text.as_str(),
        "alpha beta"
    );
}

#[test]
fn completion_popup_fits_and_insertion_is_one_undo_step() {
    let mut app = app();
    edit(&mut app, "person.gr");
    let id = app.preview().id;
    let buffer = app
        .preview_for_request(id)
        .unwrap()
        .editor
        .as_mut()
        .unwrap();
    buffer.content.move_to(text_editor::Cursor {
        position: text_editor::Position { line: 0, column: 9 },
        selection: None,
    });
    app.editor.suggestions = Some(completion::Suggestions {
        opened: std::time::Instant::now() - crate::ui::theme::COMPLETION_APPEAR,
        revision: app.editor.completion_revision,
        tab: id,
        snapshot: Arc::new("person.gr".into()),
        cursor: text_editor::Position { line: 0, column: 9 },
        items: vec![
            serde_json::json!({"label": "greet", "kind": 2, "detail": "() -> str"}),
            serde_json::json!({"label": "group", "kind": 5, "detail": "str"}),
        ],
        selected: 0,
    });
    for appearance in [
        crate::domain::workspace::Appearance::Dark,
        crate::domain::workspace::Appearance::Light,
    ] {
        app.workspace.preferences.appearance = appearance;
        let mut ui =
            iced_test::Simulator::with_size(iced::Settings::default(), (1080., 720.), app.view());
        let bounds = ui
            .find(iced::widget::Id::from("editor-completion-popup"))
            .unwrap()
            .visible_bounds()
            .unwrap();
        assert!(bounds.x >= 0. && bounds.x + bounds.width <= 1080.);
        assert!(bounds.y >= 0. && bounds.y + bounds.height <= 720.);
        if let Some(output) = std::env::var_os("IWE_RENDER_DIR") {
            ui.snapshot(&app.theme())
                .unwrap()
                .matches_image(
                    std::path::PathBuf::from(output).join(format!("completion-{appearance:?}")),
                )
                .unwrap();
        }
    }
    let _ = app.update_editor(Event::AcceptCompletion(0));
    assert_eq!(
        app.editor_buffer().unwrap().document.text.as_str(),
        "person.greet"
    );
    assert_eq!(
        app.editor_buffer()
            .unwrap()
            .content
            .cursor()
            .position
            .column,
        12
    );
    let _ = app.update_editor(Event::Undo(id, false));
    assert_eq!(
        app.editor_buffer().unwrap().document.text.as_str(),
        "person.gr"
    );
}

#[test]
fn code_editor_scales_wheel_and_trackpad_movement() {
    use iced::{Event as IcedEvent, mouse, widget::Id};

    let app = app();
    let id = app.preview().id;
    for delta in [
        mouse::ScrollDelta::Lines { x: 0., y: -1. },
        mouse::ScrollDelta::Pixels { x: 0., y: -16. },
    ] {
        let mut ui =
            iced_test::Simulator::with_size(iced::Settings::default(), (1080., 720.), app.view());
        let bounds = ui
            .find(Id::from(format!("text-editor-{id}")))
            .unwrap()
            .visible_bounds()
            .unwrap();
        ui.point_at(bounds.center());
        ui.simulate([IcedEvent::Mouse(mouse::Event::WheelScrolled { delta })]);
        assert!(ui.into_messages().any(|message| matches!(
            message,
            Message::Editor(Event::Action(target, text_editor::Action::Scroll { lines: 3 }))
                if target == id
        )));
    }
}

#[test]
fn local_edit_does_not_change_disk_snapshot_and_refresh_cannot_replace_draft() {
    let mut app = app();
    edit(&mut app, "draft\r\n");
    assert_eq!(
        app.preview().document.as_ref().unwrap().text,
        "original\r\n"
    );
    assert!(app.editor_buffer().unwrap().document.dirty());
    let id = app.preview().id;
    let _ = app.finish_preview(id, Ok(Document::plain("external".into())));
    assert_eq!(app.editor_buffer().unwrap().content.text(), "draft\r\n");
    let _ = app.update_editor(Event::Undo(id, false));
    assert_eq!(app.editor_buffer().unwrap().content.text(), "original\r\n");
}

#[test]
fn failed_saves_require_a_decision_before_close_and_project_switch() {
    let mut app = app();
    edit(&mut app, "draft");
    let id = app.preview().id;
    let _ = app.save_editor(id);
    let token = app.editor_buffer().unwrap().token;
    let _ = app.update_editor(Event::Saved(token, Err(Key::EditorConflict)));
    let _ = app.update(Message::CloseTab(id));
    assert!(app.editor.pending.is_some());
    assert_eq!(app.preview().id, id);
    let _ = app.update_editor(Event::Cancel);
    let _ = app.update(Message::SelectProject(0, 1));
    assert_eq!(app.workspace.selected_project, Some(0));
    assert!(app.editor.pending.is_some());
    let _ = app.update_editor(Event::Cancel);
    let _ = app.update(Message::CloseTab(id));
    let _ = app.update_editor(Event::Discard);
    assert!(app.editor.pending.is_none());
    assert_ne!(app.preview().id, id);
}

#[test]
fn save_completion_keeps_changes_typed_during_save_and_failures_keep_drafts() {
    let mut app = app();
    edit(&mut app, "submitted");
    let id = app.preview().id;
    let _ = app.save_editor(id);
    let token = app.editor_buffer().unwrap().token;
    edit(&mut app, "newer");
    let _ = app.update_editor(Event::Saved(token, Ok(())));
    assert_eq!(
        app.editor_buffer().unwrap().document.saved.as_str(),
        "submitted"
    );
    assert_eq!(app.editor_buffer().unwrap().content.text(), "newer");
    assert!(app.editor_buffer().unwrap().document.dirty());
    let _ = app.save_editor(id);
    let token = app.editor_buffer().unwrap().token;
    let _ = app.update_editor(Event::Saved(token, Err(Key::EditorConflict)));
    assert_eq!(app.editor_buffer().unwrap().content.text(), "newer");
    assert_eq!(
        app.editor_buffer().unwrap().error,
        Some(Key::EditorConflict)
    );
}

#[test]
fn reopening_remote_file_preserves_its_draft_and_original_profile() {
    let mut app = app();
    let _ = app.open_remote_preview(
        "/srv/file.txt".into(),
        "/srv/file.txt".into(),
        ("stage".into(), "Stage".into()),
        Arc::new("server".into()),
        None,
    );
    let id = app.preview().id;
    let _ = app.finish_preview(id, Ok(Document::plain("server".into())));
    edit(&mut app, "remote draft");
    let _ = app.open_remote_preview(
        "/srv/file.txt".into(),
        "/srv/file.txt".into(),
        ("stage".into(), "Stage".into()),
        Arc::new("external".into()),
        None,
    );
    assert_eq!(app.preview().id, id);
    assert_eq!(app.editor_buffer().unwrap().content.text(), "remote draft");
    assert!(
        matches!(&app.editor_buffer().unwrap().target, Target::Remote { profile, .. } if profile == "stage")
    );
}

#[test]
fn editor_keyboard_input_and_save_work_with_russian_physical_keys() {
    use iced::{
        Event as IcedEvent,
        keyboard::{self, Key as KeyboardKey, Modifiers, key},
        widget::Id,
    };
    for language in [Language::English, Language::Russian, Language::Spanish] {
        let mut app = app();
        app.i18n = Rc::new(I18n::new(language));
        let id = app.preview().id;
        let messages = {
            let mut ui = iced_test::Simulator::with_size(
                iced::Settings::default(),
                (1080., 720.),
                app.view(),
            );
            ui.click(Id::from(format!("text-editor-{id}"))).unwrap();
            ui.typewrite("x");
            ui.simulate([IcedEvent::Keyboard(keyboard::Event::KeyPressed {
                key: KeyboardKey::Character("ы".into()),
                modified_key: KeyboardKey::Character("ы".into()),
                physical_key: key::Physical::Code(key::Code::KeyS),
                location: keyboard::Location::Standard,
                modifiers: Modifiers::COMMAND,
                repeat: false,
                text: None,
            })]);
            ui.into_messages().collect::<Vec<_>>()
        };
        assert!(messages.iter().any(|message| matches!(message, Message::Editor(Event::Action(_, action)) if action.is_edit())));
        assert!(
            messages
                .iter()
                .any(|message| matches!(message, Message::Editor(Event::Save)))
        );
        for message in messages {
            let _ = app.update(message);
        }
        assert!(app.editor_buffer().unwrap().document.dirty());
        let token = app.editor_buffer().unwrap().token;
        let _ = app.update_editor(Event::Saved(token, Err(Key::EditorSaveFailed)));
        if let Some(output) = std::env::var_os("IWE_RENDER_DIR") {
            let mut ui = iced_test::Simulator::with_size(
                iced::Settings::default(),
                (1080., 720.),
                app.view(),
            );
            ui.snapshot(&iced::Theme::Dark)
                .unwrap()
                .matches_image(PathBuf::from(output).join(format!("editor-buffer-{language:?}")))
                .unwrap();
        }
        let _ = app.update(Message::CloseActiveTab);
        let mut ui =
            iced_test::Simulator::with_size(iced::Settings::default(), (1080., 720.), app.view());
        assert!(
            ui.find(app.i18n.text(Key::EditorUnsavedTitle))
                .unwrap()
                .visible_bounds()
                .is_some()
        );
        ui.click(app.i18n.text(Key::Cancel)).unwrap();
        if let Some(output) = std::env::var_os("IWE_RENDER_DIR") {
            ui.snapshot(&iced::Theme::Dark)
                .unwrap()
                .matches_image(PathBuf::from(output).join(format!("editor-{language:?}")))
                .unwrap();
        }
        assert!(
            ui.into_messages()
                .any(|message| matches!(message, Message::Editor(Event::Cancel)))
        );
    }
}

#[cfg(target_os = "macos")]
#[test]
fn command_chords_do_not_type_in_fields_or_the_code_editor() {
    use iced::{
        Event as IcedEvent,
        keyboard::{self, Key, Modifiers, key},
        widget::{Id, text_input},
    };

    let press = |key: Key, code, text: &str, repeat| {
        IcedEvent::Keyboard(keyboard::Event::KeyPressed {
            modified_key: key.clone(),
            key,
            physical_key: key::Physical::Code(code),
            location: keyboard::Location::Standard,
            modifiers: Modifiers::COMMAND,
            repeat,
            text: Some(text.into()),
        })
    };
    let chords = [
        press(Key::Character("f".into()), key::Code::KeyF, "f", false),
        press(Key::Character("f".into()), key::Code::KeyF, "f", true),
        press(Key::Character("ж".into()), key::Code::Semicolon, "ж", false),
    ];

    let field = text_input("", "seed").id("field").on_input(|value| value);
    let mut input: iced_test::Simulator<String> = iced_test::Simulator::new(field);
    input.click(Id::from("field")).unwrap();
    // Command is already held when the field receives focus.
    input.simulate(chords.clone());
    input.simulate([press(
        Key::Character("a".into()),
        key::Code::KeyA,
        "a",
        false,
    )]);
    input.typewrite("x");
    assert_eq!(input.into_messages().collect::<Vec<_>>(), ["x"]);

    let app = app();
    let mut editor =
        iced_test::Simulator::with_size(iced::Settings::default(), (1080., 720.), app.view());
    editor
        .click(Id::from(format!("text-editor-{}", app.preview().id)))
        .unwrap();
    editor.simulate(chords);
    editor.simulate([
        press(Key::Named(key::Named::Tab), key::Code::Tab, "\t", true),
        press(Key::Named(key::Named::Enter), key::Code::Enter, "\r", true),
    ]);
    editor.typewrite("x");
    let edits: Vec<_> = editor
        .into_messages()
        .filter_map(|message| match message {
            Message::Editor(Event::Action(_, text_editor::Action::Edit(edit))) => Some(edit),
            _ => None,
        })
        .collect();
    assert!(matches!(edits.as_slice(), [text_editor::Edit::Insert('x')]));
}

#[test]
fn native_close_preserves_dirty_buffers_and_settings_do_not_save_them() {
    let mut app = app();
    edit(&mut app, "draft");
    let window = iced::window::Id::unique();
    app.window_id = Some(window);
    let _ = app.update_window(super::super::windows::Event::Close(window));
    assert!(app.editor.pending.is_some());
    assert!(app.editor_buffer().unwrap().saving);
    let token = app.editor_buffer().unwrap().token;
    let _ = app.update_editor(Event::Saved(token, Err(Key::EditorSaveFailed)));
    let _ = app.update_editor(Event::Cancel);
    app.navigation = super::super::Navigation::Settings;
    let _ = app.update(Message::Editor(Event::Save));
    assert!(!app.editor_buffer().unwrap().saving);
    assert_eq!(app.editor_buffer().unwrap().content.text(), "draft");
}

#[tokio::test]
async fn save_and_close_writes_the_selected_file_without_saving_other_drafts() {
    use iced::futures::StreamExt;
    use iced_test::runtime::{Action, task};
    let root = tempfile::tempdir().unwrap();
    let mut app = App::new(Rc::new(I18n::new(Language::English)));
    app.workspace
        .create_space("Test".into(), vec![root.path().into()]);
    app.loaded = true;
    let mut ids = Vec::new();
    for name in ["one.txt", "two.txt"] {
        let path = root.path().join(name);
        std::fs::write(&path, "original").unwrap();
        let _ = app.open_file(path);
        let id = app.preview().id;
        let _ = app.finish_preview(id, Ok(Document::plain("original".into())));
        edit(&mut app, name);
        ids.push(id);
    }
    let mut actions = task::into_stream(app.update(Message::CloseTab(ids[0]))).unwrap();
    while let Some(action) = actions.next().await {
        if let Action::Output(message) = action {
            let _ = app.update(message);
        }
    }
    assert_eq!(
        std::fs::read_to_string(root.path().join("one.txt")).unwrap(),
        "one.txt"
    );
    assert_eq!(
        std::fs::read_to_string(root.path().join("two.txt")).unwrap(),
        "original"
    );
    assert!(!app.tabs.files.iter().any(|tab| tab.id == ids[0]));
    assert!(app.editor.pending.is_none());
    assert_eq!(app.editor_buffer().unwrap().content.text(), "two.txt");
    assert!(app.editor_buffer().unwrap().document.dirty());
}

#[test]
fn autosave_ignores_old_timers_and_resumes_newer_edits_after_completion() {
    let mut app = app();
    let id = app.preview().id;
    edit(&mut app, "first");
    let old = app.editor_buffer().unwrap().autosave;
    edit(&mut app, "second");
    let revision = app.editor_buffer().unwrap().autosave;
    let _ = app.update_editor(Event::Autosave(id, old));
    assert!(!app.editor_buffer().unwrap().saving);
    let _ = app.update_editor(Event::Autosave(id, revision));
    let token = app.editor_buffer().unwrap().token;
    assert_eq!(app.editor_buffer().unwrap().snapshot.as_str(), "second");
    edit(&mut app, "third");
    let revision = app.editor_buffer().unwrap().autosave;
    let _ = app.update_editor(Event::Autosave(id, revision));
    assert_eq!(app.editor_buffer().unwrap().token, token);
    let _ = app.update_editor(Event::Saved(token, Ok(())));
    let revision = app.editor_buffer().unwrap().autosave;
    let _ = app.update_editor(Event::Autosave(id, revision));
    assert_eq!(app.editor_buffer().unwrap().snapshot.as_str(), "third");
    assert_eq!(
        app.editor_buffer().unwrap().document.saved.as_str(),
        "second"
    );
}

#[test]
fn autosave_errors_stop_retries_even_after_more_typing() {
    let mut app = app();
    let id = app.preview().id;
    edit(&mut app, "draft");
    let revision = app.editor_buffer().unwrap().autosave;
    let _ = app.update_editor(Event::Autosave(id, revision));
    let token = app.editor_buffer().unwrap().token;
    let _ = app.update_editor(Event::Saved(token, Err(Key::EditorConflict)));
    edit(&mut app, "retained");
    let revision = app.editor_buffer().unwrap().autosave;
    let _ = app.update_editor(Event::Autosave(id, revision));
    assert!(!app.editor_buffer().unwrap().saving);
    assert_eq!(app.editor_buffer().unwrap().content.text(), "retained");
    assert_eq!(
        app.notifications.items.back().unwrap().message,
        Key::EditorConflict
    );
    let _ = app.update_editor(Event::Save);
    assert!(app.editor_buffer().unwrap().saving);
    assert!(app.editor_buffer().unwrap().error.is_none());
}

#[tokio::test]
async fn autosave_writes_inactive_file_after_delay_and_undo_cancels_changes() {
    use iced::futures::StreamExt;
    use iced_test::runtime::{Action, task};
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("file.txt");
    std::fs::write(&path, "original").unwrap();
    let mut app = app();
    let _ = app.open_file(path.clone());
    let id = app.preview().id;
    let _ = app.finish_preview(id, Ok(Document::plain("original".into())));
    edit(&mut app, "saved automatically");
    let timer = app.schedule_autosave(id);
    let _ = app.open_file(root.path().join("other.txt"));
    app.navigation = super::super::Navigation::Settings;
    let mut timer = task::into_stream(timer).unwrap();
    let Action::Output(message) = timer.next().await.unwrap() else {
        panic!("Expected autosave");
    };
    let mut save = task::into_stream(app.update(message)).unwrap();
    let Action::Output(message) = save.next().await.unwrap() else {
        panic!("Expected save result");
    };
    let _ = app.update(message);
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        "saved automatically"
    );
    assert!(
        !app.tabs
            .files
            .iter()
            .find(|tab| tab.id == id)
            .unwrap()
            .editor
            .as_ref()
            .unwrap()
            .protected()
    );
    app.navigation = super::super::Navigation::Files;
    let _ = app.select_tab(id);
    edit(&mut app, "discard by undo");
    let revision = app.editor_buffer().unwrap().autosave;
    let _ = app.update_editor(Event::Undo(id, false));
    let _ = app.update_editor(Event::Autosave(id, revision));
    assert!(!app.editor_buffer().unwrap().protected());
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        "saved automatically"
    );
}

#[test]
fn remote_autosave_waits_for_transfer_without_reporting_a_save_failure() {
    let mut app = app();
    let _ = app.open_remote_preview(
        "/test.txt".into(),
        "/test.txt".into(),
        ("stage".into(), "Stage".into()),
        Arc::new("server".into()),
        None,
    );
    let id = app.preview().id;
    let _ = app.finish_preview(id, Ok(Document::plain("server".into())));
    edit(&mut app, "draft");
    app.modules.deployment.busy = true;
    let revision = app.editor_buffer().unwrap().autosave;
    let _ = app.update_editor(Event::Autosave(id, revision));
    assert!(!app.editor_buffer().unwrap().saving);
    assert!(app.editor_buffer().unwrap().error.is_none());
    assert_eq!(app.editor_buffer().unwrap().content.text(), "draft");
    app.modules.deployment.busy = false;
    let _ = app.update_editor(Event::Autosave(id, revision));
    // The absent test profile fails validation, proving that the deferred save was attempted.
    assert!(app.editor_buffer().unwrap().error.is_some());
}

#[test]
fn undo_and_redo_keep_visible_code_when_the_caret_is_offscreen() {
    let mut app = app();
    let id = app.preview().id;
    let original = "source line\n".repeat(200);
    let _ = app.finish_preview(id, Ok(Document::plain(original.clone())));
    let _ = app.update_editor(Event::Action(
        id,
        text_editor::Action::Edit(text_editor::Edit::Insert('X')),
    ));
    let top_line = |app: &App| {
        let mut ui =
            iced_test::Simulator::with_size(iced::Settings::default(), (1080., 720.), app.view());
        ui.find(iced::widget::Id::from(format!("text-editor-{id}")))
            .unwrap();
        app.editor_buffer()
            .unwrap()
            .content
            .with_editor(|editor| editor.buffer().scroll().line)
    };
    assert_eq!(top_line(&app), 0);
    let _ = app.update_editor(Event::Action(id, text_editor::Action::Scroll { lines: 80 }));
    assert_eq!(top_line(&app), 80);

    for (redo, expected) in [
        (false, original.clone()),
        (false, original.clone()),
        (true, format!("X{original}")),
    ] {
        let _ = app.update_editor(Event::Undo(id, redo));
        let buffer = app.editor_buffer().unwrap();
        assert_eq!(buffer.content.text(), expected);
        assert_eq!(buffer.content.cursor().position.line, 0);
        assert_eq!(top_line(&app), 80);
    }
}

#[test]
fn editor_line_numbers_follow_scroll_and_settings_without_a_save_toolbar() {
    use iced::{Theme, widget::Id};
    let mut app = app();
    let id = app.preview().id;
    let text = (1..=200)
        .map(|line| format!("line {line}\n"))
        .collect::<String>();
    let _ = app.finish_preview(id, Ok(Document::plain(text)));
    app.workspace.preferences.line_numbers = true;
    for scrolled in [false, true] {
        if scrolled {
            let _ = app.update_editor(Event::Action(id, text_editor::Action::Scroll { lines: 40 }));
        }
        let mut ui =
            iced_test::Simulator::with_size(iced::Settings::default(), (1080., 720.), app.view());
        assert!(
            ui.find(Id::new("editor-line-numbers"))
                .unwrap()
                .visible_bounds()
                .is_some()
        );
        assert!(ui.find(app.i18n.text(Key::EditorSave)).is_err());
        let snapshot = ui.snapshot(&Theme::Dark).unwrap();
        if let Some(output) = std::env::var_os("IWE_RENDER_DIR") {
            snapshot
                .matches_image(PathBuf::from(output).join(format!("autosave-lines-{scrolled}")))
                .unwrap();
        }
        drop(ui);
        app.editor_buffer().unwrap().content.with_editor(|editor| {
            let lines = editor.buffer().layout_runs().collect::<Vec<_>>();
            assert!(!lines.is_empty());
            assert_eq!(lines[0].line_i, if scrolled { 40 } else { 0 });
            assert!(lines.len() < 100);
            for pair in lines.windows(2) {
                assert_eq!(pair[1].line_i, pair[0].line_i + 1);
                assert!((pair[1].line_top - pair[0].line_top - pair[0].line_height).abs() < 0.1);
            }
        });
    }
    app.workspace.preferences.line_numbers = false;
    let mut ui =
        iced_test::Simulator::with_size(iced::Settings::default(), (1080., 720.), app.view());
    assert!(ui.find(Id::new("editor-line-numbers")).is_err());
}

#[test]
fn large_editor_scrollbar_thumb_can_be_dragged_without_moving_the_caret() {
    use iced::{Event as IcedEvent, mouse, widget::Id};
    let mut app = app();
    let id = app.preview().id;
    let _ = app.finish_preview(id, Ok(Document::plain("source line\n".repeat(12_000))));
    let mut ui =
        iced_test::Simulator::with_size(iced::Settings::default(), (1080., 720.), app.view());
    let rail = ui
        .find(Id::new("editor-scrollbar"))
        .unwrap()
        .visible_bounds()
        .unwrap();
    ui.point_at((rail.center_x(), rail.y + 20.));
    ui.simulate([IcedEvent::Mouse(mouse::Event::ButtonPressed(
        mouse::Button::Left,
    ))]);
    let position = iced::Point::new(rail.center_x(), rail.y + rail.height * 0.5);
    ui.point_at(position);
    ui.simulate([
        IcedEvent::Mouse(mouse::Event::CursorMoved { position }),
        IcedEvent::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)),
    ]);
    let messages = ui.into_messages().collect::<Vec<_>>();
    assert!(messages.iter().any(|message| matches!(
        message,
        Message::Editor(Event::Action(_, text_editor::Action::Scroll { lines })) if *lines > 5_000
    )));
    for message in messages {
        let _ = app.update(message);
    }
    let buffer = app.editor_buffer().unwrap();
    assert_eq!(buffer.content.cursor().position.line, 0);
    assert!(!buffer.document.dirty());
}

#[test]
fn deployment_flushes_local_drafts_before_starting_and_keeps_failed_drafts() {
    use crate::{
        app::modules::deployment_message,
        domain::deployment::{Connection, Mapping, Profile},
        modules::deployment::{Event as DeploymentEvent, Operation},
    };
    for failed in [false, true] {
        for compare in [false, true] {
            let mut app = app();
            let project = std::env::temp_dir().join("iwe-deployment-flush");
            app.workspace.spaces[0].projects[0].directory = project.clone();
            app.tabs.files.clear();
            let path = project.join("file.txt");
            let _ = app.open_file(path.clone());
            let id = app.preview().id;
            let _ = app.finish_preview(id, Ok(Document::plain("original".into())));
            app.workspace.deployment.connections.push(Connection {
                id: "connection".into(),
                ..Default::default()
            });
            app.workspace.spaces[0].deployments.push(Profile {
                id: "profile".into(),
                name: "Server".into(),
                project: project.clone(),
                connection: "connection".into(),
                root: "/srv".into(),
                mappings: vec![Mapping::default()],
                default: true,
                ..Default::default()
            });
            edit(&mut app, "latest draft");
            let event = DeploymentEvent::Run(if compare {
                Operation::Compare(path.clone())
            } else {
                Operation::Upload(path.clone())
            });
            let _ = app.update(deployment_message(event));
            assert!(app.editor.pending.is_some());
            assert!(!app.modules.deployment.busy);
            assert!(app.editor_buffer().unwrap().saving);
            let token = app.editor_buffer().unwrap().token;
            let _ = app.update_editor(Event::Saved(
                token,
                if failed {
                    Err(Key::EditorConflict)
                } else {
                    Ok(())
                },
            ));
            assert_eq!(app.modules.deployment.busy, !failed);
            assert_eq!(app.editor.pending.is_some(), failed);
            if failed {
                assert_eq!(app.editor_buffer().unwrap().content.text(), "latest draft");
            } else {
                assert!(!app.modules.deployment.open);
            }
        }
    }
}

#[test]
fn uploading_folder_flushes_only_local_buffers_in_that_folder() {
    use crate::{
        app::modules::deployment_message,
        modules::deployment::{Event as DeploymentEvent, Operation},
    };
    let mut app = app();
    let local_id = app.preview().id;
    edit(&mut app, "local draft");
    let _ = app.open_remote_preview(
        "/srv/file.txt".into(),
        "/srv/file.txt".into(),
        ("profile".into(), "Server".into()),
        Arc::new("remote".into()),
        None,
    );
    let remote_id = app.preview().id;
    let _ = app.finish_preview(remote_id, Ok(Document::plain("remote".into())));
    edit(&mut app, "remote draft");
    let message = deployment_message(DeploymentEvent::Run(Operation::Upload("project".into())));
    assert!(app.protect_editor(&message));
    assert!(app.editor_pending_tab(local_id));
    assert!(!app.editor_pending_tab(remote_id));
}

#[tokio::test]
async fn file_move_waits_for_saved_edits_and_stops_on_external_conflicts() {
    use crate::{
        app::files::operations,
        infrastructure::file_operations::{Operation, Request},
    };
    use iced::futures::StreamExt;
    use iced_test::runtime::{Action, task};

    for conflicts in [false, true] {
        let project = tempfile::tempdir().unwrap();
        let mut app = App::new(Rc::new(I18n::new(Language::English)));
        app.workspace
            .create_space("Test".into(), vec![project.path().into()]);
        app.loaded = true;
        let original = project.path().join("old.txt");
        let renamed = project.path().join("new.txt");
        std::fs::write(&original, "original").unwrap();
        let _ = app.open_file(original.clone());
        let id = app.preview().id;
        let _ = app.finish_preview(id, Ok(Document::plain("original".into())));
        edit(&mut app, "draft");
        if conflicts {
            std::fs::write(&original, "external").unwrap();
        }
        let request = Request {
            operation: Operation::Move,
            source: Some(original.clone()),
            destination: Some(renamed.clone()),
        };
        let save = app.update(operations::message(operations::Event::Run(request)));
        assert!(app.editor_buffer().unwrap().saving);
        assert!(!renamed.exists());
        let mut actions = task::into_stream(save).unwrap();
        let mut operation = Task::none();
        while let Some(action) = actions.next().await {
            if let Action::Output(message) = action {
                operation = app.update(message);
            }
        }
        if conflicts {
            assert_eq!(std::fs::read_to_string(original).unwrap(), "external");
            assert!(!renamed.exists());
            assert!(!app.files.operations.busy);
            let buffer = app.editor_buffer().unwrap();
            assert!(buffer.document.dirty());
            assert_eq!(buffer.document.text.as_str(), "draft");
        } else {
            let mut actions = task::into_stream(operation).unwrap();
            while let Some(action) = actions.next().await {
                if let Action::Output(message) = action {
                    let _ = app.update(message);
                }
            }
            assert!(!original.exists());
            assert_eq!(std::fs::read_to_string(&renamed).unwrap(), "draft");
            assert!(
                matches!(&app.editor_buffer().unwrap().target, Target::Local(path) if *path == renamed)
            );
        }
    }
}

#[test]
fn failed_diagnostics_back_off_and_detecting_a_program_retries_promptly() {
    let mut app = app();
    let _ = app.open_file("project/source.rs".into());
    let tab = app.preview().id;
    let _ = app.finish_preview(tab, Ok(Document::new("fn main() {}".into(), "rs")));
    let report = diagnostics::Report {
        generation: app.generation,
        tab,
        text: app.editor_buffer().unwrap().document.text.clone(),
        diagnostics: Vec::new(),
        failed: true,
    };
    let _ = app.update_editor(Event::Diagnostics(report));
    assert_eq!(app.diagnostics_interval(), Some(Duration::from_secs(30)));
    let _ = app.update(Message::LanguageServerDetected(
        crate::domain::language::Language::Rust,
        None,
        true,
    ));
    assert_eq!(app.diagnostics_interval(), Some(Duration::from_millis(800)));
}

#[test]
fn right_click_preserves_selection_and_shows_grouped_actions() {
    use iced::{mouse, widget::Id};
    for language in [Language::English, Language::Russian, Language::Spanish] {
        let mut app = app();
        app.i18n = Rc::new(I18n::new(language));
        let id = app.preview().id;
        let _ = app.update_editor(Event::Action(id, text_editor::Action::SelectAll));
        let mut ui =
            iced_test::Simulator::with_size(iced::Settings::default(), (1080., 720.), app.view());
        let bounds = ui
            .find(Id::from(format!("text-editor-{id}")))
            .unwrap()
            .visible_bounds()
            .unwrap();
        ui.point_at(iced::Point::new(bounds.x + 20., bounds.y + 17.));
        ui.simulate([iced::Event::Mouse(mouse::Event::ButtonPressed(
            mouse::Button::Right,
        ))]);
        for message in ui.into_messages() {
            let _ = app.update(message);
        }
        assert!(app.editor_buffer().unwrap().content.selection().is_some());
        let mut ui =
            iced_test::Simulator::with_size(iced::Settings::default(), (1080., 720.), app.view());
        assert!(ui.find(app.i18n.text(Key::EditorRefactor)).is_err());
        assert!(
            ui.find(app.i18n.text(Key::GoToLine))
                .unwrap()
                .visible_bounds()
                .is_some()
        );
        ui.click(app.i18n.text(Key::EditorNavigate)).unwrap();
        ui.click(app.i18n.text(Key::FileCut)).unwrap();
        for message in ui.into_messages() {
            let _ = app.update(message);
        }
        assert!(app.editor.menu.is_none());
        assert!(app.editor_buffer().unwrap().document.text.is_empty());
        let _ = app.update_editor(Event::Undo(id, false));
        assert_eq!(
            app.editor_buffer().unwrap().document.text.as_str(),
            "original\r\n"
        );
    }
}

#[test]
fn grouped_editor_actions_share_one_menu_and_keyboard_skips_headings() {
    use iced::keyboard::key::Named;
    use text_editor::Position;
    for language in [Language::English, Language::Russian, Language::Spanish] {
        let mut app = app();
        app.i18n = Rc::new(I18n::new(language));
        let _ = app.open_file("project/source.rs".into());
        let id = app.preview().id;
        let _ = app.finish_preview(id, Ok(Document::new("let sample = 1;".into(), "rs")));
        let open = menu::Event::Open(
            id,
            iced::Point::new(1075., 710.),
            Some(Position { line: 0, column: 5 }),
        );
        let _ = app.update_editor(Event::Menu(open.clone()));
        fn view(app: &App) -> iced_test::Simulator<'_, Message> {
            iced_test::Simulator::with_size(iced::Settings::default(), (1080., 720.), app.view())
        }
        let mut ui = view(&app);
        let mut bottom = 0.;
        for key in [
            Key::EditorNavigate,
            Key::GoToLine,
            Key::GoToDefinition,
            Key::EditorUsages,
            Key::EditorRefactor,
            Key::EditorRenameLocal,
            Key::FormatCode,
        ] {
            let row = ui
                .find(app.i18n.text(key))
                .unwrap()
                .visible_bounds()
                .unwrap();
            assert!(row.y >= bottom && row.y + row.height <= 720.);
            bottom = row.y + row.height;
        }
        ui.click(app.i18n.text(Key::EditorRefactor)).unwrap();
        assert!(ui.into_messages().next().is_none());
        for key in [Named::ArrowUp, Named::ArrowUp, Named::Enter] {
            let mut ui = view(&app);
            ui.tap_key(key);
            for message in ui.into_messages() {
                let _ = app.update(message);
            }
        }
        assert_eq!(app.editor.rename.as_ref().unwrap().symbol, "sample");
        assert!(app.editor.menu.is_none());
        let _ = app.update_editor(Event::Menu(menu::Event::RenameCancel));
        let _ = app.update_editor(Event::Menu(open));
        let mut ui = view(&app);
        ui.click(app.i18n.text(Key::GoToLine)).unwrap();
        for message in ui.into_messages() {
            let _ = app.update(message);
        }
        assert!(app.line_navigation.is_some());
        assert!(app.editor.menu.is_none());
    }
}

#[test]
fn clicking_git_gutter_shows_original_and_current_lines() {
    use iced::{mouse, widget::Id};
    let mut app = app();
    let tab = app.preview().id;
    let after = app.editor_buffer().unwrap().document.text.clone();
    let before = Arc::new("previous\r\n".to_string());
    let hunks = Arc::new(crate::domain::editor::hunks::compare(&before, &after));
    let report = marks::Event::Compared(app.generation, tab, 0, before, after, hunks);
    let _ = app.update_editor(Event::Marks(report.clone()));
    let mut ui =
        iced_test::Simulator::with_size(iced::Settings::default(), (1080., 720.), app.view());
    let gutter = ui
        .find(Id::new("editor-line-numbers"))
        .unwrap()
        .visible_bounds()
        .unwrap();
    ui.point_at(iced::Point::new(
        gutter.x + crate::ui::theme::FILE_PANEL_RESIZE_GRAB + 4.,
        gutter.y + 16.,
    ));
    ui.simulate([iced::Event::Mouse(mouse::Event::ButtonPressed(
        mouse::Button::Left,
    ))]);
    for message in ui.into_messages() {
        let _ = app.update(message);
    }
    assert!(app.editor_buffer().unwrap().marks.open.is_some());
    let _ = app.update_editor(Event::Marks(report));
    let mut ui =
        iced_test::Simulator::with_size(iced::Settings::default(), (1080., 720.), app.view());
    assert!(ui.find("− previous").unwrap().visible_bounds().is_some());
    assert!(ui.find("+ original").unwrap().visible_bounds().is_some());
}
