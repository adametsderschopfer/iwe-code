//! Place color controls alongside visible literals and edit them in an anchored popup.

use crate::ui::theme::iced;
use crate::{
    app::{
        App, Message,
        editor::{
            self,
            color::{Control, Event, Picker},
        },
    },
    domain::editor::colors::{self, Rgba},
    i18n::Key,
    ui::{context_menu, modal, theme},
};
use iced::{
    Color, Element, Length, Point, Rectangle, Renderer, Size, Theme, mouse,
    widget::{button, canvas, column, row, text, text_editor, tooltip},
};

fn message(event: Event) -> Message {
    Message::Editor(editor::Event::Color(event))
}

fn iced_color(rgba: Rgba) -> Color {
    let [red, green, blue, alpha] = rgba.0;
    Color::from_rgba8(red, green, blue, f32::from(alpha) / 255.)
}

pub(super) struct Markers<'a> {
    pub content: &'a text_editor::Content,
    pub tab: u64,
}

impl Markers<'_> {
    fn visible(
        &self,
        bounds: Rectangle,
        hit_y: Option<f32>,
        mut visit: impl FnMut(Rectangle, text_editor::Position, Rgba),
    ) {
        use super::TEXT_PADDING as PAD;
        self.content.with_editor(|editor| {
            let buffer = editor.buffer();
            let scroll = buffer.scroll().horizontal;
            for run in buffer.layout_runs() {
                let y = PAD + run.line_top;
                if y + run.line_height <= PAD
                    || y >= bounds.height - PAD
                    || hit_y.is_some_and(|point| !(y..y + run.line_height).contains(&point))
                    || !run.text.contains(['#', '('])
                {
                    continue;
                }
                let visible: Vec<_> = run
                    .glyphs
                    .iter()
                    .filter(|glyph| {
                        glyph.x + glyph.w > scroll - PAD && glyph.x < scroll + bounds.width
                    })
                    .collect();
                let Some(start) = visible.iter().map(|glyph| glyph.start).min() else {
                    continue;
                };
                let Some(end) = visible.iter().map(|glyph| glyph.end).max() else {
                    continue;
                };
                let from = (start.saturating_sub(100)..=start)
                    .find(|&index| run.text.is_char_boundary(index))
                    .unwrap_or(start);
                let to = (end..=(end + 100).min(run.text.len()))
                    .rev()
                    .find(|&index| run.text.is_char_boundary(index))
                    .unwrap_or(end);
                for (range, color) in colors::literals(&run.text[from..to]) {
                    let start = from + range.start;
                    let end = from + range.end;
                    let mut right = f32::NEG_INFINITY;
                    for glyph in visible
                        .iter()
                        .filter(|glyph| glyph.start < end && glyph.end > start)
                    {
                        right = right.max(glyph.x + glyph.w - scroll + PAD);
                    }
                    if !right.is_finite() {
                        continue;
                    }
                    // Leave trailing punctuation visible and use the first free space after it.
                    let punctuation = run.text[end..]
                        .bytes()
                        .take_while(|byte| matches!(byte, b';' | b',' | b')' | b'\'' | b'"'))
                        .count();
                    let edge = end + punctuation;
                    let mut next = bounds.width - PAD;
                    let mut tail = right;
                    for glyph in &visible {
                        let x = glyph.x - scroll + PAD;
                        if glyph.start >= end && glyph.end <= edge {
                            tail = tail.max(x + glyph.w);
                        }
                        if x >= right
                            && glyph.start >= edge
                            && !run.text[glyph.start..glyph.end]
                                .chars()
                                .all(char::is_whitespace)
                        {
                            next = next.min(x);
                        }
                    }
                    let size = (run.line_height * 0.57).clamp(6., 13.);
                    let area = if tail + 4. + size <= next {
                        Rectangle {
                            x: tail + 4.,
                            y: y + (run.line_height - size) / 2.,
                            width: size,
                            height: size,
                        }
                    } else {
                        Rectangle {
                            x: (right - size).max(PAD),
                            y: y + run.line_height - 4.,
                            width: size,
                            height: 4.,
                        }
                    };
                    if area.x >= PAD
                        && area.x + area.width <= bounds.width - PAD
                        && area.y >= PAD
                        && area.y + area.height <= bounds.height - PAD
                    {
                        visit(
                            area,
                            text_editor::Position {
                                line: run.line_i,
                                column: start,
                            },
                            color,
                        );
                    }
                }
            }
        });
    }

    fn hit(
        &self,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<(Rectangle, text_editor::Position)> {
        let point = cursor.position_in(bounds)?;
        let mut hit = None;
        self.visible(bounds, Some(point.y), |area, position, _| {
            if area.contains(point) {
                hit = Some((area, position));
            }
        });
        hit
    }
}

impl canvas::Program<Message, Theme> for Markers<'_> {
    type State = ();

    fn update(
        &self,
        _: &mut (),
        event: &canvas::Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas::Action<Message>> {
        if !matches!(
            event,
            canvas::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
        ) {
            return None;
        }
        let (area, position) = self.hit(bounds, cursor)?;
        let anchor = Point::new(bounds.x + area.x, bounds.y + area.y + area.height + 5.);
        Some(
            canvas::Action::publish(message(Event::Open(self.tab, anchor, position))).and_capture(),
        )
    }

    fn mouse_interaction(
        &self,
        _: &(),
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> mouse::Interaction {
        if self.hit(bounds, cursor).is_some() {
            mouse::Interaction::Pointer
        } else {
            mouse::Interaction::None
        }
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
            self.visible(bounds, None, |area, _, color| {
                swatch(frame, area, color, theme)
            });
        });
        vec![frame.into_geometry()]
    }
}

fn checkerboard(frame: &mut canvas::Frame, area: Rectangle, theme: &Theme) {
    let side = 5.;
    let light = theme::TEXT.resolve(theme);
    let dark = theme::MUTED.resolve(theme);
    for y in 0..(area.height / side).ceil() as usize {
        for x in 0..(area.width / side).ceil() as usize {
            let (left, top) = (x as f32 * side, y as f32 * side);
            frame.fill_rectangle(
                Point::new(area.x + left, area.y + top),
                Size::new(side.min(area.width - left), side.min(area.height - top)),
                if (x + y) % 2 == 0 { light } else { dark },
            );
        }
    }
}

fn swatch(frame: &mut canvas::Frame, area: Rectangle, rgba: Rgba, theme: &Theme) {
    let point = Point::new(area.x, area.y);
    let color = iced_color(rgba);
    if rgba.0[3] < 255 {
        checkerboard(frame, area, theme);
    }
    frame.fill_rectangle(point, area.size(), color);
    frame.stroke_rectangle(
        point,
        area.size(),
        canvas::Stroke::default()
            .with_color(theme::FAINT.resolve(theme))
            .with_width(1.),
    );
}

struct Sample(Rgba);

impl canvas::Program<Message, Theme> for Sample {
    type State = ();

    fn draw(
        &self,
        _: &(),
        renderer: &Renderer,
        theme: &Theme,
        bounds: Rectangle,
        _: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        swatch(
            &mut frame,
            Rectangle {
                x: 1.,
                y: 1.,
                width: bounds.width - 2.,
                height: bounds.height - 2.,
            },
            self.0,
            theme,
        );
        vec![frame.into_geometry()]
    }
}

struct Track<'a> {
    control: Control,
    picker: &'a Picker,
}

impl canvas::Program<Message, Theme> for Track<'_> {
    type State = bool;

    fn update(
        &self,
        dragging: &mut bool,
        event: &canvas::Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas::Action<Message>> {
        match event {
            canvas::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
                if cursor.is_over(bounds) =>
            {
                *dragging = true
            }
            canvas::Event::Mouse(mouse::Event::CursorMoved { .. }) if *dragging => {}
            canvas::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left))
                if *dragging =>
            {
                *dragging = false;
                return Some(canvas::Action::capture());
            }
            canvas::Event::Mouse(mouse::Event::CursorLeft) => {
                *dragging = false;
                return None;
            }
            _ => return None,
        }
        let point = cursor.position_from(Point::new(bounds.x, bounds.y))?;
        let x = (point.x / bounds.width).clamp(0., 1.);
        let y = (point.y / bounds.height).clamp(0., 1.);
        Some(canvas::Action::publish(message(Event::Pick(self.control, x, y))).and_capture())
    }

    fn mouse_interaction(
        &self,
        _: &bool,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> mouse::Interaction {
        if cursor.is_over(bounds) {
            mouse::Interaction::Crosshair
        } else {
            mouse::Interaction::None
        }
    }

    fn draw(
        &self,
        _: &bool,
        renderer: &Renderer,
        theme: &Theme,
        bounds: Rectangle,
        _: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        use canvas::gradient::Linear;
        let [hue, saturation, value] = self.picker.hsv;
        let width = bounds.width;
        let height = bounds.height;
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        let horizontal = Linear::new(Point::ORIGIN, Point::new(width, 0.));
        let x = match self.control {
            Control::SaturationValue => {
                let color = iced_color(Rgba::from_hsv([hue, 1., 1.], 255));
                frame.fill_rectangle(
                    Point::ORIGIN,
                    bounds.size(),
                    horizontal.add_stop(0., Color::WHITE).add_stop(1., color),
                );
                let vertical = Linear::new(Point::ORIGIN, Point::new(0., height))
                    .add_stop(
                        0.,
                        Color {
                            a: 0.,
                            ..Color::BLACK
                        },
                    )
                    .add_stop(1., Color::BLACK);
                frame.fill_rectangle(Point::ORIGIN, bounds.size(), vertical);
                saturation
            }
            Control::Hue => {
                let gradient = (0..=6).fold(horizontal, |gradient, index| {
                    let offset = index as f32 / 6.;
                    gradient.add_stop(offset, iced_color(Rgba::from_hsv([offset, 1., 1.], 255)))
                });
                frame.fill_rectangle(Point::ORIGIN, bounds.size(), gradient);
                hue
            }
            Control::Opacity => {
                checkerboard(&mut frame, Rectangle::with_size(bounds.size()), theme);
                let color = iced_color(self.picker.rgba);
                frame.fill_rectangle(
                    Point::ORIGIN,
                    bounds.size(),
                    horizontal
                        .add_stop(0., Color { a: 0., ..color })
                        .add_stop(1., Color { a: 1., ..color }),
                );
                color.a
            }
        };
        let radius = 5.;
        let y = if matches!(self.control, Control::SaturationValue) {
            (1. - value) * height
        } else {
            height / 2.
        };
        let indicator = canvas::Path::circle(
            Point::new(
                (x * width).clamp(radius + 1., (width - radius - 1.).max(radius + 1.)),
                y.clamp(radius + 1., height - radius - 1.),
            ),
            radius,
        );
        frame.stroke(
            &indicator,
            canvas::Stroke::default()
                .with_color(Color::BLACK)
                .with_width(3.),
        );
        frame.stroke(
            &indicator,
            canvas::Stroke::default()
                .with_color(Color::WHITE)
                .with_width(1.5),
        );
        vec![frame.into_geometry()]
    }
}

pub(super) fn overlay<'a>(app: &'a App, base: Element<'a, Message>) -> Element<'a, Message> {
    let Some(picker) = app.color_picker() else {
        return base;
    };
    context_menu::overlay(
        base,
        picker.point,
        Size::new(350., 380.),
        || {
            let valid = picker.invalid.is_none();
            let field = |index: usize, key| {
                theme::labeled_field(
                    app.i18n.text(key),
                    theme::compact_field("", &picker.fields[index])
                        .id(format!("editor-color-{index}"))
                        .on_input(move |value| message(Event::Field(index, value)))
                        .on_submit_maybe(valid.then(|| message(Event::Apply)))
                        .style(theme::validated_input(picker.invalid == Some(index)))
                        .width(match index {
                            4 => Length::Fill,
                            3 => Length::Fixed(57.),
                            _ => Length::Fixed(45.),
                        }),
                )
            };
            let track = |control, height| {
                canvas(Track { control, picker })
                    .width(Length::Fill)
                    .height(height)
            };
            let mut palette = row![].spacing(4);
            for &color in &picker.swatches {
                let label = color.css("");
                palette = palette.push(
                    tooltip(
                        button(canvas(Sample(color)).width(19).height(19))
                            .padding(2)
                            .style(theme::secondary)
                            .on_press(message(Event::Swatch(color))),
                        text(label).size(12),
                        tooltip::Position::Bottom,
                    )
                    .style(theme::tooltip),
                );
            }
            crate::ui::focus::form(
                column![]
                    .spacing(modal::CONTENT_GAP)
                    .push(track(Control::SaturationValue, 124))
                    .push(
                        row![
                            canvas(Sample(picker.rgba)).width(48).height(38),
                            column![track(Control::Hue, 15), track(Control::Opacity, 15)]
                                .spacing(8),
                        ]
                        .spacing(12),
                    )
                    .push(
                        row![
                            field(0, Key::EditorColorRed),
                            field(1, Key::EditorColorGreen),
                            field(2, Key::EditorColorBlue),
                            field(3, Key::EditorColorAlpha),
                            field(4, Key::EditorColorHex),
                        ]
                        .spacing(6),
                    )
                    .push(palette)
                    .push(
                        modal::actions()
                            .push(
                                theme::text_button(app.i18n.text(Key::Cancel))
                                    .on_press(message(Event::Close)),
                            )
                            .push(
                                theme::text_button(app.i18n.text(Key::EditorColorApply))
                                    .style(theme::primary)
                                    .on_press_maybe(valid.then(|| message(Event::Apply))),
                            ),
                    )
                    .into(),
            )
        },
        message(Event::Close),
    )
}
