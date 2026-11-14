use super::*;
use crate::i18n::I18n;
use std::{path::PathBuf, rc::Rc};

fn app() -> App {
    App::new(Rc::new(I18n::new(crate::i18n::Language::English)))
}

fn project_directory() -> (tempfile::TempDir, PathBuf) {
    let directory = tempfile::tempdir().expect("tempdir");
    let canonical = directory.path().canonicalize().expect("canonical");
    (directory, canonical)
}

fn write_project_file(directory: &std::path::Path, name: &str) -> PathBuf {
    let path = directory.join(name);
    std::fs::write(&path, "content").expect("write");
    path
}

#[test]
fn requests_wait_in_the_queue_before_the_workspace_loads() {
    let mut app = app();
    let (_guard, directory) = project_directory();
    let path = write_project_file(&directory, "notes.md");
    let _ = open(&mut app, Request::File { path });
    assert_eq!(app.external.pending.len(), 1);
    assert!(app.tabs.files.iter().all(|preview| preview.path.is_none()));
}

#[test]
fn flush_opens_queued_requests_after_the_workspace_loads() {
    let mut app = app();
    let (_guard, directory) = project_directory();
    app.workspace
        .create_space("Work".into(), vec![directory.clone()]);
    let path = write_project_file(&directory, "notes.md");
    let _ = open(&mut app, Request::File { path: path.clone() });
    assert_eq!(app.external.pending.len(), 1);
    app.loaded = true;
    let _ = flush(&mut app);
    assert!(app.external.pending.is_empty());
    assert_eq!(app.workspace.selected_space, Some(0));
    assert_eq!(app.workspace.selected_project, Some(0));
    assert!(
        app.tabs
            .files
            .iter()
            .any(|preview| preview.path.as_ref() == Some(&path))
    );
}

#[test]
fn a_file_inside_a_project_selects_that_project() {
    let mut app = app();
    let (_guard, directory) = project_directory();
    app.workspace
        .create_space("Work".into(), vec![directory.clone()]);
    let other = tempfile::tempdir().expect("tempdir");
    app.workspace.create_space(
        "Other".into(),
        vec![other.path().canonicalize().expect("canonical")],
    );
    app.workspace.select_project(1, 0);
    app.loaded = true;
    let path = write_project_file(&directory, "notes.md");
    let _ = open(&mut app, Request::File { path: path.clone() });
    assert_eq!(app.workspace.selected_space, Some(0));
    assert_eq!(app.workspace.selected_project, Some(0));
    // Opening the file switches to the file browser, as a normal click does.
    assert_eq!(app.navigation, Navigation::Files);
    assert!(
        app.tabs
            .files
            .iter()
            .any(|preview| preview.path.as_ref() == Some(&path))
    );
}

#[test]
fn a_file_outside_every_project_opens_without_a_project() {
    let mut app = app();
    let (_guard, directory) = project_directory();
    app.workspace
        .create_space("Work".into(), vec![directory.clone()]);
    app.workspace.select_project(0, 0);
    app.loaded = true;
    let folder = tempfile::tempdir().expect("tempdir");
    let path = write_project_file(folder.path(), "standalone.md");
    let canonical = path.canonicalize().expect("canonical");
    let _ = open(&mut app, Request::File { path: canonical });
    assert_eq!(app.workspace.selected_space, Some(0));
    assert_eq!(app.workspace.selected_project, Some(0));
    assert_eq!(app.navigation, Navigation::Projects);
    let preview = app
        .tabs
        .files
        .iter()
        .find(|preview| preview.title == "standalone.md")
        .expect("standalone tab");
    assert!(preview.path.as_ref().is_some_and(|opened| {
        opened
            .file_name()
            .is_some_and(|name| name == "standalone.md")
    }));
}

#[test]
fn a_missing_file_notifies_without_opening_a_tab() {
    let mut app = app();
    app.loaded = true;
    let _ = open(
        &mut app,
        Request::File {
            path: PathBuf::from("/missing/notes.md"),
        },
    );
    assert!(
        app.notifications
            .items
            .iter()
            .any(|notification| notification.message == Key::ExternalFileMissing)
    );
    assert!(app.tabs.files.iter().all(|preview| preview.path.is_none()));
}

#[test]
fn a_project_link_selects_the_existing_project() {
    let mut app = app();
    let (_guard, directory) = project_directory();
    app.workspace
        .create_space("Work".into(), vec![directory.clone()]);
    app.loaded = true;
    let _ = open(
        &mut app,
        Request::Project {
            directory: directory.clone(),
        },
    );
    assert_eq!(app.workspace.selected_space, Some(0));
    assert_eq!(app.workspace.selected_project, Some(0));
    assert_eq!(app.navigation, Navigation::Projects);
}

#[test]
fn a_project_link_notifies_when_no_space_holds_it() {
    let mut app = app();
    app.loaded = true;
    let _ = open(
        &mut app,
        Request::Project {
            directory: PathBuf::from("/missing/project"),
        },
    );
    assert!(
        app.notifications
            .items
            .iter()
            .any(|notification| notification.message == Key::ExternalProjectMissing)
    );
    assert!(app.workspace.selected_space.is_none());
}

#[test]
fn a_file_in_the_selected_project_opens_without_resselection() {
    let mut app = app();
    let (_guard, directory) = project_directory();
    app.workspace
        .create_space("Work".into(), vec![directory.clone()]);
    app.workspace.select_project(0, 0);
    app.loaded = true;
    let path = write_project_file(&directory, "notes.md");
    let _ = open(&mut app, Request::File { path: path.clone() });
    assert_eq!(app.workspace.selected_space, Some(0));
    assert_eq!(app.workspace.selected_project, Some(0));
    assert!(
        app.tabs
            .files
            .iter()
            .any(|preview| preview.path.as_ref() == Some(&path))
    );
}

#[test]
fn dropped_files_route_like_file_requests() {
    let drop = iced::Event::Window(iced::window::Event::FileDropped(PathBuf::from(
        "/notes/ideas.md",
    )));
    assert!(matches!(
        drop_message(drop),
        Some(Message::External(Request::File { path })) if path == PathBuf::from("/notes/ideas.md")
    ));
    assert!(drop_message(iced::Event::Window(iced::window::Event::Focused)).is_none());
}
