//! Prepare the selected context and coordinate the desktop handoff.

use super::{App, Message};
use crate::{
    domain::explain::{Agent, Target},
    i18n::{FluentArgs, I18n, Key},
    infrastructure::assistant,
};
use iced::{Point, Task};
use std::path::PathBuf;

#[derive(Default)]
pub(crate) struct State {
    pub menu: Option<(Target, Point)>,
    pub setup: Option<Request>,
    request: u64,
}

pub(crate) struct Request {
    agent: Agent,
    prompt: String,
    directory: Option<PathBuf>,
    pub busy: bool,
}

#[derive(Debug, Clone)]
pub(crate) enum Event {
    Menu(Target, Point),
    CloseMenu,
    Choose(Target, Agent),
    Install,
    Locate,
    Located(u64, Option<PathBuf>),
    Retry,
    Launched(u64, Option<PathBuf>, bool),
    Cancel,
}

impl App {
    pub(super) fn update_explain(&mut self, event: Event) -> Task<Message> {
        match event {
            Event::Menu(target, point) => self.explain.menu = Some((target, point)),
            Event::CloseMenu => self.explain.menu = None,
            Event::Choose(target, agent) if self.explain.setup.is_none() => {
                self.explain.menu = None;
                self.files.menu = None;
                self.project_menu = None;
                self.space_menu = None;
                self.modules.git.git_actions.close_menu();
                self.modules.pipelines.jobs.overview.popup = None;
                let Some(request) = self.explanation(target, agent) else {
                    return Task::none();
                };
                self.explain.setup = Some(request);
                return self.launch_explanation(None);
            }
            Event::Install => {
                if self.explain.setup.is_some() {
                    return Task::done(Message::OpenUrl("https://chatgpt.com/codex".into()));
                }
            }
            Event::Locate => {
                if let Some(request) = self.explain.setup.as_mut().filter(|request| !request.busy) {
                    request.busy = true;
                    let id = self.explain.request;
                    return Task::perform(
                        pick_application(self.i18n.text(Key::ExplainLocate)),
                        move |path| Message::Explain(Event::Located(id, path)),
                    );
                }
            }
            Event::Located(id, path) if id == self.explain.request => {
                if let Some(request) = self.explain.setup.as_mut() {
                    request.busy = false;
                    if path.is_some() {
                        return self.launch_explanation(path);
                    }
                }
            }
            Event::Retry => return self.launch_explanation(None),
            Event::Launched(id, application, opened) if id == self.explain.request => {
                if let Some(request) = self.explain.setup.as_mut() {
                    request.busy = false;
                    if opened {
                        let prompt = request.prompt.clone();
                        let agent = request.agent;
                        self.explain.setup = None;
                        let copy = if cfg!(target_os = "macos") {
                            Task::none()
                        } else {
                            self.notifications.success(Key::ExplainPromptCopied, None);
                            iced::clipboard::write(prompt)
                        };
                        if let Some(path) = application {
                            self.workspace
                                .preferences
                                .assistant_paths
                                .insert(agent, path);
                            return Task::batch([copy, self.save()]);
                        }
                        return copy;
                    }
                }
            }
            Event::Cancel => {
                self.explain.request += 1;
                self.explain.setup = None;
            }
            Event::Choose(..) | Event::Located(..) | Event::Launched(..) => {}
        }
        Task::none()
    }

    fn launch_explanation(&mut self, application: Option<PathBuf>) -> Task<Message> {
        let Some(request) = self.explain.setup.as_mut().filter(|request| !request.busy) else {
            return Task::none();
        };
        request.busy = true;
        self.explain.request += 1;
        let id = self.explain.request;
        let path = application.clone().or_else(|| {
            self.workspace
                .preferences
                .assistant_paths
                .get(&request.agent)
                .cloned()
        });
        Task::perform(
            assistant::launch(path, request.directory.clone(), request.prompt.clone()),
            move |opened| Message::Explain(Event::Launched(id, application.clone(), opened)),
        )
    }

    fn explanation(&self, target: Target, agent: Agent) -> Option<Request> {
        let mut directory = self.workspace.directory().cloned();
        let (key, values): (_, Vec<(&str, String)>) = match target {
            Target::File(path) => {
                directory = directory
                    .filter(|root| path.starts_with(root))
                    .or_else(|| path.parent().map(std::path::Path::to_owned));
                (Key::ExplainFile, vec![("path", path.display().to_string())])
            }
            Target::Commit(commit) => {
                directory = Some(commit.root.clone());
                (Key::ExplainCommit, vec![("commit", commit.id)])
            }
            Target::Branch(branch) => {
                directory = Some(branch.root.clone());
                (Key::ExplainBranch, vec![("branch", branch.reference)])
            }
            Target::Project(name, path) => {
                directory = Some(path);
                (Key::ExplainProject, vec![("name", name)])
            }
            Target::Space(index) => {
                let space = self.workspace.spaces.get(index)?;
                directory = space
                    .projects
                    .first()
                    .map(|project| project.directory.clone());
                let projects = space
                    .projects
                    .iter()
                    .map(|project| format!("- {}: {}", project.name, project.directory.display()))
                    .collect::<Vec<_>>()
                    .join("\n");
                (
                    Key::ExplainSpace,
                    vec![("name", space.name.clone()), ("projects", projects)],
                )
            }
            Target::Pipeline(run) => {
                directory = self
                    .active_repository()
                    .map(|repo| repo.root.clone())
                    .or(directory);
                let details = &self.modules.pipelines.jobs;
                let overview = details
                    .overview
                    .entries
                    .get(&run.id)
                    .and_then(|entry| entry.details.as_ref());
                let selected = details
                    .selected
                    .as_ref()
                    .filter(|selected| selected.id == run.id)
                    .and(details.details.as_ref());
                let jobs = selected
                    .or(overview)
                    .into_iter()
                    .flat_map(|details| &details.stages)
                    .flat_map(|stage| &stage.jobs)
                    .map(|job| format!("- {}: {}", job.name, job.status))
                    .collect::<Vec<_>>()
                    .join("\n");
                (
                    Key::ExplainPipeline,
                    vec![
                        ("id", run.id.to_string()),
                        ("title", run.title),
                        ("branch", run.branch),
                        ("commit", run.commit.unwrap_or_default()),
                        ("status", run.status),
                        ("url", run.url),
                        ("jobs", jobs),
                    ],
                )
            }
            Target::Job(pipeline, job) => {
                directory = self
                    .active_repository()
                    .map(|repo| repo.root.clone())
                    .or(directory);
                (
                    Key::ExplainJob,
                    vec![
                        ("pipeline", pipeline.to_string()),
                        ("id", job.id.to_string()),
                        ("name", job.name),
                        ("status", job.status),
                        ("url", job.url),
                    ],
                )
            }
        };
        let prompt = prompt(&self.i18n, key, values, directory.as_deref());
        Some(Request {
            agent,
            prompt,
            directory,
            busy: false,
        })
    }
}

fn prompt(
    i18n: &I18n,
    key: Key,
    values: Vec<(&str, String)>,
    directory: Option<&std::path::Path>,
) -> String {
    let mut args = FluentArgs::new();
    for (name, value) in values {
        args.set(name, value);
    }
    let mut prompt = i18n.format(key, &args);
    if let Some(directory) = directory {
        let mut location = FluentArgs::new();
        location.set("path", directory.display().to_string());
        prompt.push_str("\n\n");
        prompt.push_str(&i18n.format(Key::ExplainDirectory, &location));
    }
    prompt.push_str("\n\n");
    prompt.push_str(&i18n.text(Key::ExplainReadOnly));
    prompt
}

async fn pick_application(title: String) -> Option<PathBuf> {
    // macOS presents .app bundles as files in the native dialog.
    let dialog = rfd::AsyncFileDialog::new().set_title(title);
    #[cfg(target_os = "macos")]
    let dialog = dialog.set_directory("/Applications");
    #[cfg(target_os = "windows")]
    let dialog = dialog.add_filter("EXE", &["exe"]);
    dialog
        .pick_file()
        .await
        .map(|handle| handle.path().to_owned())
}

#[cfg(test)]
mod tests;
