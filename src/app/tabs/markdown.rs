//! Prepare the active Markdown document and route its links.

use super::{App, Preview, Source, State};
use crate::{app::Message, domain::external::Request};
use iced::{
    Task,
    widget::{markdown, operation},
};
use std::sync::{Arc, Weak};
use url::{ParseError, Url};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum MarkdownViewMode {
    #[default]
    Source,
    Split,
    Preview,
}

impl Preview {
    pub(crate) fn markdown_view_mode(&self) -> Option<MarkdownViewMode> {
        use crate::domain::language::Language;

        let path = self.path.as_deref()?;
        let document = self.document.as_ref()?;
        if document.comparison.is_some()
            || self.pending_comparison.is_some()
            || (self.editor.is_none() && (self.loading || self.error.is_some()))
        {
            return None;
        }
        let plain_file = match self.source.as_ref()? {
            Source::Working { compare, .. } => !*compare,
            Source::Revision { .. } => true,
            Source::Commit { .. } => document.page.is_some(),
            Source::Remote {
                previous,
                local_root,
                ..
            } => previous.is_none() && local_root.is_none(),
        };
        (plain_file && Language::for_path(path) == Some(Language::Markdown))
            .then_some(self.markdown_mode)
    }

    pub(in crate::app) fn reading_markdown(&self) -> bool {
        self.markdown_mode == MarkdownViewMode::Preview && self.markdown_view_mode().is_some()
    }

    pub(crate) fn markdown_link(&self, link: String) -> Message {
        if let Some((_, file)) = self.working_paths()
            && !link.starts_with(['/', '\\', '#', '?'])
            && !link.is_empty()
            && matches!(Url::parse(&link), Err(ParseError::RelativeUrlWithoutBase))
            && let Ok(base) = Url::from_file_path(file)
            && let Ok(destination) = base.join(&link)
            && let Ok(path) = destination.to_file_path()
        {
            return Message::External(Request::File { path });
        }
        Message::OpenUrl(link)
    }
}

impl App {
    pub(in crate::app) fn set_markdown_view_mode(
        &mut self,
        id: u64,
        mode: MarkdownViewMode,
    ) -> Task<Message> {
        let tab = &mut self.tabs.files[self.tabs.active];
        if tab.id != id
            || tab
                .markdown_view_mode()
                .is_none_or(|current| current == mode)
        {
            return Task::none();
        }
        tab.markdown_mode = mode;
        self.editor.menu = None;
        self.editor.rename = None;
        self.editor.suggestions = None;
        self.editor.definitions.clear();
        if mode == MarkdownViewMode::Preview {
            Task::none()
        } else if tab.editor.is_some() {
            operation::focus(format!("text-editor-{id}"))
        } else {
            self.restore_preview_scroll()
        }
    }
}

#[derive(Default)]
pub(crate) struct Cache {
    source: Option<Snapshot>,
    pub items: Vec<markdown::Item>,
}

enum Snapshot {
    Editor(Weak<String>),
    Page([u8; 32]),
}

impl State {
    pub(crate) fn update_markdown(&self, cache: &mut Cache) {
        let Some(tab) = self.files.get(self.active).filter(|tab| {
            tab.markdown_mode != MarkdownViewMode::Source && tab.markdown_view_mode().is_some()
        }) else {
            if cache.source.is_some() {
                *cache = Cache::default();
            }
            return;
        };

        let (source, text) = if let Some(buffer) = &tab.editor {
            let snapshot = Arc::downgrade(&buffer.document.text);
            if matches!(&cache.source, Some(Snapshot::Editor(previous)) if previous.ptr_eq(&snapshot))
            {
                return;
            }
            (Snapshot::Editor(snapshot), buffer.document.text.as_str())
        } else if let Some(document) = &tab.document {
            if matches!(cache.source, Some(Snapshot::Page(key)) if key == document.fingerprint) {
                return;
            }
            (Snapshot::Page(document.fingerprint), document.text.as_str())
        } else {
            return;
        };
        cache.items = markdown::parse(text).collect();
        cache.source = Some(source);
    }
}
