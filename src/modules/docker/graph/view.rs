//! Draw visible resources and keep the camera and manual node positions across refreshes.
use super::{Data, Id, Node, Project};
use crate::{
    i18n::{I18n, Key},
    modules::docker::{Event, Kind},
    ui::{context_menu, ellipsis, icons::Icon, presentation::short, theme},
};
use iced::{
    Alignment, Element, Length, Point, Rectangle, Renderer, Size, Theme, Vector, mouse,
    widget::{
        button,
        canvas::{self, Frame, Geometry, Path, Stroke},
        column, container, row, text,
    },
};
use std::collections::BTreeMap;

#[derive(Debug, Clone)]
pub(crate) enum Interaction {
    Select(Option<Id>),
    Pan(Vector),
    Move(Id, Vector),
    Zoom(f32, Point),
    Reset,
}
#[derive(Debug)]
pub(crate) struct Viewport {
    pub selected: Option<Id>,
    pan: Vector,
    zoom: f32,
    moved: BTreeMap<Id, Vector>,
}
impl Default for Viewport {
    fn default() -> Self {
        Self {
            selected: None,
            pan: Vector::ZERO,
            zoom: 1.,
            moved: BTreeMap::new(),
        }
    }
}
impl Viewport {
    pub(in crate::modules::docker) fn update(&mut self, event: Interaction) {
        match event {
            Interaction::Select(id) => self.selected = id,
            Interaction::Pan(delta) => self.pan += delta,
            Interaction::Move(id, delta) => {
                let old = self.moved.entry(id).or_default();
                *old += delta;
            }
            Interaction::Zoom(factor, at) => {
                let zoom = (self.zoom * factor).clamp(0.25, 4.);
                let ratio = zoom / self.zoom;
                self.pan = (self.pan - Vector::new(at.x, at.y)) * ratio + Vector::new(at.x, at.y);
                self.zoom = zoom;
            }
            Interaction::Reset => {
                self.pan = Vector::ZERO;
                self.zoom = 1.;
                self.moved.clear();
            }
        }
    }
    pub(in crate::modules::docker) fn reconcile(&mut self, data: &Data) {
        let contains = |key: &Id| data.nodes.binary_search_by(|node| node.id.cmp(key)).is_ok();
        if self.selected.as_ref().is_some_and(|key| !contains(key)) {
            self.selected = None;
        }
        self.moved.retain(|key, _| contains(key));
    }
    fn scale(&self, bounds: Rectangle) -> f32 {
        self.zoom * (bounds.width / 710.).clamp(0.45, 1.)
    }
    fn rect(&self, node: &Node, bounds: Rectangle) -> Rectangle {
        let scale = self.scale(bounds);
        let offset = self.moved.get(&node.id).copied().unwrap_or_default();
        Rectangle {
            x: (20. + node.id.lane() as f32 * 237. + offset.x) * scale + self.pan.x,
            y: (18. + node.slot as f32 * 65. + offset.y) * scale + self.pan.y,
            width: 185. * scale,
            height: 44. * scale,
        }
    }
}

pub(in crate::modules::docker) fn panel<'a>(
    project: &'a Project,
    busy: bool,
    i18n: &'a I18n,
) -> Element<'a, Event> {
    let tool = |symbol, key, message| super::super::view::tool(i18n, symbol, key, Some(message));
    let mut args = crate::i18n::FluentArgs::new();
    args.set("project", project.name.as_str());
    let title = ellipsis::text(i18n.format(Key::DockerProjectGraph, &args), 14.);
    let mut header = row![container(title).width(Length::Fill)]
        .align_y(Alignment::Center)
        .spacing(4);
    if let Some(key) = &project.viewport.selected {
        header = header.push(
            button(text(i18n.text(Key::DockerOpenResource)).size(12))
                .on_press_maybe((!busy).then(|| Event::OpenNode(key.clone())))
                .padding([3, 6])
                .style(super::super::view::style::button(theme::secondary)),
        );
    }
    header = header
        .push(tool(
            Icon::Minus,
            Key::DockerZoomOut,
            Event::Diagram(Interaction::Zoom(0.8, Point::ORIGIN)),
        ))
        .push(tool(
            Icon::Plus,
            Key::DockerZoomIn,
            Event::Diagram(Interaction::Zoom(1.25, Point::ORIGIN)),
        ))
        .push(tool(
            Icon::Locate,
            Key::DockerResetGraph,
            Event::Diagram(Interaction::Reset),
        ))
        .push(super::super::view::tool(
            i18n,
            Icon::Refresh,
            Key::DockerRefresh,
            (!busy).then_some(Event::Refresh),
        ))
        .push(
            container(tool(Icon::Close, Key::Dismiss, Event::Dismiss))
                .id("docker-close-project-graph"),
        );
    let body: Element<'a, Event> = if let Some(error) = project.error {
        container(text(i18n.text(super::super::view::error_key(error))).size(13))
            .center(Length::Fill)
            .into()
    } else if project.data.nodes.is_empty() {
        container(
            text(i18n.text(if busy {
                Key::Loading
            } else {
                Key::DockerNoProjectContainers
            }))
            .size(13),
        )
        .center(Length::Fill)
        .into()
    } else {
        container(
            canvas::Canvas::new(Drawing {
                data: &project.data,
                camera: &project.viewport,
                i18n,
            })
            .width(Length::Fill)
            .height(Length::Fill),
        )
        .id("docker-graph-canvas")
        .into()
    };
    column![
        container(header)
            .padding([0, 8])
            .height(34)
            .align_y(Alignment::Center),
        context_menu::separator(),
        body
    ]
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

struct Drawing<'a> {
    data: &'a Data,
    camera: &'a Viewport,
    i18n: &'a I18n,
}
impl Drawing<'_> {
    fn hit(&self, bounds: Rectangle, at: Point) -> Option<&Node> {
        self.data
            .nodes
            .iter()
            .rev()
            .find(|node| self.camera.rect(node, bounds).contains(at))
    }
}
impl canvas::Program<Event> for Drawing<'_> {
    type State = Option<(Point, Option<Id>)>;
    fn update(
        &self,
        drag: &mut Self::State,
        event: &canvas::Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas::Action<Event>> {
        if matches!(
            event,
            canvas::Event::Mouse(
                mouse::Event::ButtonReleased(mouse::Button::Left) | mouse::Event::CursorLeft
            ) | canvas::Event::Window(iced::window::Event::Unfocused)
        ) {
            return drag.take().map(|_| canvas::Action::request_redraw());
        }
        let at = cursor.position_in(bounds)?;
        let msg = match event {
            canvas::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                let key = self.hit(bounds, at).map(|n| n.id.clone());
                *drag = Some((at, key.clone()));
                Interaction::Select(key)
            }
            canvas::Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                let (old, key) = drag.as_mut()?;
                let delta = at - *old;
                *old = at;
                match key {
                    Some(key) => {
                        Interaction::Move(key.clone(), delta * (1. / self.camera.scale(bounds)))
                    }
                    None => Interaction::Pan(delta),
                }
            }
            canvas::Event::Mouse(mouse::Event::WheelScrolled { delta }) => {
                let amount = match delta {
                    mouse::ScrollDelta::Lines { y, .. } => *y * 0.14,
                    mouse::ScrollDelta::Pixels { y, .. } => *y * 0.003,
                };
                Interaction::Zoom(amount.clamp(-1., 1.).exp(), at)
            }
            _ => return None,
        };
        Some(canvas::Action::publish(Event::Diagram(msg)).and_capture())
    }
    fn mouse_interaction(
        &self,
        drag: &Self::State,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> mouse::Interaction {
        if drag.is_some() {
            mouse::Interaction::Grabbing
        } else if cursor.is_over(bounds) {
            mouse::Interaction::Grab
        } else {
            mouse::Interaction::default()
        }
    }
    fn draw(
        &self,
        _: &Self::State,
        renderer: &Renderer,
        theme: &Theme,
        bounds: Rectangle,
        _: mouse::Cursor,
    ) -> Vec<Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());
        let area = Rectangle::with_size(bounds.size());
        frame.with_clip(area, |frame| {
            for &(a, b) in &self.data.edges {
                let (a, b) = (&self.data.nodes[a], &self.data.nodes[b]);
                let (ra, rb) = (self.camera.rect(a, bounds), self.camera.rect(b, bounds));
                let start = Point::new(ra.x + ra.width, ra.center_y());
                let end = Point::new(rb.x, rb.center_y());
                let mid = (start.x + end.x) / 2.;
                if Rectangle::new(
                    Point::new(start.x.min(end.x) - 1., start.y.min(end.y) - 1.),
                    Size::new((end.x - start.x).abs() + 2., (end.y - start.y).abs() + 2.),
                )
                .intersection(&area)
                .is_none()
                {
                    continue;
                }
                let active = self
                    .camera
                    .selected
                    .as_ref()
                    .is_some_and(|s| s == &a.id || s == &b.id);
                let line = Path::new(|p| {
                    p.move_to(start);
                    p.bezier_curve_to(Point::new(mid, start.y), Point::new(mid, end.y), end);
                });
                frame.stroke(
                    &line,
                    Stroke::default()
                        .with_width(if active { 2. } else { 1. })
                        .with_color(if active { theme::BLUE } else { theme::MUTED }.resolve(theme)),
                );
            }
            let scale = self.camera.scale(bounds);
            for node in &self.data.nodes {
                let rect = self.camera.rect(node, bounds);
                if rect.intersection(&area).is_none() {
                    continue;
                }
                let color = match node.id.0 {
                    Kind::Containers => theme::GREEN,
                    Kind::Images => theme::BLUE,
                    Kind::Networks => theme::PURPLE,
                    Kind::Volumes => theme::YELLOW,
                    Kind::Compose => theme::PINK,
                }
                .resolve(theme);
                frame.fill_rectangle(rect.position(), rect.size(), theme::code_background(theme));
                let selected = self.camera.selected.as_ref() == Some(&node.id);
                frame.stroke_rectangle(
                    rect.position(),
                    rect.size(),
                    Stroke::default()
                        .with_width(if selected { 2. } else { 1. })
                        .with_color(if selected {
                            color
                        } else {
                            theme::LINE.resolve(theme)
                        }),
                );
                frame.fill_rectangle(rect.position(), Size::new(3. * scale, rect.height), color);
                for (label, y, size, tone) in [
                    (self.i18n.text(node.id.0.key()), 4., 10., color),
                    (short(&node.name, 22), 19., 13., theme::TEXT.resolve(theme)),
                ] {
                    frame.fill_text(canvas::Text {
                        content: label,
                        position: Point::new(rect.x + 9. * scale, rect.y + y * scale),
                        size: (size * scale).into(),
                        color: tone,
                        font: theme::font(),
                        ..Default::default()
                    });
                }
            }
        });
        vec![frame.into_geometry()]
    }
}
