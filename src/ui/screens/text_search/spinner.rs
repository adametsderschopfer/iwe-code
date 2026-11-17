//! Animate pending searches without status text.
use crate::{app::Message, ui::theme};
use iced::{Element, Point, Rectangle, Renderer, Theme, mouse, widget::canvas};
use std::time::Instant;

pub(super) fn view<'a>() -> Element<'a, Message> {
    canvas::Canvas::new(Spinner).width(16).height(16).into()
}
struct Spinner;
impl canvas::Program<Message> for Spinner {
    type State = Option<Instant>;
    fn update(
        &self,
        state: &mut Self::State,
        event: &canvas::Event,
        _: Rectangle,
        _: mouse::Cursor,
    ) -> Option<canvas::Action<Message>> {
        if let canvas::Event::Window(iced::window::Event::RedrawRequested(now)) = event {
            state.get_or_insert(*now);
            Some(canvas::Action::request_redraw())
        } else {
            None
        }
    }
    fn draw(
        &self,
        state: &Self::State,
        renderer: &Renderer,
        theme: &Theme,
        bounds: Rectangle,
        _: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        let phase = state.map_or(0., |start| start.elapsed().as_secs_f32() * 8.);
        for index in 0..8 {
            let angle = (index as f32 + phase) * std::f32::consts::TAU / 8.;
            let mut color = theme::MUTED.resolve(theme);
            color.a = (index + 1) as f32 / 8.;
            frame.fill(
                &canvas::Path::circle(
                    Point::new(8. + angle.cos() * 5., 8. + angle.sin() * 5.),
                    1.2,
                ),
                color,
            );
        }
        vec![frame.into_geometry()]
    }
}
