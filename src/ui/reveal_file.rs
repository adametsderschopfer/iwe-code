//! Scroll a rendered row into view after its list has updated.

use iced::{
    Rectangle, Task, Vector,
    advanced::widget::{
        self, Id, Operation,
        operation::{self, Outcome, Scrollable},
    },
};

pub(crate) fn row<Message: Send + 'static>(path: &std::path::Path, depth: usize) -> Task<Message> {
    in_list(
        "project-files-browser".into(),
        Id::from(format!("file-tree-{}", path.to_string_lossy())),
        depth as f32 * super::theme::FILE_TREE_INDENT,
    )
}

pub(crate) fn in_list<Message: Send + 'static>(
    browser: Id,
    target: Id,
    indent: f32,
) -> Task<Message> {
    widget::operate(Reveal {
        browser,
        target,
        indent,
        viewport: None,
        row: None,
    })
}

struct Reveal {
    browser: Id,
    target: Id,
    indent: f32,
    viewport: Option<(Rectangle, Vector)>,
    row: Option<Rectangle>,
}

impl<Message: Send + 'static> Operation<Message> for Reveal {
    fn traverse(&mut self, operate: &mut dyn FnMut(&mut dyn Operation<Message>)) {
        operate(self);
    }
    fn scrollable(
        &mut self,
        id: Option<&Id>,
        bounds: Rectangle,
        _: Rectangle,
        translation: Vector,
        _: &mut dyn Scrollable,
    ) {
        if id == Some(&self.browser) {
            self.viewport = Some((bounds, translation));
        }
    }
    fn container(&mut self, id: Option<&Id>, bounds: Rectangle) {
        if id == Some(&self.target) {
            self.row = Some(bounds);
        }
    }
    fn finish(&self) -> Outcome<Message> {
        let (Some((viewport, offset)), Some(row)) = (self.viewport, self.row) else {
            return Outcome::None;
        };
        let y = row.y - viewport.y;
        let x = row.x + self.indent - viewport.x;
        let vertical = if y < offset.y || y + row.height > offset.y + viewport.height {
            (y - (viewport.height - row.height) / 2.).max(0.)
        } else {
            offset.y
        };
        let horizontal = if x < offset.x || x + 80. > offset.x + viewport.width {
            (x - 12.).max(0.)
        } else {
            offset.x
        };
        Outcome::Chain(Box::new(operation::scrollable::scroll_to(
            self.browser.clone(),
            operation::scrollable::AbsoluteOffset {
                x: Some(horizontal),
                y: Some(vertical),
            },
        )))
    }
}
