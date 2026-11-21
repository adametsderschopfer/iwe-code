//! Paint cached colors and request only the lines reached by the editor viewport.

#[cfg(test)]
mod tests;
mod worker;

use crate::ui::theme::Tone;
use iced::Subscription;
use std::{
    hash::{Hash, Hasher},
    ops::Range,
    sync::Arc,
};
use tokio::sync::watch;
pub(crate) use worker::Prepared;

const BLOCK: usize = 16;
type Span = (Range<usize>, Tone);

#[derive(Debug, Clone)]
pub(crate) struct Cache {
    prepared: Option<Arc<Prepared>>,
    demand: watch::Sender<usize>,
}

impl Default for Cache {
    fn default() -> Self {
        Self {
            prepared: None,
            demand: watch::channel(0).0,
        }
    }
}

impl Cache {
    pub(crate) fn accept(
        &mut self,
        prepared: Arc<Prepared>,
        text: &Arc<String>,
        path: Option<&std::path::Path>,
    ) {
        if prepared.matches(text, path)
            && self.prepared.as_ref().is_none_or(|previous| {
                !previous.matches(text, path) || previous.blocks.len() < prepared.blocks.len()
            })
        {
            self.prepared = Some(prepared);
        }
    }

    pub(crate) fn settings(&self, text: &Arc<String>, language: String) -> Settings {
        Settings {
            text: Arc::clone(text),
            language,
            cache: self.clone(),
        }
    }
}

#[derive(Clone)]
pub(crate) struct Settings {
    text: Arc<String>,
    language: String,
    cache: Cache,
}

impl PartialEq for Settings {
    fn eq(&self, rhs: &Self) -> bool {
        Arc::ptr_eq(&self.text, &rhs.text)
            && self.language == rhs.language
            && self.cache.demand.same_channel(&rhs.cache.demand)
            && self.cache.prepared.as_ref().map(Arc::as_ptr)
                == rhs.cache.prepared.as_ref().map(Arc::as_ptr)
    }
}

// Completed blocks update the renderer without restarting its worker.
impl Hash for Settings {
    fn hash<H: Hasher>(&self, state: &mut H) {
        Arc::as_ptr(&self.text).hash(state);
        self.language.hash(state);
    }
}

impl Settings {
    pub(crate) fn subscription(self, tab: u64) -> Subscription<(u64, Arc<Prepared>)> {
        Subscription::run_with((tab, self), worker::stream)
    }

    fn reusable(&self) -> usize {
        self.cache
            .prepared
            .as_ref()
            .map_or(0, |p| p.reusable(&self.text, &self.language))
    }
}

pub(super) struct Highlighter {
    settings: Settings,
    blocks: usize,
    line: usize,
    requested: usize,
}

impl iced::advanced::text::Highlighter for Highlighter {
    type Settings = Settings;
    type Highlight = Tone;
    type Iterator<'a> = std::iter::Cloned<std::slice::Iter<'a, Span>>;

    fn new(settings: &Settings) -> Self {
        Self {
            settings: settings.clone(),
            blocks: settings.reusable(),
            line: 0,
            requested: 0,
        }
    }

    fn update(&mut self, settings: &Settings) {
        let blocks = settings.reusable();
        let common = match (&self.settings.cache.prepared, &settings.cache.prepared) {
            (Some(old), Some(new))
                if self
                    .settings
                    .cache
                    .demand
                    .same_channel(&settings.cache.demand) =>
            {
                let count = self.blocks.min(blocks);
                if count == 0 || Arc::ptr_eq(&old.blocks[count - 1], &new.blocks[count - 1]) {
                    count
                } else {
                    old.blocks
                        .iter()
                        .zip(&new.blocks)
                        .take(count)
                        .take_while(|(a, b)| Arc::ptr_eq(a, b))
                        .count()
                }
            }
            _ => 0,
        };
        self.line = self.line.min(common * BLOCK);
        self.requested = 0;
        self.blocks = blocks;
        self.settings = settings.clone();
    }

    fn change_line(&mut self, line: usize) {
        self.line = self.line.min(line);
    }
    fn current_line(&self) -> usize {
        self.line
    }

    fn highlight_line(&mut self, _: &str) -> Self::Iterator<'_> {
        let line = self.line;
        self.line += 1;
        let block = line / BLOCK;
        if block >= self.blocks && line >= self.requested {
            self.requested = (block + 1) * BLOCK;
            self.settings.cache.demand.send_replace(self.requested);
        }
        self.settings
            .cache
            .prepared
            .as_ref()
            .filter(|_| block < self.blocks)
            .and_then(|p| p.blocks[block].lines.get(line % BLOCK))
            .map_or(&[][..], |spans| spans.as_slice())
            .iter()
            .cloned()
    }
}
