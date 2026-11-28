//! Resume multiline parsing from the last unchanged block outside the render thread.

use super::{BLOCK, Settings, Span};
use crate::ui::theme::iced;
use crate::ui::{
    code::{PALETTE, SYNTAXES, syntax_for},
    theme::syntax,
};
use iced::{
    advanced::graphics::text::cosmic_text::LineIter,
    futures::{SinkExt, Stream},
};
use std::{path::Path, sync::Arc};
use two_face::re_exports::syntect::{
    easy::HighlightLines, highlighting::HighlightState, parsing::ParseState,
};

#[derive(Debug)]
pub(super) struct Block {
    pub(super) lines: Vec<Vec<Span>>,
    end: usize,
    state: (HighlightState, ParseState),
}

#[derive(Debug, Clone)]
pub(crate) struct Prepared {
    text: Arc<String>,
    language: String,
    pub(super) blocks: Vec<Arc<Block>>,
}

impl Prepared {
    pub(crate) fn matches(&self, text: &Arc<String>, path: Option<&Path>) -> bool {
        Arc::ptr_eq(&self.text, text) && path.map(syntax_for).unwrap_or_default() == self.language
    }

    pub(super) fn reusable(&self, text: &Arc<String>, language: &str) -> usize {
        if self.language != language {
            return 0;
        }
        if Arc::ptr_eq(&self.text, text) {
            return self.blocks.len();
        }
        let mut start = 0;
        self.blocks
            .iter()
            .take_while(|block| {
                let end = block.end;
                // Reparse the final block when text grows. CR/LF pairs can cross a byte boundary.
                let boundary = |bytes: &[u8]| {
                    matches!(
                        (bytes.get(end - 1), bytes.get(end)),
                        (Some(b'\n'), Some(b'\r')) | (Some(b'\r'), Some(b'\n'))
                    )
                };
                let same = end < self.text.len()
                    && text.as_bytes().get(start..end) == self.text.as_bytes().get(start..end)
                    && boundary(text.as_bytes()) == boundary(self.text.as_bytes());
                start = end;
                same
            })
            .count()
    }

    fn resume(settings: &Settings) -> Self {
        let blocks = settings
            .cache
            .prepared
            .iter()
            .flat_map(|p| &p.blocks)
            .take(settings.reusable())
            .cloned()
            .collect();
        Self {
            text: Arc::clone(&settings.text),
            language: settings.language.clone(),
            blocks,
        }
    }

    fn offset(&self) -> usize {
        self.blocks.last().map_or(0, |block| block.end)
    }

    fn extend(&mut self, target: usize) {
        let Some(grammar) = SYNTAXES.find_syntax_by_token(&self.language) else {
            return;
        };
        let start = self.offset();
        let source = &self.text[start..];
        let mut iter = LineIter::new(source);
        while self.blocks.len() * BLOCK < target && self.offset() < self.text.len() {
            let mut parser = if let Some(block) = self.blocks.last() {
                HighlightLines::from_state(&PALETTE, block.state.0.clone(), block.state.1.clone())
            } else {
                HighlightLines::new(grammar, &PALETTE)
            };
            let mut lines = Vec::new();
            let mut end = start;
            for (range, ending) in iter.by_ref().take(BLOCK) {
                end = start + range.end + ending.as_str().len();
                lines.push(highlight(&mut parser, &source[range]));
            }
            self.blocks.push(Arc::new(Block {
                lines,
                end,
                state: parser.state(),
            }));
        }
    }
}

fn highlight(parser: &mut HighlightLines<'_>, line: &str) -> Vec<Span> {
    let mut spans: Vec<Span> = Vec::new();
    if line.len() > 16 * 1024 {
        return spans;
    }
    let mut start = 0;
    for (style, part) in parser.highlight_line(line, &SYNTAXES).unwrap_or_default() {
        let end = start + part.len();
        let color = syntax::tone(style.foreground);
        if color != syntax::TEXT {
            if let Some((previous, tone)) = spans.last_mut()
                && *tone == color
                && previous.end == start
            {
                previous.end = end;
            } else {
                spans.push((start..end, color));
            }
        }
        start = end;
    }
    spans
}

pub(super) fn stream(
    (tab, settings): &(u64, Settings),
) -> impl Stream<Item = (u64, Arc<Prepared>)> + use<> {
    let tab = *tab;
    let mut demand = settings.cache.demand.subscribe();
    let mut prepared = Arc::new(Prepared::resume(settings));
    iced::stream::channel(1, async move |mut output| {
        // Limit abandoned work when the user types or switches tabs during a cold grammar load.
        static PARSER: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(1);
        while prepared.offset() < prepared.text.len() {
            let done = prepared.blocks.len() * BLOCK;
            let target = *demand.borrow_and_update();
            if done >= target {
                if demand.changed().await.is_err() {
                    break;
                }
                continue;
            }
            let Ok(permit) = PARSER.acquire().await else {
                break;
            };
            let Ok(next) = tokio::task::spawn_blocking(move || {
                let _permit = permit;
                Arc::make_mut(&mut prepared).extend(target.min(if done == 0 {
                    BLOCK
                } else {
                    done + 4 * BLOCK
                }));
                prepared
            })
            .await
            else {
                break;
            };
            prepared = next;
            if prepared.blocks.len() * BLOCK == done {
                break;
            }
            // A jump near EOF can require many checkpoints before any visible line is ready.
            let ready = prepared.blocks.len() * BLOCK;
            let first_screen = done == 0 && target <= 4 * BLOCK;
            if ready < target && prepared.offset() < prepared.text.len() && !first_screen {
                continue;
            }
            if output.send((tab, Arc::clone(&prepared))).await.is_err() {
                break;
            }
        }
    })
}
