//! Move focus between form fields and reveal the selected field in its scroll area.

use crate::ui::theme::iced;
use iced::{
    Element, Event, Length, Rectangle, Renderer, Size, Theme, Vector,
    advanced::{
        Clipboard, Layout, Shell, Widget, layout, mouse, overlay, renderer,
        widget::{
            Operation, Tree,
            operation::{self, Outcome, focusable},
        },
    },
    keyboard,
};

pub(crate) fn modal_scope<'a, Message: 'a>(content: Element<'a, Message>) -> Element<'a, Message> {
    Element::new(Input {
        content,
        modal: true,
    })
}

pub(crate) fn form<'a, Message: 'a>(content: Element<'a, Message>) -> Element<'a, Message> {
    Element::new(Input {
        content,
        modal: false,
    })
}

struct Input<'a, Message> {
    content: Element<'a, Message>,
    modal: bool,
}

impl<Message> Widget<Message, Theme, Renderer> for Input<'_, Message> {
    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(&self.content)]
    }
    fn diff(&self, tree: &mut Tree) {
        tree.diff_children(std::slice::from_ref(&self.content));
    }
    fn size(&self) -> Size<Length> {
        self.content.as_widget().size()
    }
    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        self.content
            .as_widget_mut()
            .layout(&mut tree.children[0], renderer, limits)
    }
    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn Operation,
    ) {
        self.content
            .as_widget_mut()
            .operate(&mut tree.children[0], layout, renderer, operation);
    }
    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        if let Event::Keyboard(keyboard::Event::KeyPressed {
            key: keyboard::Key::Named(keyboard::key::Named::Tab),
            modifiers,
            ..
        }) = event
            && (*modifiers == keyboard::Modifiers::NONE || *modifiers == keyboard::Modifiers::SHIFT)
        {
            let mut count = focusable::count();
            self.operate(
                tree,
                layout,
                renderer,
                &mut operation::black_box(&mut count),
            );
            if let Outcome::Some(count) = count.finish()
                && count.total > 0
                && (self.modal || count.focused.is_some())
            {
                let target = if modifiers.shift() {
                    (count.focused.unwrap_or(0) + count.total - 1) % count.total
                } else {
                    count.focused.map_or(0, |index| (index + 1) % count.total)
                };
                let mut focus = FocusField {
                    target,
                    ..Default::default()
                };
                self.operate(tree, layout, renderer, &mut focus);
                if !focus.offsets.is_empty() {
                    self.operate(
                        tree,
                        layout,
                        renderer,
                        &mut Reveal {
                            next: 0,
                            offsets: focus.offsets,
                        },
                    );
                }
                shell.request_redraw();
                shell.capture_event();
                return;
            }
        }
        self.content.as_widget_mut().update(
            &mut tree.children[0],
            event,
            layout,
            cursor,
            renderer,
            clipboard,
            shell,
            viewport,
        );
        if self.modal && !matches!(event, Event::Window(_)) {
            shell.capture_event();
        }
    }
    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        self.content.as_widget().mouse_interaction(
            &tree.children[0],
            layout,
            cursor,
            viewport,
            renderer,
        )
    }
    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        self.content.as_widget().draw(
            &tree.children[0],
            renderer,
            theme,
            style,
            layout,
            cursor,
            viewport,
        );
    }
    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'b>,
        renderer: &Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'b, Message, Theme, Renderer>> {
        self.content.as_widget_mut().overlay(
            &mut tree.children[0],
            layout,
            renderer,
            viewport,
            translation,
        )
    }
}

#[derive(Default)]
struct FocusField {
    index: usize,
    target: usize,
    next_scroll: usize,
    entering: Option<ScrollArea>,
    ancestors: Vec<ScrollArea>,
    offsets: Vec<(usize, f32)>,
}

struct ScrollArea {
    index: usize,
    bounds: Rectangle,
    offset: f32,
    maximum: f32,
}

impl Operation for FocusField {
    fn traverse(&mut self, visit: &mut dyn FnMut(&mut dyn Operation)) {
        if let Some(scroll) = self.entering.take() {
            self.ancestors.push(scroll);
            visit(self);
            self.ancestors.pop();
        } else {
            visit(self);
        }
    }

    fn scrollable(
        &mut self,
        _: Option<&iced::widget::Id>,
        bounds: Rectangle,
        content: Rectangle,
        offset: Vector,
        _: &mut dyn operation::Scrollable,
    ) {
        self.entering = Some(ScrollArea {
            index: self.next_scroll,
            bounds,
            offset: offset.y,
            maximum: (content.height - bounds.height).max(0.),
        });
        self.next_scroll += 1;
    }

    fn focusable(
        &mut self,
        _: Option<&iced::widget::Id>,
        mut bounds: Rectangle,
        field: &mut dyn focusable::Focusable,
    ) {
        if self.index == self.target {
            field.focus();
            for scroll in self.ancestors.iter().rev() {
                let top = bounds.y - scroll.bounds.y;
                let last = top - (scroll.bounds.height - bounds.height).max(0.);
                let offset = scroll.offset.clamp(last, top).clamp(0., scroll.maximum);
                if offset != scroll.offset {
                    self.offsets.push((scroll.index, offset));
                }
                bounds.y -= offset;
            }
        } else {
            field.unfocus();
        }
        self.index += 1;
    }
}

struct Reveal {
    next: usize,
    offsets: Vec<(usize, f32)>,
}

impl Operation for Reveal {
    fn traverse(&mut self, visit: &mut dyn FnMut(&mut dyn Operation)) {
        visit(self);
    }

    fn scrollable(
        &mut self,
        _: Option<&iced::widget::Id>,
        _: Rectangle,
        _: Rectangle,
        _: Vector,
        state: &mut dyn operation::Scrollable,
    ) {
        if let Some((_, y)) = self.offsets.iter().find(|(index, _)| *index == self.next) {
            state.scroll_to(operation::scrollable::AbsoluteOffset {
                x: None,
                y: Some(*y),
            });
        }
        self.next += 1;
    }
}

#[cfg(test)]
mod tests;
