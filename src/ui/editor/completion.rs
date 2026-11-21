//! Display completion candidates beside the text cursor.

use crate::{
    app::{App, Message, editor::Event},
    ui::theme,
};
use iced::{
    Element, Length, Padding,
    widget::{button, column, container, opaque, responsive, row, scrollable, space, stack, text},
};

pub(super) fn overlay<'a>(app: &'a App, base: Element<'a, Message>) -> Element<'a, Message> {
    let Some(suggestions) = app
        .editor
        .suggestions
        .as_ref()
        .filter(|value| value.tab == app.preview().id)
    else {
        return stack![base].into();
    };
    let Some(buffer) = app.editor_buffer() else {
        return stack![base].into();
    };
    let size = app.workspace.preferences.code_size as f32;
    let prefix = suggestions
        .snapshot
        .split('\n')
        .nth(suggestions.cursor.line)
        .and_then(|line| line.get(..suggestions.cursor.column))
        .unwrap_or_default();
    let query: String = prefix
        .chars()
        .rev()
        .take_while(|character| character.is_alphanumeric() || *character == '_')
        .collect::<String>()
        .chars()
        .rev()
        .collect();
    let mut origin = iced::Point::new(
        super::TEXT_PADDING + suggestions.cursor.column as f32 * size * 0.6,
        super::TEXT_PADDING
            + (suggestions.cursor.line + 1) as f32
                * size
                * (f32::from(app.workspace.preferences.code_line_height) / 100.),
    );
    buffer.content.with_editor(|editor| {
        if let Some(run) = editor
            .buffer()
            .layout_runs()
            .find(|run| run.line_i == suggestions.cursor.line)
        {
            let line = suggestions
                .snapshot
                .split('\n')
                .nth(run.line_i)
                .unwrap_or_default();
            let byte = suggestions.cursor.column.min(line.len());
            origin.x = super::TEXT_PADDING
                + run
                    .glyphs
                    .iter()
                    .find(|glyph| glyph.start >= byte)
                    .map(|glyph| glyph.x)
                    .unwrap_or_else(|| run.glyphs.last().map_or(0., |glyph| glyph.x + glyph.w));
            origin.y = super::TEXT_PADDING + run.line_top + run.line_height;
        }
    });
    let popup = responsive(move |available| {
        let width = theme::COMPLETION_WIDTH.min(available.width);
        let progress = (suggestions.opened.elapsed().as_secs_f32()
            / theme::COMPLETION_APPEAR.as_secs_f32())
        .min(1.);
        let ease = 1. - (1. - progress).powi(3);
        let height = (suggestions.items.len().min(9) as f32 * theme::COMPLETION_ROW_HEIGHT)
            .min(available.height - 12.)
            .max(0.)
            * (0.9 + 0.1 * ease);
        let left = origin.x.min((available.width - width).max(0.));
        let top = if origin.y + height <= available.height {
            origin.y
        } else {
            (origin.y
                - height
                - size * (f32::from(app.workspace.preferences.code_line_height) / 100.))
                .max(0.)
        };
        let mut rows = column![];
        for (index, item) in suggestions.items.iter().enumerate() {
            let label = item["label"].as_str().unwrap_or_default();
            let detail = item["detail"].as_str().unwrap_or_default();
            let spans: Vec<iced::advanced::text::Span<'_, ()>> = if !query.is_empty()
                && let Some(start) = label.find(&query)
            {
                vec![
                    iced::widget::span(&label[..start]),
                    iced::widget::span(&label[start..start + query.len()])
                        .color(theme::BLUE.resolve(&app.theme())),
                    iced::widget::span(&label[start + query.len()..]),
                ]
            } else {
                vec![iced::widget::span(label)]
            };
            let symbol = match item["kind"].as_u64() {
                Some(2 | 3) => "ƒ",
                Some(7 | 8 | 22) => "◇",
                Some(6 | 5) => "○",
                Some(14) => "★",
                _ => "·",
            };
            rows = rows.push(
                button(
                    row![
                        text(symbol).size(size).style(theme::MUTED.text()),
                        iced::widget::rich_text(spans)
                            .font(theme::selected_code_font(
                                app.workspace.preferences.code_font
                            ))
                            .size(size)
                            .wrapping(iced::widget::text::Wrapping::None),
                        space().width(Length::Fill),
                        text(detail)
                            .size(11)
                            .style(theme::MUTED.text())
                            .wrapping(iced::widget::text::Wrapping::None)
                    ]
                    .spacing(10)
                    .align_y(iced::Alignment::Center),
                )
                .padding([6, 8])
                .height(theme::COMPLETION_ROW_HEIGHT)
                .width(Length::Fill)
                .style(theme::completion_row(index == suggestions.selected))
                .on_press(Message::Editor(Event::AcceptCompletion(index))),
            );
        }
        container(opaque(
            container(
                scrollable(rows)
                    .style(theme::scroll)
                    .id("editor-completions")
                    .height(height),
            )
            .id("editor-completion-popup")
            .padding(6)
            .width(width)
            .style(theme::tooltip),
        ))
        .padding(Padding {
            top,
            right: 0.,
            bottom: 0.,
            left,
        })
        .into()
    });
    stack![base, popup].into()
}
