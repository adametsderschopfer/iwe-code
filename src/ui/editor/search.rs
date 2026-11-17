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
                    for (index, hit) in self
                        .search
                        .results
                        .hits
                        .iter()
                        .enumerate()
                        .filter(|(_, hit)| hit.found.line == run.line_i + 1)
                    {
                        let found = &hit.found;
                        let mut left = f32::MAX;
                        let mut right = 0.0_f32;
                        for glyph in run.glyphs.iter().filter(|glyph| {
                            glyph.start < found.end_byte_column && glyph.end > found.byte_column
                        }) {
                            left = left.min(glyph.x);
                            right = right.max(glyph.x + glyph.w);
                        }
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
