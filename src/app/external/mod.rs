//! Route requests that arrive from outside the application.
//!
//! A file request opens the file inside the project that owns it. When no
//! project owns the file, the file opens on its own. A project request
//! switches to the existing project with the given directory.

use super::{App, Message, Navigation};
use crate::{domain::external::Request, i18n::Key, infrastructure::external};
use iced::futures::SinkExt;
use iced::{Subscription, Task, window};
use serde_json::{Value, json};
use std::path::PathBuf;

/// External requests that wait for the workspace to load.
#[derive(Debug, Default)]
pub(crate) struct State {
    pending: Vec<Request>,
}

/// Receive requests from the operating system, later instances, and drops.
pub(crate) fn subscription() -> Subscription<Message> {
    Subscription::batch([
        Subscription::run(|| {
            iced::stream::channel::<Message>(16, async move |mut output| {
                external::serve();
                let Some(mut receiver) = external::take_receiver() else {
                    return;
                };
                while let Some(event) = receiver.recv().await {
                    let message = match event {
                        external::Event::Open(request) => Message::External(request),
                        external::Event::Inspect(include_text, reply) => {
                            Message::InspectWindow(include_text, reply)
                        }
                    };
                    if output.send(message).await.is_err() {
                        break;
                    }
                }
            })
        }),
        iced::event::listen_with(|event, _status, _id| drop_message(event)),
    ])
}

/// Project current window state without changing focus, tabs, or saved data.
pub(super) fn window_snapshot(app: &App, include_text: bool) -> Value {
    const TAB_LIMIT: usize = 100;
    if !app.loaded {
        return json!({ "state": "loading" });
    }
    let active = app.preview();
    let navigation = match app.navigation {
        Navigation::Projects => "projects",
        Navigation::Files => "files",
        Navigation::Settings => "settings",
    };
    let space = app
        .workspace
        .selected_space
        .and_then(|index| app.workspace.spaces.get(index));
    let project = app.workspace.project().map(|project| {
        json!({ "id": project.id(), "name": project.name, "directory": project.directory.display().to_string() })
    });
    let open_tabs = app.tabs.files.iter().filter(|tab| tab.is_open());
    let tab_count = open_tabs.clone().count();
    let tabs: Vec<_> = open_tabs.take(TAB_LIMIT).map(tab_snapshot).collect();
    let active_tab = active.is_open().then(|| tab_snapshot(active));
    let mut snapshot = json!({
        "state": "ready",
        "navigation": navigation,
        "space": space.map(|space| &space.name),
        "project": project,
        "activeTab": active_tab,
        "tabs": tabs,
        "tabCount": tab_count,
    });
    if include_text {
        let selection = if let Some(buffer) = app.editor_buffer() {
            buffer.content.selection()
        } else {
            active
                .document
                .as_ref()
                .and_then(|document| active.selection.copy(document))
        };
        let draft = app.editor_buffer().filter(|buffer| buffer.document.dirty());
        snapshot["selection"] = json!(selection.as_deref().map(text_snapshot));
        snapshot["draft"] = json!(draft.map(|buffer| text_snapshot(&buffer.document.text)));
    }
    snapshot
}

fn tab_snapshot(tab: &super::tabs::Preview) -> Value {
    json!({
        "id": tab.id,
        "title": tab.title,
        "path": tab.path.as_ref().map(|path| path.display().to_string()),
        "revision": tab.revision,
        "remote": tab.remote,
        "loading": tab.loading,
        "dirty": tab.editor.as_ref().is_some_and(|buffer| buffer.document.dirty()),
    })
}

fn text_snapshot(text: &str) -> Value {
    const LIMIT: usize = 32 * 1024;
    let mut end = text.len().min(LIMIT);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    json!({ "text": &text[..end], "truncated": end < text.len() })
}

/// Turn a dropped-file event into an external file request.
fn drop_message(event: iced::Event) -> Option<Message> {
    match event {
        iced::Event::Window(iced::window::Event::FileDropped(path)) => {
            Some(Message::External(Request::File { path }))
        }
        _ => None,
    }
}

/// Handle one external request, queueing it before the workspace loads.
pub(crate) fn open(app: &mut App, request: Request) -> Task<Message> {
    if !app.loaded {
        app.external.pending.push(request);
        return Task::none();
    }
    let focus = app
        .window_id
        .map(window::gain_focus)
        .unwrap_or_else(Task::none);
    match request {
        Request::Focus => focus,
        Request::File { path } => Task::batch([focus, open_file(app, path)]),
        Request::Project { directory } => Task::batch([focus, select_project(app, directory)]),
    }
}

/// Handle the requests that arrived before the workspace loaded.
pub(crate) fn flush(app: &mut App) -> Task<Message> {
    let pending = std::mem::take(&mut app.external.pending);
    let mut tasks = Vec::new();
    for request in pending {
        tasks.push(open(app, request));
    }
    Task::batch(tasks)
}

/// Open a file inside its project, or on its own when no project owns it.
fn open_file(app: &mut App, path: PathBuf) -> Task<Message> {
    let Ok(path) = std::fs::canonicalize(&path) else {
        app.notifications.error(Key::ExternalFileMissing, None);
        return Task::none();
    };
    let Some((space, project)) = app.workspace.project_for_path(&path) else {
        return app.open_standalone_file(path);
    };
    if app.workspace.selected_space == Some(space)
        && app.workspace.selected_project == Some(project)
    {
        return app.open_file(path);
    }
    app.navigation = Navigation::Projects;
    if !app.workspace.select_project(space, project) {
        return app.open_standalone_file(path);
    }
    Task::batch([app.load_directory(), app.save(), app.open_file(path)])
}

/// Switch to the existing project with the given directory.
fn select_project(app: &mut App, directory: PathBuf) -> Task<Message> {
    let Ok(directory) = std::fs::canonicalize(&directory) else {
        app.notifications.error(Key::ExternalProjectMissing, None);
        return Task::none();
    };
    match app.workspace.project_for_path(&directory) {
        Some((space, project)) => app.update(Message::SelectProject(space, project)),
        None => {
            app.notifications.error(Key::ExternalProjectMissing, None);
            Task::none()
        }
    }
}

#[cfg(test)]
mod tests;
