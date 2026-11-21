//! Shared vertical toolbar for workspace tool windows.
use super::{
    icons::{Icon, sized_icon},
    theme, tooltip,
};
use iced::{
    Element, Length,
    widget::{Column, button, container, scrollable, text},
};

pub(crate) const WIDTH: f32 = 42.;
pub(crate) const GAP: f32 = 2.;

pub(crate) fn item<'a, M: Clone + 'a>(
    id: &'static str,
    symbol: Icon,
    label: String,
    active: bool,
    message: Option<M>,
) -> Element<'a, M> {
    let color = if active { theme::BLUE } else { theme::MUTED };
    tooltip::view(
        container(
            button(container(sized_icon(symbol, color, 16.)).center_x(Length::Fill))
                .width(Length::Fill)
                .padding([7, 0])
                .on_press_maybe(message)
                .style(theme::tree_item(active)),
        )
        .id(id),
        text(label).size(12),
        tooltip::Position::Right,
    )
    .into()
}

pub(crate) fn view<'a, M: 'a>(
    id: &'static str,
    scroll_id: &'static str,
    items: Column<'a, M>,
) -> Element<'a, M> {
    container(
        scrollable(items.spacing(GAP).padding(3))
            .id(scroll_id)
            .direction(scrollable::Direction::Vertical(
                scrollable::Scrollbar::hidden(),
            ))
            .height(Length::Fill)
            .style(theme::scroll),
    )
    .id(id)
    .width(WIDTH)
    .height(Length::Fill)
    .style(theme::surface)
    .into()
}
