//! Reveal a trailing row action without reserving space when it is hidden.

use super::theme;
use crate::ui::theme::iced;
use iced::{
    Animation, Element, Event, Length, Point, Rectangle, Renderer, Size, Theme, Vector,
    advanced::{
        Clipboard, Layout, Shell, Widget, layout, mouse, overlay,
        renderer::{self, Renderer as _},
        widget::{Operation, Tree, tree},
    },
    window,
};
use std::time::{Duration, Instant};

pub(crate) fn trailing<'a, Message: 'a>(
    content: impl Into<Element<'a, Message>>,
    action: impl Into<Element<'a, Message>>,
    animate: bool,
) -> Element<'a, Message> {
    Element::new(Trailing {
        children: [content.into(), action.into()],
        animate,
    })
}

struct Trailing<'a, Message> {
    children: [Element<'a, Message>; 2],
    animate: bool,
}

struct State {
    animation: Animation<bool>,
    progress: f32,
}

impl Default for State {
    fn default() -> Self {
        Self {
            animation: Animation::new(false).duration(theme::SPACE_ACTION_TRANSITION),
            progress: 0.,
        }
    }
}

impl<Message> Widget<Message, Theme, Renderer> for Trailing<'_, Message> {
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(State::default())
    }

    fn children(&self) -> Vec<Tree> {
        self.children.iter().map(Tree::new).collect()
    }

    fn diff(&self, tree: &mut Tree) {
        tree.diff_children(&self.children);
    }

    fn size(&self) -> Size<Length> {
        self.children[0].as_widget().size()
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        const GAP: f32 = 4.;
        let width = limits.width(Length::Fill).max().width;
        let action = self.children[1].as_widget_mut().layout(
            &mut tree.children[1],
            renderer,
            &layout::Limits::new(Size::ZERO, limits.max()),
        );
        let progress = tree.state.downcast_ref::<State>().progress;
        let reserved = (action.size().width + GAP) * progress;
        let content = self.children[0].as_widget_mut().layout(
            &mut tree.children[0],
            renderer,
            &limits.width((width - reserved).max(0.)),
        );
        let height = content.size().height.max(action.size().height);
        let action_y = (height - action.size().height) / 2.;
        let action = action.move_to(Point::new(width - reserved + GAP, action_y));
        layout::Node::with_children(Size::new(width, height), vec![content, action])
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn Operation,
    ) {
        for ((child, tree), layout) in self
            .children
            .iter_mut()
            .zip(&mut tree.children)
            .zip(layout.children())
        {
            child
                .as_widget_mut()
                .operate(tree, layout, renderer, operation);
        }
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
        let state = tree.state.downcast_mut::<State>();
        let visible = layout.bounds().intersection(viewport);
        let active = visible.is_some_and(|bounds| cursor.is_over(bounds));
        let now = match event {
            Event::Window(window::Event::RedrawRequested(now)) => *now,
            _ => Instant::now(),
        };
        if active != state.animation.value() {
            state.animation.go_mut(active, now);
            shell.request_redraw();
        }
        let redraw = matches!(event, Event::Window(window::Event::RedrawRequested(_)));
        if redraw || !self.animate {
            let progress = if self.animate {
                state.animation.interpolate(0., 1., now)
            } else {
                f32::from(active)
            };
            if progress != state.progress {
                state.progress = progress;
                shell.invalidate_layout();
                shell.request_redraw();
            }
            if self.animate && state.animation.is_animating(now) {
                shell.request_redraw_at(now + Duration::from_millis(16));
            }
        }

        // Forward release events even after the pointer leaves so pressed buttons reset.
        for ((child, tree), child_layout) in self
            .children
            .iter_mut()
            .zip(&mut tree.children)
            .zip(layout.children())
            .rev()
        {
            let child_cursor = if active {
                cursor
            } else {
                mouse::Cursor::Unavailable
            };
            child.as_widget_mut().update(
                tree,
                event,
                child_layout,
                child_cursor,
                renderer,
                clipboard,
                shell,
                viewport,
            );
            if shell.is_event_captured() {
                return;
            }
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
        if !layout
            .bounds()
            .intersection(viewport)
            .is_some_and(|bounds| cursor.is_over(bounds))
        {
            return mouse::Interaction::None;
        }
        self.children
            .iter()
            .zip(&tree.children)
            .zip(layout.children())
            .rev()
            .map(|((child, tree), layout)| {
                child
                    .as_widget()
                    .mouse_interaction(tree, layout, cursor, viewport, renderer)
            })
            .find(|interaction| *interaction != mouse::Interaction::None)
            .unwrap_or_default()
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
        let Some(clip) = layout.bounds().intersection(viewport) else {
            return;
        };
        renderer.with_layer(clip, |renderer| {
            for ((child, tree), layout) in self
                .children
                .iter()
                .zip(&tree.children)
                .zip(layout.children())
            {
                if layout.bounds().intersects(&clip) {
                    child
                        .as_widget()
                        .draw(tree, renderer, theme, style, layout, cursor, &clip);
                }
            }
        });
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'b>,
        renderer: &Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'b, Message, Theme, Renderer>> {
        overlay::from_children(
            &mut self.children,
            tree,
            layout,
            renderer,
            viewport,
            translation,
        )
    }
}
