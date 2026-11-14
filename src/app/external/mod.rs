//! Route requests that arrive from outside the application.
//!
//! A file request opens the file inside the project that owns it. When no
//! project owns the file, the file opens on its own. A project request
//! switches to the existing project with the given directory.

use super::{App, Message, Navigation};
use crate::{domain::external::Request, i18n::Key, infrastructure::external};
use iced::futures::SinkExt;
use iced::{Subscription, Task, window};
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
                while let Some(request) = receiver.recv().await {
                    let _ = output.send(Message::External(request)).await;
                }
            })
        }),
        iced::event::listen_with(|event, _status, _id| drop_message(event)),
    ])
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
