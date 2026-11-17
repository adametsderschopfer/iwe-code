//! Draw diagnostic underlines using the editor's shaped text positions.

use crate::{
    domain::language::{Diagnostic, Position, byte_offset},
    ui::theme,
};
use iced::{
    Point, Rectangle, Renderer, Theme, mouse,
    widget::{canvas, text_editor},
};

pub(super) struct Underlines<'a> {
    pub content: &'a text_editor::Content,
    pub diagnostics: &'a [Diagnostic],
    pub text: &'a str,
}

impl<Message> canvas::Program<Message> for Underlines<'_> {
    type State = ();
    fn update(
        &self,
        _: &mut (),
        event: &canvas::Event,
        _: Rectangle,
        _: mouse::Cursor,
    ) -> Option<canvas::Action<Message>> {
        matches!(
            event,
            canvas::Event::Mouse(mouse::Event::CursorMoved { .. } | mouse::Event::CursorLeft)
        )
        .then(canvas::Action::request_redraw)
    }
    fn draw(
        &self,
        _: &(),
        renderer: &Renderer,
        theme: &Theme,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        let mut hovered = None;
        frame.with_clip(Rectangle::with_size(bounds.size()), |frame| {
            self.content.with_editor(|editor| {
                for run in editor.buffer().layout_runs() {
                    let Some(line) = self.text.split('\n').nth(run.line_i) else {
                        continue;
                    };
                    for diagnostic in self.diagnostics.iter().filter(|value| {
                        value.range.start.line as usize <= run.line_i
                            && value.range.end.line as usize >= run.line_i
                    }) {
                        let start = if diagnostic.range.start.line as usize == run.line_i {
                            byte_offset(
                                line,
                                Position {
                                    line: 0,
                                    character: diagnostic.range.start.character,
                                },
                            )
                            .unwrap_or(line.len())
                        } else {
                            0
                        };
                        let end = if diagnostic.range.end.line as usize == run.line_i {
                            byte_offset(
                                line,
                                Position {
                                    line: 0,
                                    character: diagnostic.range.end.character,
                                },
                            )
                            .unwrap_or(line.len())
                        } else {
                            line.len()
                        };
                        let mut left = f32::MAX;
                        let mut right = 0_f32;
                        for glyph in run
                            .glyphs
                            .iter()
                            .filter(|glyph| glyph.start < end && glyph.end > start)
                        {
                            left = left.min(glyph.x);
                            right = right.max(glyph.x + glyph.w);
                        }
                        if left < right {
                            let area = Rectangle {
                                x: super::TEXT_PADDING + left,
                                y: super::TEXT_PADDING + run.line_top,
                                width: right - left,
                                height: run.line_height,
                            };
                            if cursor
                                .position_in(bounds)
                                .is_some_and(|point| area.contains(point))
                            {
                                hovered = Some((&diagnostic.message, area));
                            }
                            let color = if diagnostic.severity == Some(1) {
                                theme::RED.resolve(theme)
                            } else {
                                theme::MUTED.resolve(theme)
                            };
                            let y = super::TEXT_PADDING + run.line_top + run.line_height - 2.;
                            let path = canvas::Path::new(|builder| {
                                builder.move_to(Point::new(super::TEXT_PADDING + left, y));
                                let mut x = left;
                                let mut down = true;
                                while x < right {
                                    x = (x + 2.).min(right);
                                    builder.line_to(Point::new(
                                        super::TEXT_PADDING + x,
                                        y + if down { 2. } else { 0. },
                                    ));
                                    down = !down;
                                }
                            });
                            frame.stroke(
                                &path,
                                canvas::Stroke::default().with_color(color).with_width(1.),
                            );
                        }
                    }
                }
            });
        });
        if let Some((message, area)) = hovered {
            let width = 480_f32.min(bounds.width);
            let columns = ((width - 24.) / 7.5).max(1.) as usize;
            let mut lines = Vec::new();
            for line in message.lines() {
                let characters: Vec<_> = line.chars().collect();
                for chunk in characters.chunks(columns) {
                    lines.push(chunk.iter().collect::<String>());
                }
            }
            let height = (lines.len() as f32 * 18. + 20.).min(bounds.height);
            let x = area.x.min((bounds.width - width).max(0.));
            let y = if area.y + area.height + height <= bounds.height {
                area.y + area.height
            } else {
                (area.y - height).max(0.)
            };
            frame.with_clip(
                Rectangle {
                    x,
                    y,
                    width,
                    height,
                },
                |frame| {
                    frame.fill_rectangle(
                        Point::ORIGIN,
                        iced::Size::new(width, height),
                        theme::CODE_SELECTION.resolve(theme),
                    );
                    for (index, line) in lines.iter().enumerate() {
                        frame.fill_text(canvas::Text {
                            content: line.clone(),
                            position: Point::new(12., 10. + index as f32 * 18.),
                            color: theme::syntax::TEXT.resolve(theme),
                            size: 13.into(),
                            ..Default::default()
                        });
                    }
                },
            );
        }
        vec![frame.into_geometry()]
    }
}
