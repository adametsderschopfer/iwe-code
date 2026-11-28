//! Include the canvas in Iced's keyboard focus operations.

use super::{Event, Screen, State};
use crate::ui::theme::iced;
use iced::{
    Element, Length, Rectangle, Renderer, Size, Theme,
    advanced::{
        Clipboard, Layout, Shell, Widget, layout, mouse, renderer,
        widget::{Operation, Tree, operation::focusable::Focusable, tree},
    },
    widget::{Id, canvas::Canvas},
};

pub(super) fn view(screen: Screen<'_>) -> Element<'_, Event> {
    Element::new(FocusableScreen {
        request: screen.request,
        id: Id::from(super::super::state::screen_id(screen.request)),
        canvas: Canvas::new(screen).width(Length::Fill).height(Length::Fill),
    })
}

struct FocusableScreen<'a> {
    request: u64,
    id: Id,
    canvas: Canvas<Screen<'a>, Event, Theme>,
}

impl Focusable for State {
    fn is_focused(&self) -> bool {
        self.focused
    }
    fn focus(&mut self) {
        self.focused = true;
    }
    fn unfocus(&mut self) {
        self.focused = false;
    }
}

impl Widget<Event, Theme, Renderer> for FocusableScreen<'_> {
    fn tag(&self) -> tree::Tag {
        self.canvas.tag()
    }
    fn state(&self) -> tree::State {
        self.canvas.state()
    }
    fn size(&self) -> Size<Length> {
        self.canvas.size()
    }
    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        self.canvas.layout(tree, renderer, limits)
    }
    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        _: &Renderer,
        operation: &mut dyn Operation,
    ) {
        let state = tree.state.downcast_mut::<State>();
        state.switch_to(self.request);
        operation.focusable(Some(&self.id), layout.bounds(), state);
    }
    fn update(
        &mut self,
        tree: &mut Tree,
        event: &iced::Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Event>,
        viewport: &Rectangle,
    ) {
        self.canvas.update(
            tree, event, layout, cursor, renderer, clipboard, shell, viewport,
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
        self.canvas
            .mouse_interaction(tree, layout, cursor, viewport, renderer)
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
        self.canvas
            .draw(tree, renderer, theme, style, layout, cursor, viewport);
    }
}
