//! Share syntax roles between Darcula and the light editor palette.

use super::Tone;
use crate::domain::theme::ColorRole;
use two_face::re_exports::syntect::highlighting::{
    Color as SyntaxColor, StyleModifier, Theme, ThemeItem, ThemeSettings,
};

pub(crate) const TEXT: Tone = Tone(ColorRole::SyntaxText);
pub(crate) const KEYWORD: Tone = Tone(ColorRole::SyntaxKeyword);
pub(crate) const FUNCTION: Tone = Tone(ColorRole::SyntaxFunction);
pub(crate) const STRING: Tone = Tone(ColorRole::SyntaxString);
pub(crate) const COMMENT: Tone = Tone(ColorRole::SyntaxComment);
pub(crate) const NUMBER: Tone = Tone(ColorRole::SyntaxNumber);
pub(crate) const PARAMETER: Tone = Tone(ColorRole::SyntaxParameter);
pub(crate) const FIELD: Tone = Tone(ColorRole::SyntaxField);
pub(crate) const ESCAPE: Tone = Tone(ColorRole::SyntaxEscape);
pub(crate) const DECORATOR: Tone = Tone(ColorRole::SyntaxDecorator);

fn color(color: Tone) -> SyntaxColor {
    let [r, g, b, a] = color.resolve(&super::Theme::Dark).into_rgba8();
    SyntaxColor { r, g, b, a }
}

pub(crate) fn tone(color: SyntaxColor) -> Tone {
    [
        TEXT, KEYWORD, FUNCTION, STRING, COMMENT, NUMBER, PARAMETER, FIELD, ESCAPE, DECORATOR,
    ]
    .into_iter()
    .find(|tone| {
        tone.resolve(&super::Theme::Dark).into_rgba8() == [color.r, color.g, color.b, color.a]
    })
    .unwrap_or(TEXT)
}

pub(crate) fn palette() -> Theme {
    let rules = [
        ("comment", COMMENT),
        ("keyword, storage, constant.language", KEYWORD),
        ("string, constant.other", STRING),
        ("constant.numeric", NUMBER),
        (
            "entity.name.function, support.function, variable.function, entity.name.type, support.type, entity.other.inherited-class",
            FUNCTION,
        ),
        ("variable.parameter", PARAMETER),
        (
            "variable.other.member, variable.other.property, entity.name.tag",
            FIELD,
        ),
        (
            "entity.other.attribute-name, support.type.property-name, meta.mapping.key string",
            FUNCTION,
        ),
        ("constant.character.escape, string.regexp", ESCAPE),
        (
            "meta.annotation, meta.function.decorator, entity.name.function.decorator, punctuation.definition.annotation",
            DECORATOR,
        ),
        ("keyword.operator", TEXT),
        (
            "keyword.operator.word, keyword.operator.logical.python",
            KEYWORD,
        ),
        (
            "source.css support.function, source.css entity.other.attribute-name",
            PARAMETER,
        ),
    ];
    Theme {
        name: Some("Gerry Darcula".to_owned()),
        settings: ThemeSettings {
            foreground: Some(color(TEXT)),
            ..Default::default()
        },
        scopes: rules
            .into_iter()
            .map(|(scope, foreground)| ThemeItem {
                scope: scope.parse().expect("Valid embedded syntax scopes"),
                style: StyleModifier {
                    foreground: Some(color(foreground)),
                    ..Default::default()
                },
            })
            .collect(),
        ..Default::default()
    }
}
