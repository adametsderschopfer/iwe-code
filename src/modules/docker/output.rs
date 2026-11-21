//! Read Docker output in a dialog that owns its request and pagination.
use super::{Action, Error, Event, Output, State, adapter, model};
use iced::{Task, task::Handle, widget::operation};
use std::{path::Path, sync::Arc};

#[derive(Clone)]
pub(super) struct Request {
    pub action: Action,
    pub name: String,
    args: Vec<String>,
}

pub(super) struct Dialog {
    pub request: Request,
    pub content: Content,
}

pub(super) enum Content {
    Loading { _task: Handle },
    Ready { output: Arc<Output>, page: usize },
    Failed(Error),
}

pub(super) const SCROLL_ID: &str = "docker-output-scroll";

impl State {
    pub(super) fn open_output(
        &mut self,
        action: Action,
        target: Option<&str>,
        root: &Path,
    ) -> Task<Event> {
        let Some(item) = self.find(target) else {
            return Task::none();
        };
        let Some(args) = model::arguments(self.kind, action, Some(item), "", "") else {
            return Task::none();
        };
        self.read_output(
            Request {
                action,
                name: item.name.clone(),
                args,
            },
            root,
        )
    }

    fn read_output(&mut self, request: Request, root: &Path) -> Task<Event> {
        let Some(context) = self.context.clone() else {
            return Task::none();
        };
        self.request += 1;
        let id = self.request;
        let root = root.to_owned();
        let args = request.args.clone();
        let logs = request.action == Action::Logs;
        let (task, handle) = Task::perform(
            async move {
                adapter::run(Some(&context), &args, &root, false, logs)
                    .await
                    .map(|text| Arc::new(Output::new(text)))
            },
            move |result| Event::OutputLoaded(id, result),
        )
        .abortable();
        self.output = Some(Dialog {
            request,
            content: Content::Loading {
                _task: handle.abort_on_drop(),
            },
        });
        task
    }

    pub(super) fn update_output(&mut self, event: Event, root: &Path) -> Task<Event> {
        let Some(dialog) = &mut self.output else {
            return Task::none();
        };
        match event {
            Event::Dismiss => {
                self.output = None;
            }
            Event::Refresh if !matches!(dialog.content, Content::Loading { .. }) => {
                let request = dialog.request.clone();
                return self.read_output(request, root);
            }
            Event::OutputLoaded(id, result) if id == self.request => {
                dialog.content = match result {
                    Ok(output) => Content::Ready { output, page: 0 },
                    Err(error) => Content::Failed(error),
                };
            }
            Event::OutputPage(next) => {
                if let Content::Ready { output, page } = &mut dialog.content {
                    let new_page = if next {
                        (*page + 1).min(output.offsets.len() - 1)
                    } else {
                        page.saturating_sub(1)
                    };
                    if new_page != *page {
                        *page = new_page;
                        return operation::scroll_to(
                            SCROLL_ID,
                            operation::AbsoluteOffset { x: 0., y: 0. },
                        );
                    }
                }
            }
            _ => {}
        }
        Task::none()
    }
}
