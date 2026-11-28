//! Share a selection across the shaped text blocks of a Markdown preview.

use super::Action;
use crate::ui::theme::iced;
use crate::{
    domain::shortcuts::{Command, Shortcuts},
    ui::{shortcuts, theme},
};
use iced::{
    Element, Event, Font, Length, Point, Rectangle, Renderer, Size, Theme, Vector,
    advanced::{
        Clipboard, Layout, Shell, Widget,
        graphics::text::{Paragraph, cosmic_text::Cursor},
        layout, mouse,
        renderer::{self, Renderer as _},
        widget::{Operation, Tree, operation, tree},
    },
    keyboard,
    widget::text::{self, Rich},
    window,
};
use std::{any::Any, cell::Cell, rc::Rc};

type RichText<'a> = Rich<'a, String, Action, Theme>;
type Range = (Position, Position);

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct Position {
    block: usize,
    line: usize,
    byte: usize,
}

#[derive(Default)]
pub(super) struct Shared {
    next: Cell<usize>,
    range: Rc<Cell<Option<Range>>>,
}

impl Shared {
    pub(super) fn block<'a>(
        &self,
        spans: impl AsRef<[text::Span<'a, String, Font>]> + 'a,
        configure: impl FnOnce(RichText<'a>) -> RichText<'a>,
    ) -> Element<'a, Action> {
        let index = self.next.get();
        self.next.set(index + 1);
        Element::new(Block {
            index,
            range: self.range.clone(),
            content: configure(
                Rich::with_spans(spans)
                    .wrapping(text::Wrapping::WordOrGlyph)
                    .on_link_click(Action::Link),
            ),
        })
    }
}

struct Block<'a> {
    index: usize,
    range: Rc<Cell<Option<Range>>>,
    content: RichText<'a>,
}

impl Widget<Action, Theme, Renderer> for Block<'_> {
    fn tag(&self) -> tree::Tag {
        struct Tag;
        tree::Tag::of::<Tag>()
    }

    fn diff(&self, tree: &mut Tree) {
        let content: &dyn Widget<Action, Theme, Renderer> = &self.content;
        tree.diff_children(&[content]);
    }

    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(
            &self.content as &dyn Widget<Action, Theme, Renderer>,
        )]
    }

    fn size(&self) -> Size<Length> {
        Widget::size(&self.content)
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        self.content.layout(&mut tree.children[0], renderer, limits)
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        _renderer: &Renderer,
        operation: &mut dyn Operation,
    ) {
        let paragraph = self.content.paragraph(&tree.children[0]);
        if paragraph.buffer().lines.len() == 1 {
            operation.text(None, layout.bounds(), paragraph.buffer().lines[0].text());
        }
        operation.custom(
            None,
            layout.bounds(),
            &mut Some(BlockGeometry {
                index: self.index,
                bounds: layout.bounds(),
                visible: None,
                paragraph: paragraph.clone(),
            }),
        );
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Action>,
        viewport: &Rectangle,
    ) {
        self.content.update(
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
        self.content.draw(
            &tree.children[0],
            renderer,
            theme,
            style,
            layout,
            cursor,
            viewport,
        );
        let paragraph = self.content.paragraph(&tree.children[0]);
        if let Some((start, end)) = self.range.get()
            && let Some((from, to)) = block_range(self.index, paragraph, start, end)
        {
            for run in paragraph.buffer().layout_runs() {
                if let Some((x, width)) = run.highlight(from, to)
                    && let Some(bounds) = Rectangle::new(
                        layout.position() + Vector::new(x, run.line_top),
                        Size::new(width, run.line_height),
                    )
                    .intersection(viewport)
                {
                    renderer.fill_quad(
                        renderer::Quad {
                            bounds,
                            ..Default::default()
                        },
                        theme::BLUE.resolve(theme).scale_alpha(0.3),
                    );
                }
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
        let link =
            self.content
                .mouse_interaction(&tree.children[0], layout, cursor, viewport, renderer);
        if link == mouse::Interaction::None
            && cursor.is_over(layout.bounds())
            && cursor.is_over(*viewport)
        {
            mouse::Interaction::Text
        } else {
            link
        }
    }
}

struct BlockGeometry {
    index: usize,
    bounds: Rectangle,
    visible: Option<Rectangle>,
    paragraph: Paragraph,
}

#[derive(Default)]
struct Geometry {
    blocks: Vec<BlockGeometry>,
    offset: Vector,
    clip: Option<Rectangle>,
    scroll: Option<(Rectangle, Vector)>,
}

impl Operation for Geometry {
    fn scrollable(
        &mut self,
        _id: Option<&iced::widget::Id>,
        bounds: Rectangle,
        _content_bounds: Rectangle,
        translation: Vector,
        _state: &mut dyn operation::Scrollable,
    ) {
        self.scroll = Some((bounds, translation));
    }

    fn traverse(&mut self, visit: &mut dyn FnMut(&mut dyn Operation)) {
        if let Some((bounds, translation)) = self.scroll.take() {
            let mut nested = Geometry {
                offset: self.offset + translation,
                clip: self
                    .clip
                    .and_then(|clip| clip.intersection(&(bounds - self.offset))),
                ..Self::default()
            };
            visit(&mut nested);
            self.blocks.extend(nested.blocks);
        } else {
            visit(self);
        }
    }

    fn custom(&mut self, _id: Option<&iced::widget::Id>, _bounds: Rectangle, state: &mut dyn Any) {
        if let Some(block) = state.downcast_mut::<Option<BlockGeometry>>()
            && let Some(mut block) = block.take()
        {
            block.bounds = block.bounds - self.offset;
            block.visible = self.clip.and_then(|clip| clip.intersection(&block.bounds));
            self.blocks.push(block);
        }
    }
}

impl Geometry {
    fn hit(&self, pointer: Point, nearest: bool) -> Option<Position> {
        let block = self
            .blocks
            .iter()
            .find(|block| block.visible.is_some_and(|area| area.contains(pointer)))
            .or_else(|| {
                if !nearest {
                    return None;
                }
                self.blocks
                    .iter()
                    .filter_map(|block| {
                        let area = block.visible?;
                        let point = Point::new(
                            pointer.x.clamp(area.x, area.x + area.width),
                            pointer.y.clamp(area.y, area.y + area.height),
                        );
                        Some((block, point.distance(pointer)))
                    })
                    .min_by(|(_, left), (_, right)| left.total_cmp(right))
                    .map(|(block, _)| block)
            })?;
        let local = pointer - block.bounds.position();
        let buffer = block.paragraph.buffer();
        let hit = if local.y < 0. {
            Cursor::default()
        } else if local.y >= block.bounds.height {
            end_cursor(&block.paragraph)?
        } else {
            buffer.hit(local.x, local.y)?
        };
        Some(Position {
            block: block.index,
            line: hit.line,
            byte: hit.index,
        })
    }

    fn all(&self) -> Option<Range> {
        let first = self.blocks.iter().min_by_key(|block| block.index)?;
        let last = self.blocks.iter().max_by_key(|block| block.index)?;
        let end = end_cursor(&last.paragraph)?;
        Some((
            Position {
                block: first.index,
                line: 0,
                byte: 0,
            },
            Position {
                block: last.index,
                line: end.line,
                byte: end.index,
            },
        ))
    }

    fn copy(&mut self, (start, end): Range) -> String {
        self.blocks.sort_unstable_by_key(|block| block.index);
        self.blocks
            .iter()
            .filter_map(|block| {
                let (from, to) = block_range(block.index, &block.paragraph, start, end)?;
                let lines = &block.paragraph.buffer().lines;
                let mut selected = String::new();
                for line in from.line..=to.line {
                    let value = lines.get(line)?.text();
                    let start = if line == from.line { from.index } else { 0 };
                    let end = if line == to.line {
                        to.index
                    } else {
                        value.len()
                    };
                    selected.push_str(value.get(start..end)?);
                    if line < to.line {
                        selected.push('\n');
                    }
                }
                Some(selected)
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}

fn end_cursor(paragraph: &Paragraph) -> Option<Cursor> {
    let lines = &paragraph.buffer().lines;
    Some(Cursor::new(
        lines.len().checked_sub(1)?,
        lines.last()?.text().len(),
    ))
}

fn block_range(
    index: usize,
    paragraph: &Paragraph,
    start: Position,
    end: Position,
) -> Option<(Cursor, Cursor)> {
    if start == end || index < start.block || index > end.block {
        return None;
    }
    let from = if index == start.block {
        Cursor::new(start.line, start.byte)
    } else {
        Cursor::default()
    };
    let to = if index == end.block {
        Cursor::new(end.line, end.byte)
    } else {
        end_cursor(paragraph)?
    };
    Some((from, to))
}

pub(super) fn document<'a>(
    content: Element<'a, Action>,
    selection: Shared,
    identity: (u64, u64),
    shortcuts: Shortcuts,
) -> Element<'a, Action> {
    Element::new(Document {
        content,
        selection,
        identity,
        shortcuts,
    })
}

struct Document<'a> {
    content: Element<'a, Action>,
    selection: Shared,
    identity: (u64, u64),
    shortcuts: Shortcuts,
}

#[derive(Default)]
struct State {
    identity: (u64, u64),
    focused: bool,
    press: Option<(Point, Position)>,
    selection: Option<Range>,
}

impl Widget<Action, Theme, Renderer> for Document<'_> {
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(State {
            identity: self.identity,
            ..State::default()
        })
    }

    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(&self.content)]
    }

    fn diff(&self, tree: &mut Tree) {
        let state = tree.state.downcast_mut::<State>();
        if state.identity != self.identity {
            *state = State {
                identity: self.identity,
                ..State::default()
            };
        }
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
        self.selection
            .range
            .set(tree.state.downcast_ref::<State>().selection);
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

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        mut cursor: mouse::Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Action>,
        viewport: &Rectangle,
    ) {
        let state = tree.state.downcast_mut::<State>();
        let child = &mut tree.children[0];
        let mut geometry = || {
            let mut geometry = Geometry {
                clip: viewport.intersection(&layout.bounds()),
                ..Geometry::default()
            };
            self.content
                .as_widget_mut()
                .operate(child, layout, renderer, &mut geometry);
            geometry
        };
        match event {
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                state.press = cursor
                    .position_over(layout.bounds())
                    .and_then(|pointer| geometry().hit(pointer, false).map(|hit| (pointer, hit)));
                state.focused = state.press.is_some();
                if state.selection.take().is_some() {
                    shell.request_redraw();
                }
            }
            Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                if let Some((origin, anchor)) = state.press
                    && let Some(pointer) = cursor.position()
                    && (state.selection.is_some() || origin.distance(pointer) >= 3.)
                    && let Some(end) = geometry().hit(pointer, true)
                {
                    state.selection = Some((anchor.min(end), anchor.max(end)));
                    shell.request_redraw();
                    shell.capture_event();
                    return;
                }
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                if state.press.take().is_some() && state.selection.is_some() {
                    // Let a pressed link reset without activating it after a drag.
                    cursor = mouse::Cursor::Unavailable;
                    shell.capture_event();
                }
            }
            Event::Window(window::Event::Unfocused) | Event::Mouse(mouse::Event::CursorLeft) => {
                state.press = None;
            }
            Event::Keyboard(keyboard::Event::KeyPressed {
                key,
                physical_key,
                modifiers,
                ..
            }) if state.focused => {
                let matches = |command| {
                    shortcuts::matches(
                        &self.shortcuts.binding(command),
                        key,
                        *physical_key,
                        *modifiers,
                    )
                };
                if matches(Command::Copy) {
                    if let Some(range) = state.selection {
                        let text = geometry().copy(range);
                        if !text.is_empty() {
                            shell.publish(Action::Copy(text));
                            shell.capture_event();
                        }
                    }
                } else if matches(Command::SelectAll) {
                    state.selection = geometry().all();
                    shell.capture_event();
                    shell.request_redraw();
                } else if *key == keyboard::Key::Named(keyboard::key::Named::Escape)
                    && state.selection.take().is_some()
                {
                    state.press = None;
                    shell.capture_event();
                    shell.request_redraw();
                }
            }
            _ => {}
        }
        self.content.as_widget_mut().update(
            child, event, layout, cursor, renderer, clipboard, shell, viewport,
        );
    }
}
