//! Track editable text and bounded undo history independently of the interface.

use std::{collections::VecDeque, sync::Arc};

pub(crate) const LIMIT: usize = 2 * 1024 * 1024;
const HISTORY_LIMIT: usize = 16 * 1024 * 1024;

#[derive(Debug, Clone)]
pub(crate) struct Document {
    pub text: Arc<String>,
    pub saved: Arc<String>,
    undo: VecDeque<Arc<String>>,
    redo: Vec<Arc<String>>,
}

impl Document {
    pub(crate) fn new(text: String) -> Self {
        let text = Arc::new(text);
        Self {
            saved: text.clone(),
            text,
            undo: VecDeque::new(),
            redo: Vec::new(),
        }
    }

    pub(crate) fn dirty(&self) -> bool {
        self.text != self.saved
    }

    pub(crate) fn edit(&mut self, text: String) -> bool {
        if text.len() > LIMIT || text == *self.text {
            return false;
        }
        self.undo
            .push_back(std::mem::replace(&mut self.text, Arc::new(text)));
        self.redo.clear();
        let mut bytes: usize = self.undo.iter().map(|text| text.len()).sum();
        while bytes > HISTORY_LIMIT || self.undo.len() > 200 {
            bytes -= self.undo.pop_front().map_or(0, |text| text.len());
        }
        true
    }

    pub(crate) fn undo(&mut self, redo: bool) -> bool {
        let replacement = if redo {
            self.redo.pop()
        } else {
            self.undo.pop_back()
        };
        let Some(replacement) = replacement else {
            return false;
        };
        let previous = std::mem::replace(&mut self.text, replacement);
        if redo {
            self.undo.push_back(previous);
        } else {
            self.redo.push(previous);
        }
        true
    }
}

#[cfg(test)]
mod tests;
