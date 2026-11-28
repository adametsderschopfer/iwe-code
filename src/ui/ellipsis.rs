//! Fit a single line using the renderer's actual text measurements.

use crate::ui::theme::iced;
use iced::{
    Element, Font, Length, Rectangle, Renderer, Size, Theme,
    advanced::{
        Layout, Widget, layout, mouse, renderer,
        text::{self, Paragraph as _, Renderer as _},
        widget::{Operation, Tree, tree},
    },
    alignment,
};
use std::borrow::Cow;

type Paragraph = <Renderer as text::Renderer>::Paragraph;

pub(crate) fn measured_width(content: &str, size: f32) -> f32 {
    paragraph(content, size, super::theme::font()).min_width()
}

pub(crate) fn text<'a, Message: 'a>(
    content: impl Into<Cow<'a, str>>,
    size: f32,
) -> Element<'a, Message> {
    text_with_font(content, size, super::theme::font())
}

pub(crate) fn text_with_font<'a, Message: 'a>(
    content: impl Into<Cow<'a, str>>,
    size: f32,
    font: Font,
) -> Element<'a, Message> {
    Element::new(Ellipsis {
        content: content.into(),
        size,
        font,
        shrink: false,
    })
}

pub(crate) fn compact_text<'a, Message: 'a>(
    content: impl Into<Cow<'a, str>>,
    size: f32,
) -> Element<'a, Message> {
    Element::new(Ellipsis {
        content: content.into(),
        size,
        font: super::theme::font(),
        shrink: true,
    })
}

struct Ellipsis<'a> {
    content: Cow<'a, str>,
    size: f32,
    font: Font,
    shrink: bool,
}

#[derive(Default)]
struct State {
    original: String,
    displayed: String,
    width: f32,
    size: f32,
    font: Font,
    paragraph: Paragraph,
}

fn paragraph(content: &str, size: f32, font: Font) -> Paragraph {
    Paragraph::with_text(text::Text {
        content,
        bounds: Size::new(f32::INFINITY, size * 1.3),
        size: size.into(),
        line_height: text::LineHeight::Relative(1.3),
        font,
        align_x: text::Alignment::Left,
        align_y: alignment::Vertical::Top,
        shaping: text::Shaping::Advanced,
        wrapping: text::Wrapping::None,
    })
}

impl State {
    fn update(&mut self, content: &str, width: f32, size: f32, font: Font) {
        if self.original == content && self.width == width && self.size == size && self.font == font
        {
            return;
        }
        self.original = content.to_owned();
        self.width = width;
        self.size = size;
        self.font = font;
        self.displayed = content.to_owned();
        self.paragraph = paragraph(content, size, font);
        if self.paragraph.min_width() > width {
            let boundaries: Vec<_> = content
                .char_indices()
                .map(|(index, _)| index)
                .chain([content.len()])
                .collect();
            let mut low = 0;
            let mut high = boundaries.len() - 1;
            while low < high {
                let middle = (low + high).div_ceil(2);
                let candidate = format!("{}…", content[..boundaries[middle]].trim_end());
                if paragraph(&candidate, size, font).min_width() <= width {
                    low = middle;
                } else {
                    high = middle - 1;
                }
            }
            self.displayed = format!("{}…", content[..boundaries[low]].trim_end());
            self.paragraph = paragraph(&self.displayed, size, font);
            if self.paragraph.min_width() > width {
                self.displayed.clear();
                self.paragraph = paragraph("", size, font);
            }
        }
    }
}

impl<Message> Widget<Message, Theme, Renderer> for Ellipsis<'_> {
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State>()
    }
    fn state(&self) -> tree::State {
        tree::State::new(State::default())
    }
    fn size(&self) -> Size<Length> {
        Size::new(
            if self.shrink {
                Length::Shrink
            } else {
                Length::Fill
            },
            Length::Shrink,
        )
    }
    fn layout(&mut self, tree: &mut Tree, _: &Renderer, limits: &layout::Limits) -> layout::Node {
        let state = tree.state.downcast_mut::<State>();
        state.update(&self.content, limits.max().width, self.size, self.font);
        layout::Node::new(limits.resolve(
            if self.shrink {
                Length::Shrink
            } else {
                Length::Fill
            },
            Length::Shrink,
            Size::new(
                if self.shrink {
                    state.paragraph.min_width()
                } else {
                    state.width
                },
                self.size * 1.3,
            ),
        ))
    }
    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        _: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        _: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        if let Some(clip) = layout.bounds().intersection(viewport) {
            renderer.fill_paragraph(
                &tree.state.downcast_ref::<State>().paragraph,
                layout.position(),
                style.text_color,
                clip,
            );
        }
    }
    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        _: &Renderer,
        operation: &mut dyn Operation,
    ) {
        operation.text(
            None,
            layout.bounds(),
            &tree.state.downcast_ref::<State>().displayed,
        );
    }
}

#[cfg(test)]
mod tests;
