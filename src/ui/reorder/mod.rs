//! Reorder column rows while preserving clicks and trailing row actions.

use super::theme;
use crate::ui::theme::iced;
use iced::{
    Border, Element, Event, Length, Point, Rectangle, Renderer, Size, Theme, Vector,
    advanced::{
        Clipboard, Layout, Shell, Widget, layout, mouse, overlay,
        renderer::{self, Renderer as _},
        widget::{Operation, Tree, tree},
    },
    widget::Column,
    window,
};
use std::time::{Duration, Instant};

pub(crate) fn column<'a, Message: 'a>(
    content: Column<'a, Message>,
    action_width: f32,
    on_move: impl Fn(usize, usize) -> Message + 'a,
    on_scroll: impl Fn(f32) -> Message + 'a,
) -> Element<'a, Message> {
    Element::new(Reorder {
        content: content.into(),
        action_width,
        groups: None,
        on_move: Box::new(on_move),
        on_scroll: Some(Box::new(on_scroll)),
    })
}

pub(crate) fn fixed_column<'a, Message: 'a>(
    content: Column<'a, Message>,
    on_move: impl Fn(usize, usize) -> Message + 'a,
) -> Element<'a, Message> {
    Element::new(Reorder {
        content: content.into(),
        action_width: 0.,
        groups: None,
        on_move: Box::new(on_move),
        on_scroll: None,
    })
}

pub(crate) fn groups<'a, Message: 'a>(
    content: Column<'a, Message>,
    project_counts: Vec<usize>,
    has_pinned: bool,
    on_move: impl Fn(usize, usize) -> Message + 'a,
    on_project_drop: impl Fn(usize, usize, usize, Option<usize>) -> Option<Message> + 'a,
    on_scroll: impl Fn(f32) -> Message + 'a,
) -> Element<'a, Message> {
    Element::new(Reorder {
        content: content.into(),
        action_width: 36.,
        groups: Some(Groups {
            project_counts,
            has_pinned,
            on_project_drop: Box::new(on_project_drop),
        }),
        on_move: Box::new(on_move),
        on_scroll: Some(Box::new(on_scroll)),
    })
}

type ProjectDrop<'a, Message> = dyn Fn(usize, usize, usize, Option<usize>) -> Option<Message> + 'a;

struct Groups<'a, Message> {
    project_counts: Vec<usize>,
    has_pinned: bool,
    on_project_drop: Box<ProjectDrop<'a, Message>>,
}

struct Reorder<'a, Message> {
    content: Element<'a, Message>,
    action_width: f32,
    groups: Option<Groups<'a, Message>>,
    on_move: Box<dyn Fn(usize, usize) -> Message + 'a>,
    on_scroll: Option<Box<dyn Fn(f32) -> Message + 'a>>,
}

#[derive(Clone, Copy)]
enum Source {
    Row(usize),
    Project { group: usize, row: usize },
}

struct Drop<Message> {
    message: Message,
    indicator: Rectangle,
}

#[derive(Default)]
struct State {
    drag: Option<Drag>,
}

struct Drag {
    source: Source,
    origin: Point,
    active: bool,
    last_scroll: Instant,
}

fn target(
    bounds: Rectangle,
    rows: impl Iterator<Item = Rectangle>,
    cursor: mouse::Cursor,
    viewport: &Rectangle,
    source: usize,
) -> Option<(usize, f32)> {
    let visible = bounds.intersection(viewport)?;
    let position = cursor.position().filter(|point| visible.contains(*point))?;
    let mut slot = 0;
    let mut edge = bounds.y;
    let mut count = 0;
    for row in rows {
        count += 1;
        if position.y >= row.center_y() {
            slot += 1;
            edge = row.y + row.height;
        } else if slot == count - 1 {
            edge = row.y;
        }
    }
    if source >= count {
        return None;
    }
    Some((slot.saturating_sub(usize::from(slot > source)), edge))
}

impl<Message> Reorder<'_, Message> {
    fn source_at(&self, layout: Layout<'_>, point: Point) -> Option<Source> {
        for (group, entry) in layout.children().enumerate() {
            if !entry.bounds().contains(point) {
                continue;
            }
            let Some(groups) = &self.groups else {
                return (point.x < entry.bounds().x + entry.bounds().width - self.action_width)
                    .then_some(Source::Row(group));
            };
            let heading = entry.children().next()?.bounds();
            if heading.contains(point) {
                return (!(groups.has_pinned && group == 0)
                    && point.x < heading.x + heading.width - self.action_width)
                    .then_some(Source::Row(group));
            }
            if groups
                .project_counts
                .get(group)
                .copied()
                .unwrap_or_default()
                > 0
            {
                return entry
                    .children()
                    .nth(1)?
                    .children()
                    .position(|row| row.bounds().contains(point))
                    .map(|row| Source::Project { group, row });
            }
        }
        None
    }

    fn source_bounds(&self, layout: Layout<'_>, source: Source) -> Option<Rectangle> {
        match source {
            Source::Row(index) => {
                let entry = layout.children().nth(index)?;
                Some(if self.groups.is_some() {
                    entry.children().next()?.bounds()
                } else {
                    entry.bounds()
                })
            }
            Source::Project { group, row } => Some(
                layout
                    .children()
                    .nth(group)?
                    .children()
                    .nth(1)?
                    .children()
                    .nth(row)?
                    .bounds(),
            ),
        }
    }

    fn drop_at(
        &self,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        source: Source,
    ) -> Option<Drop<Message>> {
        let line = |bounds: Rectangle, edge: f32| Rectangle {
            x: bounds.x,
            y: edge - 1.,
            width: bounds.width,
            height: 2.,
        };
        match source {
            Source::Row(source) => {
                let skip =
                    usize::from(self.groups.as_ref().is_some_and(|groups| groups.has_pinned));
                let bounds = layout.bounds();
                let rows = layout.children().skip(skip);
                let bounds = Rectangle {
                    y: layout.children().nth(skip)?.bounds().y,
                    ..bounds
                };
                let bounds = Rectangle {
                    height: layout.bounds().y + layout.bounds().height - bounds.y,
                    ..bounds
                };
                let source = source.checked_sub(skip)?;
                let (destination, edge) = target(
                    bounds,
                    rows.map(|row| row.bounds()),
                    cursor,
                    viewport,
                    source,
                )?;
                (source != destination).then(|| Drop {
                    message: (self.on_move)(source, destination),
                    indicator: line(bounds, edge),
                })
            }
            Source::Project { group, row } => {
                let groups = self.groups.as_ref()?;
                let position = cursor
                    .position()
                    .filter(|point| viewport.contains(*point))?;
                let (destination, entry) =
                    layout.children().enumerate().find(|(index, entry)| {
                        !(groups.has_pinned && *index == 0) && entry.bounds().contains(position)
                    })?;
                let heading = entry.children().next()?.bounds();
                let (slot, indicator) = if group == destination {
                    let projects = entry.children().nth(1)?;
                    let bounds = projects.bounds();
                    let (slot, edge) = target(
                        bounds,
                        projects.children().map(|row| row.bounds()),
                        cursor,
                        viewport,
                        row,
                    )?;
                    if slot == row {
                        return None;
                    }
                    (Some(slot), line(bounds, edge))
                } else {
                    (None, heading)
                };
                Some(Drop {
                    message: (groups.on_project_drop)(group, row, destination, slot)?,
                    indicator,
                })
            }
        }
    }
}

impl<Message> Widget<Message, Theme, Renderer> for Reorder<'_, Message> {
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(State::default())
    }

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
        let state = tree.state.downcast_mut::<State>();
        match event {
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                if let Some(position) = cursor.position()
                    && viewport.contains(position)
                    && let Some(source) = self.source_at(layout, position)
                {
                    state.drag = Some(Drag {
                        source,
                        origin: position,
                        active: false,
                        last_scroll: Instant::now(),
                    });
                    shell.capture_event();
                    return;
                }
            }
            Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                if let Some(drag) = &mut state.drag
                    && let Some(position) = cursor.position()
                {
                    // Scroll containers translate the cursor, but retain window coordinates in events.
                    drag.active |= drag.origin.distance(position) > 6.;
                    if drag.active {
                        shell.request_redraw();
                    }
                    shell.capture_event();
                    return;
                }
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                if let Some(drag) = state.drag.take() {
                    if drag.active {
                        if let Some(drop) = self.drop_at(layout, cursor, viewport, drag.source) {
                            shell.publish(drop.message);
                        }
                        shell.request_redraw();
                        shell.capture_event();
                        return;
                    }
                    if self
                        .source_bounds(layout, drag.source)
                        .is_some_and(|bounds| {
                            cursor.is_over(bounds)
                                && cursor
                                    .position()
                                    .is_some_and(|point| viewport.contains(point))
                        })
                    {
                        let mut messages = Vec::new();
                        let mut press = Shell::new(&mut messages);
                        self.content.as_widget_mut().update(
                            &mut tree.children[0],
                            &Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
                            layout,
                            cursor,
                            renderer,
                            clipboard,
                            &mut press,
                            viewport,
                        );
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
                        shell.merge(press, std::convert::identity);
                        return;
                    }
                }
            }
            Event::Window(window::Event::Unfocused) | Event::Mouse(mouse::Event::CursorLeft) => {
                state.drag = None;
                shell.request_redraw();
            }
            Event::Window(window::Event::RedrawRequested(now)) => {
                if let Some(drag) = &mut state.drag
                    && drag.active
                    && let Some(visible) = layout.bounds().intersection(viewport)
                    && let Some(position) =
                        cursor.position().filter(|point| visible.contains(*point))
                {
                    let bounds = layout.bounds();
                    let speed = if position.y < visible.y + 28. && bounds.y < visible.y {
                        -420.
                    } else if position.y > visible.y + visible.height - 28.
                        && bounds.y + bounds.height > visible.y + visible.height
                    {
                        420.
                    } else {
                        0.
                    };
                    if speed != 0.
                        && let Some(on_scroll) = &self.on_scroll
                    {
                        let elapsed = now
                            .saturating_duration_since(drag.last_scroll)
                            .as_secs_f32()
                            .min(0.05);
                        shell.publish(on_scroll(speed * elapsed));
                        shell.request_redraw_at(*now + Duration::from_millis(16));
                    }
                    drag.last_scroll = *now;
                }
            }
            _ => {}
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
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        if tree
            .state
            .downcast_ref::<State>()
            .drag
            .as_ref()
            .is_some_and(|drag| drag.active)
        {
            mouse::Interaction::Grabbing
        } else {
            self.content.as_widget().mouse_interaction(
                &tree.children[0],
                layout,
                cursor,
                viewport,
                renderer,
            )
        }
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
        let drag = tree
            .state
            .downcast_ref::<State>()
            .drag
            .as_ref()
            .filter(|drag| drag.active);
        self.content.as_widget().draw(
            &tree.children[0],
            renderer,
            theme,
            style,
            layout,
            if drag.is_some() {
                mouse::Cursor::Unavailable
            } else {
                cursor
            },
            viewport,
        );
        if let Some(drag) = drag
            && let Some(clip) = layout.bounds().intersection(viewport)
            && let Some(source) = self.source_bounds(layout, drag.source)
        {
            renderer.with_layer(clip, |renderer| {
                renderer.fill_quad(
                    renderer::Quad {
                        bounds: source,
                        border: Border {
                            color: theme::BLUE.resolve(theme),
                            width: 1.,
                            radius: theme.control_radius().into(),
                        },
                        ..Default::default()
                    },
                    theme::BLUE.resolve(theme).scale_alpha(0.12),
                );
                if let Some(drop) = self.drop_at(layout, cursor, viewport, drag.source) {
                    let mut bounds = drop.indicator;
                    if bounds.height == 2. {
                        bounds.y = bounds
                            .y
                            .clamp(clip.y, (clip.y + clip.height - 2.).max(clip.y));
                    }
                    let color = theme::BLUE.resolve(theme);
                    let background = if bounds.height == 2. {
                        color
                    } else {
                        color.scale_alpha(0.24)
                    };
                    renderer.fill_quad(
                        renderer::Quad {
                            bounds,
                            ..Default::default()
                        },
                        background,
                    );
                }
            });
        }
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'b>,
        renderer: &Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'b, Message, Theme, Renderer>> {
        if tree
            .state
            .downcast_ref::<State>()
            .drag
            .as_ref()
            .is_some_and(|drag| drag.active)
        {
            return None;
        }
        self.content.as_widget_mut().overlay(
            &mut tree.children[0],
            layout,
            renderer,
            viewport,
            translation,
        )
    }
}

#[cfg(test)]
mod tests;
