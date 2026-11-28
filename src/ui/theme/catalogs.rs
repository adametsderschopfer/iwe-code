//! Adapt Iced's default widget styles to the application theme.

use super::Theme;
use ::iced::widget::{
    button, checkbox, container, markdown, overlay::menu, pane_grid, pick_list, progress_bar, rule,
    scrollable, slider, table, text, text_editor, text_input, toggler,
};

macro_rules! catalog {
    ($widget:ident, $default:expr) => {
        impl $widget::Catalog for Theme {
            type Class<'a> = $widget::StyleFn<'a, Self>;

            fn default<'a>() -> <Self as $widget::Catalog>::Class<'a> {
                Box::new($default)
            }

            fn style(&self, class: &<Self as $widget::Catalog>::Class<'_>) -> $widget::Style {
                class(self)
            }
        }
    };
    ($widget:ident, $default:expr, status) => {
        impl $widget::Catalog for Theme {
            type Class<'a> = $widget::StyleFn<'a, Self>;

            fn default<'a>() -> <Self as $widget::Catalog>::Class<'a> {
                Box::new($default)
            }

            fn style(
                &self,
                class: &<Self as $widget::Catalog>::Class<'_>,
                status: $widget::Status,
            ) -> $widget::Style {
                class(self, status)
            }
        }
    };
}

catalog!(container, container::transparent);
catalog!(text, |_| text::Style::default());
catalog!(menu, |theme| menu::default(theme.native()));
catalog!(pane_grid, |theme| pane_grid::default(theme.native()));
catalog!(rule, |theme| rule::default(theme.native()));
catalog!(table, |theme| table::default(theme.native()));
catalog!(progress_bar, |theme| progress_bar::primary(theme.native()));
catalog!(
    button,
    |theme, status| button::primary(theme.native(), status),
    status
);
catalog!(
    checkbox,
    |theme, status| checkbox::primary(theme.native(), status),
    status
);
catalog!(
    pick_list,
    |theme, status| pick_list::default(theme.native(), status),
    status
);
catalog!(
    scrollable,
    |theme, status| scrollable::default(theme.native(), status),
    status
);
catalog!(
    slider,
    |theme, status| slider::default(theme.native(), status),
    status
);
catalog!(
    text_editor,
    |theme, status| text_editor::default(theme.native(), status),
    status
);
catalog!(
    text_input,
    |theme, status| text_input::default(theme.native(), status),
    status
);
catalog!(
    toggler,
    |theme, status| toggler::default(theme.native(), status),
    status
);

impl markdown::Catalog for Theme {
    fn code_block<'a>() -> <Self as container::Catalog>::Class<'a> {
        Box::new(|theme| container::dark(theme.native()))
    }
}
