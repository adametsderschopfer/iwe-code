//! Populate the native editor before its first layout. Only visible lines need glyph positions.

use crate::ui::theme::iced;
use iced::{
    Font, Pixels, Size,
    advanced::{
        graphics::text::{Editor, cosmic_text::LineIter},
        text::{Editor as _, LineHeight, Wrapping, highlighter::PlainText},
    },
    widget::text_editor::{Action, Content, Cursor, Edit, Position},
};
use std::sync::Arc;

pub(super) fn content(text: &str) -> Content {
    // An unconstrained Iced buffer shapes the entire document before its widget gets a height.
    let mut editor = Editor::with_text("");
    editor.update(
        Size::new(1., 1.),
        Font::MONOSPACE,
        Pixels(1.),
        LineHeight::Relative(1.),
        Wrapping::None,
        &mut PlainText,
    );
    // Keep native font-lock intervals short. Split at complete line endings, including CRLF.
    let mut start = 0;
    for (index, (line, ending)) in LineIter::new(text).enumerate() {
        let end = line.end + ending.as_str().len();
        if end - start >= 4096 || index % 64 == 63 || end == text.len() {
            editor.perform(Action::Edit(Edit::Paste(Arc::new(
                text[start..end].to_owned(),
            ))));
            start = end;
        }
    }
    editor.move_to(Cursor {
        position: Position { line: 0, column: 0 },
        selection: None,
    });
    Content::from_editor(editor)
}
