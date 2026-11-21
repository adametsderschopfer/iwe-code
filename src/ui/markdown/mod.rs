//! Display a read-only Markdown document with the workspace palette and fonts.

use crate::ui::theme;
use iced::{
    Element, Font, Length, Theme,
    widget::{container, markdown, rich_text, scrollable, text},
};

pub(crate) fn view<'a>(
    items: &'a [markdown::Item],
    appearance: &Theme,
    code_font: Font,
    code_size: u16,
) -> Element<'a, String> {
    let mut style = markdown::Style::from(appearance);
    style.font = theme::font();
    style.link_color = theme::BLUE.resolve(appearance);
    style.inline_code_highlight.background = theme::code_background(appearance).into();
    style.inline_code_color = theme::TEXT.resolve(appearance);
    style.inline_code_font = code_font;
    style.code_block_font = code_font;
    let mut settings = markdown::Settings::with_text_size(16, style);
    settings.code_size = f32::from(code_size).into();

    let document = markdown::view_with(items, settings, &Viewer);
    container(
        scrollable(container(document).padding(16).width(Length::Fill))
            .width(Length::Fill)
            .height(Length::Fill)
            .style(theme::scroll),
    )
    .id("markdown-preview-pane")
    .clip(true)
    .width(Length::Fill)
    .height(Length::Fill)
    .style(theme::surface)
    .into()
}

struct Viewer;

impl<'a> markdown::Viewer<'a, String> for Viewer {
    fn on_link_click(url: String) -> String {
        url
    }

    fn paragraph(
        &self,
        settings: markdown::Settings,
        value: &markdown::Text,
    ) -> Element<'a, String> {
        rich_text(value.spans(settings.style))
            .size(settings.text_size)
            .wrapping(text::Wrapping::WordOrGlyph)
            .on_link_click(Self::on_link_click)
            .into()
    }

    fn code_block(
        &self,
        settings: markdown::Settings,
        _language: Option<&'a str>,
        code: &'a str,
        _lines: &'a [markdown::Text],
    ) -> Element<'a, String> {
        let code = text(code.strip_suffix('\n').unwrap_or(code))
            .font(settings.style.code_block_font)
            .size(settings.code_size)
            .wrapping(text::Wrapping::None);
        let scroller = scrollable(container(code).padding(12))
            .direction(scrollable::Direction::Horizontal(Default::default()))
            .width(Length::Fill)
            .style(theme::scroll);
        container(scroller)
            .width(Length::Fill)
            .style(theme::code_surface)
            .into()
    }
}

#[cfg(test)]
mod tests;
