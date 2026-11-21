//! Shared text controls keep native editing, keyboard input, and selection.

use super::{
    BLUE, CODE_BACKGROUND, CODE_SELECTION, CONTROL_RADIUS, DANGER_HOVER, ELEVATED, FAINT,
    INPUT_BORDER, INPUT_SELECTION, LINE, MUTED, PRIMARY_DISABLED, RED, TEXT, font, syntax,
};
use iced::{
    Border, Element, Theme,
    advanced::text::highlighter::PlainText,
    widget::{Column, TextEditor, TextInput, column, text, text_editor, text_input},
};

const FIELD_SIZE: f32 = 14.;
const FIELD_PADDING: [f32; 2] = [7., 10.];

pub(crate) fn text_field<'a, M: Clone>(placeholder: &str, value: &str) -> TextInput<'a, M> {
    text_input(placeholder, value)
        .font(font())
        .size(FIELD_SIZE)
        .padding(FIELD_PADDING)
        .style(input)
}

pub(crate) fn compact_field<'a, M: Clone>(placeholder: &str, value: &str) -> TextInput<'a, M> {
    text_field(placeholder, value).size(13).padding([4, 8])
}

pub(crate) fn labeled_field<'a, M: 'a>(
    label: String,
    field: impl Into<Element<'a, M>>,
) -> Column<'a, M> {
    column![text(label).size(13).style(MUTED.text()), field.into()].spacing(6)
}

pub(crate) fn multiline_field<M: Clone>(
    content: &text_editor::Content,
) -> TextEditor<'_, PlainText, M> {
    text_editor(content)
        .font(font())
        .size(FIELD_SIZE)
        .padding(FIELD_PADDING)
        .style(multiline_input)
}

pub(crate) fn validated_input(
    invalid: bool,
) -> fn(&Theme, text_input::Status) -> text_input::Style {
    if invalid { invalid_input } else { input }
}

fn input(theme: &Theme, status: text_input::Status) -> text_input::Style {
    use text_input::Status::*;
    let (outline, width) = match status {
        Active => (INPUT_BORDER, 1.),
        Hovered => (MUTED, 1.),
        Focused { .. } => (BLUE, 2.),
        Disabled => (LINE, 1.),
    };
    let disabled = status == Disabled;
    text_input::Style {
        background: if disabled { PRIMARY_DISABLED } else { ELEVATED }
            .resolve(theme)
            .into(),
        border: Border {
            color: outline.resolve(theme),
            width,
            radius: CONTROL_RADIUS.into(),
        },
        icon: MUTED.resolve(theme),
        placeholder: FAINT.resolve(theme),
        value: if disabled { FAINT } else { TEXT }.resolve(theme),
        selection: INPUT_SELECTION.resolve(theme),
    }
}

fn invalid_input(theme: &Theme, status: text_input::Status) -> text_input::Style {
    let mut style = input(theme, status);
    style.border.color = match status {
        text_input::Status::Active | text_input::Status::Focused { .. } => RED.resolve(theme),
        text_input::Status::Hovered => DANGER_HOVER.resolve(theme),
        text_input::Status::Disabled => style.border.color,
    };
    style
}

fn multiline_input(theme: &Theme, status: text_editor::Status) -> text_editor::Style {
    let field = input(
        theme,
        match status {
            text_editor::Status::Active => text_input::Status::Active,
            text_editor::Status::Hovered => text_input::Status::Hovered,
            text_editor::Status::Focused { is_hovered } => {
                text_input::Status::Focused { is_hovered }
            }
            text_editor::Status::Disabled => text_input::Status::Disabled,
        },
    );
    text_editor::Style {
        background: field.background,
        border: field.border,
        placeholder: field.placeholder,
        value: field.value,
        selection: field.selection,
    }
}

pub(crate) fn code_editor(theme: &Theme, status: text_editor::Status) -> text_editor::Style {
    let field = multiline_input(theme, status);
    text_editor::Style {
        background: CODE_BACKGROUND.resolve(theme).into(),
        border: Border::default(),
        value: syntax::TEXT.resolve(theme),
        selection: CODE_SELECTION.resolve(theme),
        ..field
    }
}
