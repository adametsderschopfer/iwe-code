//! Paint visible search matches using the editor's shaped glyph positions.
use crate::{app::text_search::Search, ui::theme};
use iced::{
    Point, Rectangle, Renderer, Theme, mouse,
    widget::{canvas, text_editor},
};
pub(super) struct Highlights<'a> {
    pub content: &'a text_editor::Content,
    pub search: &'a Search,
}
impl<Message> canvas::Program<Message> for Highlights<'_> {
    type State = ();
    fn draw(
        &self,
        _: &(),
        renderer: &Renderer,
        theme: &Theme,
        bounds: Rectangle,
        _: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        frame.with_clip(Rectangle::with_size(bounds.size()), |frame| {
            self.content.with_editor(|editor| {
                for run in editor.buffer().layout_runs() {
                    let hits = &self.search.results.hits;
                    let line = run.line_i + 1;
                    let start = hits.partition_point(|hit| hit.found.line < line);
                    let hits = &hits[start..];
                    let hits = &hits[..hits.partition_point(|hit| hit.found.line == line)];
                    if hits.is_empty() {
                        continue;
                    }
                    let mut bounds = vec![(f32::MAX, f32::MIN); hits.len()];
                    for glyph in run.glyphs {
                        // Glyphs can follow visual order in bidirectional text.
                        let first =
                            hits.partition_point(|hit| hit.found.end_byte_column <= glyph.start);
                        for (index, _) in hits[first..]
                            .iter()
                            .take_while(|hit| hit.found.byte_column < glyph.end)
                            .enumerate()
                        {
                            let (left, right) = &mut bounds[first + index];
                            *left = left.min(glyph.x);
                            *right = right.max(glyph.x + glyph.w);
                        }
                    }
                    for (offset, (left, right)) in bounds.into_iter().enumerate() {
                        let index = start + offset;
                        if left < right {
                            let mut color = theme::CODE_SELECTION.resolve(theme);
                            color.a = if index == self.search.selected {
                                0.65
                            } else {
                                0.3
                            };
                            frame.fill_rectangle(
                                Point::new(
                                    super::TEXT_PADDING + left,
                                    super::TEXT_PADDING + run.line_top,
                                ),
                                iced::Size::new(right - left, run.line_height),
                                color,
                            );
                        }
                    }
                }
            });
        });
        vec![frame.into_geometry()]
    }
}
