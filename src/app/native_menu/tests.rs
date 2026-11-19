use super::*;
use crate::i18n::{I18n, Language};
use std::rc::Rc;

#[tokio::test]
async fn native_menu_waits_for_selections_when_idle() {
    use iced::advanced::subscription::into_recipes;
    use std::time::Duration;

    let mut app = App::new(Rc::new(I18n::new(Language::English)));
    assert!(into_recipes(app.native_menu_subscription()).is_empty());
    let _ = app.window_opened(window::Id::unique());
    let mut recipes = into_recipes(app.native_menu_subscription());
    assert_eq!(recipes.len(), 1);
    let mut events = recipes.remove(0).stream(Box::pin(stream::pending()));
    assert!(matches!(
        events.next().await,
        Some(Message::NativeMenu(Event::Ready))
    ));
    assert!(
        tokio::time::timeout(Duration::from_millis(250), events.next())
            .await
            .is_err()
    );
}

#[test]
fn native_menu_syncs_only_when_its_contents_change() {
    let mut app = App::new(Rc::new(I18n::new(Language::English)));
    let _ = app.window_opened(window::Id::unique());
    assert_eq!(app.sync_native_menu().units(), 0);
    let _ = app.update_native_menu(Event::Ready);
    assert!(app.sync_native_menu().units() > 0);
    assert_eq!(app.sync_native_menu().units(), 0);
    app.loaded = true;
    app.workspace
        .create_space("Work".into(), vec!["/front".into()]);
    assert!(app.sync_native_menu().units() > 0);
    assert_eq!(app.sync_native_menu().units(), 0);
    let _ = app.update_space(spaces::Message::New);
    assert!(app.sync_native_menu().units() > 0);
    assert_eq!(app.sync_native_menu().units(), 0);
}

#[test]
fn menu_accelerators_pause_only_while_recording_shortcuts() {
    let mut app = App::new(Rc::new(I18n::new(Language::English)));
    assert!(!app.menu_model().recording_shortcut);
    let _ = app.update_shortcuts(super::super::shortcuts::Event::Record(
        crate::domain::shortcuts::Command::Copy,
    ));
    assert!(app.menu_model().recording_shortcut);
    let _ = app.update_shortcuts(super::super::shortcuts::Event::Cancel);
    assert!(!app.menu_model().recording_shortcut);
}

#[test]
fn menu_model_tracks_spaces_projects_and_languages() {
    for language in [Language::English, Language::Russian, Language::Spanish] {
        let mut app = App::new(Rc::new(I18n::new(language)));
        app.loaded = true;
        app.workspace
            .create_space("Work".into(), vec!["/front".into(), "/back".into()]);
        let model = app.menu_model();
        assert_eq!(model.settings, app.i18n.text(Key::Settings));
        assert_eq!(model.new_space, app.i18n.text(Key::NewSpace));
        assert_eq!(model.spaces[0].0, "Work");
        assert_eq!(model.spaces[0].1[0].0, "front");
        assert_eq!(model.spaces[0].1[1].0, "back");
        assert!(model.enabled);
        let _ = app.update_space(spaces::Message::New);
        assert!(!app.menu_model().enabled);
    }
}

#[test]
fn new_space_menu_action_opens_creation_and_respects_modal_dialogs() {
    let mut app = App::new(Rc::new(I18n::new(Language::English)));
    app.loaded = true;
    app.workspace.create_space("Work".into(), vec![]);
    let _ = app.update_native_menu(Event::Selected(Action::NewSpace));
    assert!(
        app.space_draft
            .as_ref()
            .is_some_and(|draft| draft.editing.is_none())
    );
    assert_eq!(app.workspace.spaces.len(), 1);
    assert!(!app.menu_model().enabled);
    let _ = app.update_native_menu(Event::Selected(Action::NewSpace));
    assert_eq!(app.workspace.spaces.len(), 1);
    assert!(
        app.space_draft
            .as_ref()
            .is_some_and(|draft| draft.editing.is_none())
    );
}

#[test]
fn menu_project_actions_resolve_paths_after_reordering() {
    let mut app = App::new(Rc::new(I18n::new(Language::English)));
    app.loaded = true;
    app.workspace
        .create_space("Work".into(), vec!["/front".into(), "/back".into()]);
    app.workspace.move_project(0, 0, 1);
    let _ = app.update_native_menu(Event::Selected(Action::Project(0, "/front".into())));
    assert_eq!(
        app.workspace.directory(),
        Some(&std::path::PathBuf::from("/front"))
    );
    let _ = app.update_native_menu(Event::Selected(Action::Project(0, "/removed".into())));
    assert_eq!(
        app.workspace.directory(),
        Some(&std::path::PathBuf::from("/front"))
    );
}

#[test]
fn deployment_menu_opens_matching_settings_and_preserves_project() {
    let mut app = App::new(Rc::new(I18n::new(Language::English)));
    app.loaded = true;
    app.workspace
        .create_space("Work".into(), vec!["/front".into()]);
    let project = app.workspace.directory().cloned();
    for (action, tab) in [
        (Action::DeploymentConnections, SettingsTab::Connections),
        (Action::DeploymentProfiles, SettingsTab::Deployment),
    ] {
        let _ = app.update_native_menu(Event::Selected(action));
        assert!(if tab == SettingsTab::Connections {
            app.windows.connections.is_some()
        } else {
            app.windows.deployment.is_some()
        });
        assert!(app.windows.settings.is_none());
        assert_eq!(app.workspace.directory(), project.as_ref());
    }
}

#[tokio::test]
async fn quit_flushes_edits_before_exit_and_keeps_conflicting_drafts_open() {
    use iced::{futures::StreamExt, widget::text_editor};
    use iced_test::runtime::{Action as RuntimeAction, task};
    use std::{collections::VecDeque, sync::Arc};
    for conflict in [false, true] {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("quit.txt");
        std::fs::write(&path, "original").unwrap();
        let mut app = App::new(Rc::new(I18n::new(Language::English)));
        app.loaded = true;
        app.save_blocked = true;
        app.workspace
            .create_space("Test".into(), vec![directory.path().into()]);
        app.window_id = Some(iced::window::Id::unique());
        let _ = app.open_file(path.clone());
        let id = app.preview().id;
        let _ = app.finish_preview(id, Ok(crate::ui::code::Document::plain("original".into())));
        let _ = app.update_editor(super::super::editor::Event::Action(
            id,
            text_editor::Action::SelectAll,
        ));
        let _ = app.update_editor(super::super::editor::Event::Action(
            id,
            text_editor::Action::Edit(text_editor::Edit::Paste(Arc::new(
                "saved before quit".into(),
            ))),
        ));
        if conflict {
            std::fs::write(&path, "external").unwrap();
        }
        let quit = app.update_native_menu(Event::Selected(Action::Quit));
        assert!(app.editor.pending.is_some());
        let mut tasks = VecDeque::from([quit]);
        while let Some(next) = tasks.pop_front() {
            if let Some(mut actions) = task::into_stream(next) {
                while let Some(action) = actions.next().await {
                    if let RuntimeAction::Output(message) = action {
                        tasks.push_back(app.update(message));
                    }
                }
            }
        }
        assert_eq!(app.exit_fade_started, !conflict);
        assert_eq!(app.exit_ready, !conflict);
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            if conflict {
                "external"
            } else {
                "saved before quit"
            }
        );
        assert_eq!(app.editor.pending.is_some(), conflict);
        if conflict {
            assert_eq!(
                app.editor_buffer().unwrap().document.text,
                "saved before quit"
            );
            assert_eq!(
                app.editor_buffer().unwrap().error,
                Some(Key::EditorConflict)
            );
            let _ = app.open_settings();
            let _ = app.open_deployment_settings(false);
            for window in [
                app.windows.settings.unwrap(),
                app.windows.deployment.unwrap(),
            ] {
                let mut ui = iced_test::Simulator::with_size(
                    iced::Settings::default(),
                    (1120., 760.),
                    app.view_window(window),
                );
                assert!(
                    ui.find(app.i18n.text(Key::EditorUnsavedTitle))
                        .unwrap()
                        .visible_bounds()
                        .is_some()
                );
            }
        }
    }
}

#[test]
fn closing_empty_workspace_waits_for_save_and_fade() {
    let mut app = App::new(Rc::new(I18n::new(Language::English)));
    let id = window::Id::unique();
    app.window_id = Some(id);
    app.loaded = true;
    app.workspace.preferences.restore_project_tabs = true;
    assert!(app.workspace.directory().is_none());

    let close = Message::Window(windows::Event::Close(id));
    let _ = app.update(close.clone());
    assert!(app.saving);
    assert!(!app.exit_fade_started);

    let _ = app.update(Message::Saved(Err(crate::domain::Failure::Storage)));
    assert!(!app.exit_fade_started);
    assert_eq!(app.window_id, Some(id));

    let _ = app.update(close.clone());
    let _ = app.update(Message::Saved(Ok(())));
    assert!(app.exit_fade_started);
    let _ = app.update(close);
    assert!(app.exit_fade_started);
    assert!(!app.exit_ready);

    let _ = app.update(Message::Window(windows::Event::ExitFadeFinished));
    assert!(app.exit_ready);
}
