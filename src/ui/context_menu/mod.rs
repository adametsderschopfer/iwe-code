//! Anchor context menus to right-click positions without tracking every pointer movement.

mod item;
pub(crate) use item::{ICON_COLUMN_WIDTH, action, action_style, icon_label, label};

use iced::{
    Element, Event, Length, Point, Rectangle, Renderer, Size, Theme, Vector,
    advanced::{
        Clipboard, Layout, Shell, Widget, layout, mouse, overlay, renderer,
        widget::{Operation, Tree},
    },
    keyboard,
    widget::{container, mouse_area, opaque, responsive, stack},
};

pub(crate) fn trigger<'a, Message: 'a>(
    content: impl Into<Element<'a, Message>>,
    on_open: impl Fn(Point) -> Message + 'a,
) -> Element<'a, Message> {
    Element::new(Trigger {
        content: content.into(),
        on_open: Some(Box::new(move |point, _| on_open(point))),
        on_key: None,
        window_anchor: None,
        popup_position: None,
        anchored: false,
        fallback: false,
    })
}

/// Resolve popup anchors before a scrollable translates the pointer into content coordinates.
pub(crate) fn window_anchor<'a, Message: 'a>(
    content: Element<'a, Message>,
    map: fn(Message, Point) -> Message,
) -> Element<'a, Message> {
    Element::new(Trigger {
        content,
        on_open: None,
        on_key: None,
        window_anchor: Some(map),
        popup_position: None,
        anchored: false,
        fallback: false,
    })
}

/// Include the content-relative position for widgets that perform hit testing.
pub(crate) fn positioned<'a, Message: 'a>(
    content: impl Into<Element<'a, Message>>,
    on_open: impl Fn(Point, Point) -> Message + 'a,
) -> Element<'a, Message> {
    Element::new(Trigger {
        content: content.into(),
        on_open: Some(Box::new(on_open)),
        on_key: None,
        window_anchor: None,
        popup_position: None,
        anchored: false,
        fallback: false,
    })
}

pub(crate) fn fallback<'a, Message: 'a>(
    content: impl Into<Element<'a, Message>>,
    on_open: impl Fn(Point) -> Message + 'a,
) -> Element<'a, Message> {
    Element::new(Trigger {
        content: content.into(),
        on_open: Some(Box::new(move |point, _| on_open(point))),
        on_key: None,
        window_anchor: None,
        popup_position: None,
        anchored: false,
        fallback: true,
    })
}

pub(crate) fn anchored<'a, Message: 'a>(
    content: impl Into<Element<'a, Message>>,
    on_open: impl Fn(Point) -> Message + 'a,
) -> Element<'a, Message> {
    Element::new(Trigger {
        content: content.into(),
        on_open: Some(Box::new(move |point, _| on_open(point))),
        on_key: None,
        window_anchor: None,
        popup_position: None,
        anchored: true,
        fallback: false,
    })
}

type KeyHandler<'a, Message> = Box<dyn Fn(&keyboard::Event) -> Option<Message> + 'a>;

/// Handle popup keys before focused content can consume them.
pub(crate) fn on_key<'a, Message: 'a>(
    content: impl Into<Element<'a, Message>>,
    handler: impl Fn(&keyboard::Event) -> Option<Message> + 'a,
) -> Element<'a, Message> {
    Element::new(Trigger {
        content: content.into(),
        on_open: None,
        on_key: Some(Box::new(handler)),
        window_anchor: None,
        popup_position: None,
        anchored: false,
        fallback: false,
    })
}

pub(crate) fn dismiss_on_escape<'a, Message: Clone + 'a>(
    content: impl Into<Element<'a, Message>>,
    dismiss: Message,
) -> Element<'a, Message> {
    on_key(content, move |event| {
        matches!(
            event,
            keyboard::Event::KeyPressed {
                key: keyboard::Key::Named(keyboard::key::Named::Escape),
                ..
            }
        )
        .then(|| dismiss.clone())
    })
}

struct Trigger<'a, Message> {
    content: Element<'a, Message>,
    on_open: Option<Box<dyn Fn(Point, Point) -> Message + 'a>>,
    on_key: Option<KeyHandler<'a, Message>>,
    window_anchor: Option<fn(Message, Point) -> Message>,
    popup_position: Option<Placement>,
    anchored: bool,
    fallback: bool,
}

impl<Message> Widget<Message, Theme, Renderer> for Trigger<'_, Message> {
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
        let node = self
            .content
            .as_widget_mut()
            .layout(&mut tree.children[0], renderer, limits);
        let Some(position) = self.popup_position else {
            return node;
        };
        let available = limits.max();
        let panel = node.size();
        node.move_to(position.origin(panel, available))
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
        if let Event::Keyboard(event) = event
            && let Some(message) = self.on_key.as_ref().and_then(|handler| handler(event))
        {
            shell.publish(message);
            shell.capture_event();
            return;
        }
        let mut update_child = |shell: &mut Shell<'_, Message>| {
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
        };
        if let Some(map) = self.window_anchor
            && matches!(
                event,
                Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Right))
            )
            && let Some(point) = cursor.position()
        {
            let mut messages = Vec::new();
            let mut child_shell = Shell::new(&mut messages);
            update_child(&mut child_shell);
            shell.merge(child_shell, |message| map(message, point));
            return;
        }
        if self.fallback {
            update_child(shell);
            if shell.is_event_captured() {
                return;
            }
        }
        let button = if self.anchored {
            mouse::Button::Left
        } else {
            mouse::Button::Right
        };
        if matches!(event, Event::Mouse(mouse::Event::ButtonPressed(pressed)) if *pressed == button)
            && let Some(on_open) = &self.on_open
            && let Some(position) = cursor
                .position()
                .filter(|point| layout.bounds().contains(*point) && viewport.contains(*point))
        {
            let position = if self.anchored {
                let bounds = layout.bounds();
                Point::new(bounds.x, bounds.y + bounds.height + 6.)
            } else {
                position
            };
            shell.publish(on_open(
                position,
                Point::new(
                    position.x - layout.bounds().x,
                    position.y - layout.bounds().y,
                ),
            ));
            shell.capture_event();
            return;
        }
        if !self.fallback {
            update_child(shell);
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
        if self.anchored && cursor.is_over(layout.bounds()) && cursor.is_over(*viewport) {
            return mouse::Interaction::Pointer;
        }
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

pub(crate) use super::presentation::divider as separator;

pub(crate) const ROW_PADDING: [u16; 2] = [5, 8];
pub(crate) const TEXT_SIZE: f32 = 13.;
pub(crate) const SURFACE_PADDING: u16 = 6;

pub(crate) fn row<'a, Message: Clone + 'a>(
    content: impl Into<Element<'a, Message>>,
) -> iced::widget::Button<'a, Message> {
    iced::widget::button(content)
        .padding(ROW_PADDING)
        .width(Length::Fill)
        .style(super::theme::action(false))
}

pub(crate) fn text_row<'a, Message: Clone + 'a>(
    label: String,
) -> iced::widget::Button<'a, Message> {
    row(iced::widget::text(label)
        .size(TEXT_SIZE)
        .wrapping(iced::widget::text::Wrapping::None))
}

pub(crate) fn surface<'a, Message: 'a>(
    content: Element<'a, Message>,
    width: f32,
) -> Element<'a, Message> {
    opaque(
        container(content)
            .padding(SURFACE_PADDING)
            .width(width)
            .style(super::theme::tooltip),
    )
}

fn popup_origin(anchor: f32, extent: f32, available: f32) -> f32 {
    let anchor = anchor.clamp(0., available);
    if anchor + extent <= available {
        anchor
    } else if extent <= anchor {
        anchor - extent
    } else {
        (available - extent).max(0.)
    }
}

#[derive(Clone, Copy)]
enum Placement {
    Cursor(Point),
    Beside(Rectangle),
}

impl Placement {
    fn origin(self, panel: Size, available: Size) -> Point {
        match self {
            Self::Cursor(point) => Point::new(
                popup_origin(point.x, panel.width, available.width),
                popup_origin(point.y, panel.height, available.height),
            ),
            Self::Beside(anchor) => {
                let right = anchor.x + anchor.width + 4.;
                let left = anchor.x - panel.width - 4.;
                let max_x = (available.width - panel.width).max(0.);
                let x = if right <= max_x || (left < 0. && right - max_x <= -left) {
                    right
                } else {
                    left
                };
                Point::new(
                    x.clamp(0., max_x),
                    anchor
                        .y
                        .clamp(0., (available.height - panel.height).max(0.)),
                )
            }
        }
    }
}

pub(crate) fn overlay<'a, Message: Clone + 'a>(
    base: Element<'a, Message>,
    position: Point,
    size: Size,
    content: impl Fn() -> Element<'a, Message> + 'a,
    dismiss: Message,
) -> Element<'a, Message> {
    overlay_with(
        base,
        position,
        move |available| surface(content(), size.width.min(available)),
        dismiss,
    )
}

/// Position a module-styled menu using the same event handling and screen bounds.
pub(crate) fn overlay_with<'a, Message: Clone + 'a>(
    base: Element<'a, Message>,
    position: Point,
    content: impl Fn(f32) -> Element<'a, Message> + 'a,
    dismiss: Message,
) -> Element<'a, Message> {
    modal(base, panel(Placement::Cursor(position), content), dismiss)
}

pub(crate) fn beside<'a, Message: Clone + 'a>(
    base: Element<'a, Message>,
    anchor: Rectangle,
    width: f32,
    content: impl Fn() -> Element<'a, Message> + 'a,
    dismiss: Message,
    has_parent: bool,
) -> Element<'a, Message> {
    let popup = panel(Placement::Beside(anchor), move |available| {
        surface(content(), width.min(available))
    });
    if has_parent {
        dismiss_on_escape(stack![base, popup], dismiss)
    } else {
        modal(base, popup, dismiss)
    }
}

fn panel<'a, Message: 'a>(
    placement: Placement,
    content: impl Fn(f32) -> Element<'a, Message> + 'a,
) -> Element<'a, Message> {
    responsive(move |available| {
        Element::new(Trigger {
            content: content(available.width),
            on_open: None,
            on_key: None,
            window_anchor: None,
            popup_position: Some(placement),
            anchored: false,
            fallback: false,
        })
    })
    .into()
}

fn modal<'a, Message: Clone + 'a>(
    base: Element<'a, Message>,
    popup: Element<'a, Message>,
    dismiss: Message,
) -> Element<'a, Message> {
    let content = stack![
        base,
        opaque(
            mouse_area(popup)
                .on_press(dismiss.clone())
                .on_right_press(dismiss.clone())
        )
    ];
    dismiss_on_escape(content, dismiss)
}

#[cfg(test)]
mod tests;
