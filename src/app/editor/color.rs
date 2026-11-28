//! Keep the draft color tied to the exact document snapshot it came from.

use super::{App, Buffer, Message};
use crate::ui::theme::iced;
use crate::{
    domain::editor::colors::{self, Rgba},
    i18n::Key,
};
use iced::{Point, Task, advanced::graphics::text::cosmic_text::LineIter, widget::text_editor};
use std::{ops::Range, sync::Arc};

#[derive(Debug, Clone, Copy)]
pub(crate) enum Control {
    SaturationValue,
    Hue,
    Opacity,
}

#[derive(Debug, Clone)]
pub(crate) enum Event {
    Open(u64, Point, text_editor::Position),
    Pick(Control, f32, f32),
    Field(usize, String),
    Swatch(Rgba),
    Apply,
    Close,
}

pub(crate) struct Picker {
    pub point: Point,
    pub fields: [String; 5],
    pub invalid: Option<usize>,
    pub rgba: Rgba,
    pub hsv: [f32; 3],
    pub swatches: Vec<Rgba>,
    tab: u64,
    position: text_editor::Position,
    snapshot: Arc<String>,
    range: Range<usize>,
    original: Rgba,
}

pub(crate) fn literal_at_cursor(buffer: &Buffer) -> Option<text_editor::Position> {
    let mut position = buffer.content.cursor().position;
    let line = buffer.content.line(position.line)?;
    position.column = colors::literals(&line.text)
        .find(|(range, _)| (range.start..=range.end).contains(&position.column))?
        .0
        .start;
    Some(position)
}

impl Picker {
    fn new(
        tab: u64,
        point: Point,
        position: text_editor::Position,
        snapshot: Arc<String>,
    ) -> Option<Self> {
        let (line, _) = LineIter::new(&snapshot).nth(position.line)?;
        let offset = line.start;
        let line = &snapshot[line];
        let (range, rgba) =
            colors::literals(line).find(|(range, _)| range.start == position.column)?;
        let mut swatches = Vec::new();
        let palette_end = (0..=65536.min(snapshot.len()))
            .rev()
            .find(|&index| snapshot.is_char_boundary(index))
            .unwrap_or(0);
        for (_, other) in snapshot[..palette_end].lines().flat_map(colors::literals) {
            if !swatches.contains(&other) {
                swatches.push(other);
                if swatches.len() == 10 {
                    break;
                }
            }
        }
        let mut picker = Self {
            point,
            fields: Default::default(),
            invalid: None,
            rgba,
            hsv: rgba.hsv(),
            swatches,
            tab,
            position,
            snapshot,
            range: offset + range.start..offset + range.end,
            original: rgba,
        };
        picker.update_fields();
        Some(picker)
    }

    fn update_fields(&mut self) {
        let [r, g, b, alpha] = self.rgba.0;
        let a = f32::from(alpha) * 100. / 255.;
        self.fields = [
            r.to_string(),
            g.to_string(),
            b.to_string(),
            format!("{a:.1}"),
            self.rgba.css(""),
        ];
        self.invalid = None;
    }

    fn set_color(&mut self, rgba: Rgba) {
        if rgba.0[..3] != self.rgba.0[..3] {
            let [h, s, v] = rgba.hsv();
            self.hsv = [if s == 0. { self.hsv[0] } else { h }, s, v];
        }
        self.rgba = rgba;
        self.update_fields();
    }

    fn field(&mut self, index: usize, value: String) {
        let Some(field) = self.fields.get_mut(index) else {
            return;
        };
        let value: String = value.chars().take(32).collect();
        *field = value.clone();
        let color = if index == 4 {
            Rgba::hex(value.trim())
        } else {
            let mut rgba = self.rgba;
            let limit = if index == 3 { 100. } else { 255. };
            colors::number(&value)
                .filter(|number| (0. ..=limit).contains(number))
                .map(|number| {
                    rgba.0[index] = (number * 255. / limit).round() as u8;
                    rgba
                })
        };
        if let Some(rgba) = color {
            self.set_color(rgba);
        } else {
            self.update_fields();
            self.invalid = Some(index);
        }
        self.fields[index] = value;
    }

    fn pick(&mut self, control: Control, x: f32, y: f32) {
        match control {
            Control::SaturationValue => {
                self.hsv[1] = x;
                self.hsv[2] = 1. - y;
            }
            Control::Hue => self.hsv[0] = x,
            Control::Opacity => {
                self.rgba.0[3] = (255. * x).round() as u8;
                self.update_fields();
                return;
            }
        }
        self.rgba = Rgba::from_hsv(self.hsv, self.rgba.0[3]);
        self.update_fields();
    }
}

impl App {
    pub(crate) fn color_picker(&self) -> Option<&Picker> {
        let picker = self.editor.color.as_ref()?;
        let buffer = self.editor_buffer()?;
        (self.preview().id == picker.tab
            && self.navigation != crate::app::Navigation::Settings
            && Arc::ptr_eq(&buffer.document.text, &picker.snapshot))
        .then_some(picker)
    }

    pub(in crate::app) fn dismiss_stale_color_picker(&mut self) {
        if self.editor.color.is_some() && self.color_picker().is_none() {
            self.editor.color = None;
        }
    }

    pub(super) fn update_editor_color(&mut self, event: Event) -> Task<Message> {
        match event {
            Event::Open(tab, point, position)
                if self.preview().id == tab && self.editor.pending.is_none() =>
            {
                let Some(buffer) = self.editor_buffer() else {
                    return Task::none();
                };
                let picker = Picker::new(tab, point, position, Arc::clone(&buffer.document.text));
                if picker.is_some() {
                    self.editor.suggestions = None;
                    self.editor.definitions.clear();
                    self.editor.menu = None;
                }
                self.editor.color = picker;
                if self.editor.color.is_some() {
                    use iced::widget::operation;
                    return operation::focus("editor-color-4")
                        .chain(operation::select_all("editor-color-4"));
                }
            }
            Event::Close => self.editor.color = None,
            Event::Apply if self.color_picker().is_some() && self.editor.pending.is_none() => {
                let Some(picker) = self.editor.color.take() else {
                    return Task::none();
                };
                if picker.invalid.is_some() || picker.rgba == picker.original {
                    return Task::none();
                }
                let mut text = picker.snapshot.as_ref().clone();
                let replacement = picker.rgba.css(&text[picker.range.clone()]);
                text.replace_range(picker.range, &replacement);
                if text.len() > crate::domain::editor::LIMIT {
                    self.notifications.error(Key::EditorTooLarge, None);
                    return Task::none();
                }
                let task = self.update_editor(super::Event::Replace(picker.tab, text));
                if let Some(buffer) = self
                    .preview_for_request(picker.tab)
                    .and_then(|tab| tab.editor.as_mut())
                {
                    buffer.content.move_to(text_editor::Cursor {
                        position: text_editor::Position {
                            line: picker.position.line,
                            column: picker.position.column + replacement.len(),
                        },
                        selection: None,
                    });
                }
                return task;
            }
            event => {
                if let Some(picker) = &mut self.editor.color {
                    match event {
                        Event::Pick(control, x, y) => picker.pick(control, x, y),
                        Event::Field(index, value) => picker.field(index, value),
                        Event::Swatch(color) => picker.set_color(color),
                        _ => {}
                    }
                }
            }
        }
        Task::none()
    }
}
