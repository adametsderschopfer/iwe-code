//! Display a read-only Markdown document with the workspace palette and fonts.

use crate::ui::theme::iced;
mod selection;

use crate::{domain::shortcuts::Shortcuts, ui::theme};
use iced::{
    Border, Element, Font, Length, Padding, Theme, padding,
    widget::{container, markdown, row, rule, scrollable, text},
};

const DOCUMENT_WIDTH: f32 = 680.;
const DOCUMENT_PADDING: Padding = Padding {
    top: 36.,
    right: 24.,
    bottom: 56.,
    left: 24.,
};

#[derive(Debug, PartialEq)]
pub(crate) enum Action {
    Link(String),
    Copy(String),
}

pub(crate) fn view<'a>(
    items: &'a [markdown::Item],
    appearance: &Theme,
    code_font: Font,
    code_size: u16,
    identity: (u64, u64),
    shortcuts: &Shortcuts,
) -> Element<'a, Action> {
    let mut style = markdown::Style::from(appearance.native());
    style.font = appearance.interface_font();
    style.link_color = theme::BLUE.resolve(appearance);
    style.inline_code_highlight.background = theme::MARKDOWN_INLINE_CODE.resolve(appearance).into();
    style.inline_code_padding = [0, 3].into();
    style.inline_code_color = theme::TEXT.resolve(appearance);
    style.inline_code_font = code_font;
    style.code_block_font = code_font;
    let mut settings = markdown::Settings::with_text_size(16, style);
    settings.code_size = f32::from(code_size).into();
    settings.spacing = 16.into();
    settings.h1_size = 34.into();
    settings.h2_size = 25.into();
    settings.h3_size = 20.into();
    settings.h4_size = 17.into();
    settings.h5_size = 16.into();
    settings.h6_size = 15.into();

    let selection = selection::Shared::default();
    let viewer = Viewer(selection);
    let document = container(markdown::view_with(items, settings, &viewer))
        .id("markdown-document")
        .width(Length::Fill)
        .max_width(DOCUMENT_WIDTH);
    let pane = container(
        scrollable(
            container(document)
                .padding(DOCUMENT_PADDING)
                .center_x(Length::Fill),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .style(theme::scroll),
    )
    .id("markdown-preview-pane")
    .clip(true)
    .width(Length::Fill)
    .height(Length::Fill)
    .style(|appearance| container::Style {
        text_color: Some(theme::MARKDOWN_TEXT.resolve(appearance)),
        ..theme::surface(appearance)
    });
    selection::document(pane.into(), viewer.0, identity, shortcuts.clone())
}

struct Viewer(selection::Shared);

impl<'a> markdown::Viewer<'a, Action, Theme> for Viewer {
    fn on_link_click(url: String) -> Action {
        Action::Link(url)
    }

    fn heading(
        &self,
        mut settings: markdown::Settings,
        level: &'a markdown::HeadingLevel,
        value: &'a markdown::Text,
        index: usize,
    ) -> Element<'a, Action> {
        use markdown::HeadingLevel::*;

        let size = match level {
            H1 => settings.h1_size,
            H2 => settings.h2_size,
            H3 => settings.h3_size,
            H4 => settings.h4_size,
            H5 => settings.h5_size,
            H6 => settings.h6_size,
        };
        let top = match (index, level) {
            (0, _) => 0,
            (_, H1 | H2) => 16,
            _ => 8,
        };
        settings.style.font.weight = iced::font::Weight::Semibold;
        container(self.0.block(value.spans(settings.style), |rich| {
            rich.size(size).line_height(1.22).style(theme::TEXT.text())
        }))
        .padding(padding::top(top))
        .into()
    }

    fn paragraph(
        &self,
        settings: markdown::Settings,
        value: &markdown::Text,
    ) -> Element<'a, Action> {
        self.0.block(value.spans(settings.style), |rich| {
            rich.size(settings.text_size).line_height(1.55)
        })
    }

    fn code_block(
        &self,
        settings: markdown::Settings,
        _language: Option<&'a str>,
        code: &'a str,
        _lines: &'a [markdown::Text],
    ) -> Element<'a, Action> {
        let code = self.0.block(
            vec![text::Span::new(code.strip_suffix('\n').unwrap_or(code))],
            |rich| {
                rich.font(settings.style.code_block_font)
                    .size(settings.code_size)
                    .line_height(1.45)
                    .wrapping(text::Wrapping::None)
            },
        );
        let scroller = scrollable(container(code).padding([13, 16]))
            .direction(scrollable::Direction::Horizontal(Default::default()))
            .width(Length::Fill)
            .style(theme::scroll);
        container(scroller)
            .width(Length::Fill)
            .style(|appearance| container::Style {
                border: Border {
                    color: theme::LINE.resolve(appearance),
                    width: 1.,
                    radius: appearance.control_radius().into(),
                },
                ..theme::code_surface(appearance)
            })
            .into()
    }

    fn image(
        &self,
        settings: markdown::Settings,
        _url: &'a String,
        _title: &'a str,
        alt: &markdown::Text,
    ) -> Element<'a, Action> {
        container(self.0.block(alt.spans(settings.style), |rich| rich))
            .padding(settings.spacing.0)
            .class(<Theme as markdown::Catalog>::code_block())
            .into()
    }

    fn quote(
        &self,
        settings: markdown::Settings,
        contents: &'a [markdown::Item],
    ) -> Element<'a, Action> {
        let quote = row![
            rule::vertical(2).style(|appearance| rule::Style {
                color: theme::BLUE.resolve(appearance),
                radius: 1.0.into(),
                ..rule::default(appearance.native())
            }),
            markdown::items(self, settings, contents),
        ]
        .spacing(14)
        .height(Length::Shrink);
        container(quote)
            .style(|appearance| container::Style {
                text_color: Some(theme::MUTED.resolve(appearance)),
                ..Default::default()
            })
            .into()
    }

    fn rule(&self, _settings: markdown::Settings) -> Element<'a, Action> {
        rule::horizontal(1)
            .style(|appearance| rule::Style {
                color: theme::LINE.resolve(appearance),
                ..rule::default(appearance.native())
            })
            .into()
    }
}

#[cfg(test)]
mod tests;
