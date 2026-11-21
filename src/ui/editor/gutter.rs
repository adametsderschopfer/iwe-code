//! Draw line numbers from the editor's actual layout and scroll position.

use crate::{
    app::{
        Message,
        editor::{Event, marks},
    },
    domain::editor::hunks,
    ui::theme,
};
use iced::{
    Point, Rectangle, Renderer, Theme, mouse,
    widget::{canvas, text_editor},
};

// Reserve the splitter's drag area beside the file tree.
const MARKER_START: f32 = theme::FILE_PANEL_RESIZE_GRAB + 2.;
const MARKER_WIDTH: f32 = 4.;
pub(super) const MARKER_LANE: f32 = MARKER_START + MARKER_WIDTH + 3.;

pub(super) struct Gutter<'a> {
    pub content: &'a text_editor::Content,
    pub tab: u64,
    pub marks: &'a [hunks::Hunk],
    pub numbers: bool,
    pub size: f32,
    pub line_height: f32,
    pub font: iced::Font,
}

impl Gutter<'_> {
    fn hit(&self, bounds: Rectangle, cursor: mouse::Cursor) -> Option<usize> {
        let point = cursor.position_in(bounds)?;
        if !(MARKER_START - 2. ..=MARKER_LANE).contains(&point.x) {
            return None;
        }
        self.content.with_editor(|editor| {
            let line = editor.buffer().layout_runs().find(|run| {
                let top = super::TEXT_PADDING + run.line_top;
                point.y >= top && point.y < top + run.line_height
            })?;
            hunks::at(self.marks, line.line_i)
        })
    }
}

impl canvas::Program<Message> for Gutter<'_> {
    type State = ();

    fn mouse_interaction(
        &self,
        _: &(),
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> mouse::Interaction {
        if self.hit(bounds, cursor).is_some() {
            mouse::Interaction::Pointer
        } else {
            mouse::Interaction::default()
        }
    }

    fn update(
        &self,
        _: &mut (),
        event: &canvas::Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas::Action<Message>> {
        if matches!(
            event,
            canvas::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
        ) {
            let index = self.hit(bounds, cursor)?;
            return Some(
                canvas::Action::publish(Message::Editor(Event::Marks(marks::Event::Show(
                    self.tab,
                    index,
                    cursor.position()?,
                ))))
                .and_capture(),
            );
        }
        None
    }

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
                for line in editor.buffer().layout_runs() {
                    if let Some(index) = hunks::at(self.marks, line.line_i) {
                        let hunk = &self.marks[index];
                        let color = if hunk.old.is_empty() {
                            theme::GIT_ADDED
                        } else if hunk.new.is_empty() {
                            theme::GIT_REMOVED
                        } else {
                            theme::DIFF_MODIFIED
                        };
                        frame.fill_rectangle(
                            Point::new(MARKER_START, super::TEXT_PADDING + line.line_top),
                            iced::Size::new(
                                MARKER_WIDTH,
                                if hunk.new.is_empty() {
                                    4.
                                } else {
                                    line.line_height
                                },
                            ),
                            color.resolve(theme),
                        );
                    }
                    if !self.numbers {
                        continue;
                    }
                    frame.fill_text(canvas::Text {
                        content: (line.line_i + 1).to_string(),
                        position: Point::new(
                            bounds.width - 8.,
                            super::TEXT_PADDING + line.line_top,
                        ),
                        color: theme::MUTED.resolve(theme),
                        size: self.size.into(),
                        line_height: iced::widget::text::LineHeight::Relative(self.line_height),
                        font: self.font,
                        align_x: iced::widget::text::Alignment::Right,
                        ..Default::default()
                    });
                }
            });
        });
        vec![frame.into_geometry()]
    }
}
