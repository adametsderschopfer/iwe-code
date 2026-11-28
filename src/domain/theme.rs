//! Describe versioned themes without dependencies on the renderer.

use serde::Deserialize;
use std::collections::BTreeMap;

pub(crate) const BUILTIN_ID: &str = "iwe";
pub(crate) const ISLANDS_ID: &str = "iwe-islands";

pub(crate) fn is_bundled(id: &str) -> bool {
    matches!(id, BUILTIN_ID | ISLANDS_ID)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum ColorRole {
    Background,
    CodeBackground,
    Sidebar,
    Elevated,
    Hover,
    Selected,
    TreeSelected,
    CompletionSelected,
    CodeSelection,
    Dialog,
    Fold,
    InputBorder,
    InputSelection,
    Outline,
    Primary,
    PrimaryHover,
    DangerHover,
    PrimaryDisabled,
    PrimaryText,
    Thumb,
    ThumbHover,
    Text,
    MarkdownText,
    MarkdownInlineCode,
    Muted,
    Faint,
    Line,
    Green,
    UpdateBlue,
    Blue,
    Red,
    Yellow,
    Purple,
    Pink,
    PipelineGreen,
    PipelineRed,
    PipelineBlue,
    PipelineAmber,
    PipelineGray,
    PipelineGlyph,
    GitAdded,
    DiffModified,
    GitRemoved,
    UnifiedModified,
    FileAdded,
    FileModified,
    IconGray,
    IconBlue,
    IconYellow,
    IconCyan,
    IconOrange,
    IconPink,
    SyntaxText,
    SyntaxKeyword,
    SyntaxFunction,
    SyntaxString,
    SyntaxComment,
    SyntaxNumber,
    SyntaxParameter,
    SyntaxField,
    SyntaxEscape,
    SyntaxDecorator,
    AnsiBlack,
    AnsiRed,
    AnsiGreen,
    AnsiYellow,
    AnsiBlue,
    AnsiMagenta,
    AnsiCyan,
    AnsiWhite,
    AnsiBrightBlack,
    AnsiBrightRed,
    AnsiBrightGreen,
    AnsiBrightYellow,
    AnsiBrightBlue,
    AnsiBrightMagenta,
    AnsiBrightCyan,
    AnsiBrightWhite,
    TerminalBackground,
    TerminalText,
    IconHighlight,
    IconJavascriptText,
    IconVueGreen,
    IconVueBlue,
    IconHtml,
    IconYaml,
    TooltipSuccessText,
}

impl ColorRole {
    pub(crate) const COUNT: usize = Self::TooltipSuccessText as usize + 1;
    pub(crate) const ANSI: [Self; 16] = [
        Self::AnsiBlack,
        Self::AnsiRed,
        Self::AnsiGreen,
        Self::AnsiYellow,
        Self::AnsiBlue,
        Self::AnsiMagenta,
        Self::AnsiCyan,
        Self::AnsiWhite,
        Self::AnsiBrightBlack,
        Self::AnsiBrightRed,
        Self::AnsiBrightGreen,
        Self::AnsiBrightYellow,
        Self::AnsiBrightBlue,
        Self::AnsiBrightMagenta,
        Self::AnsiBrightCyan,
        Self::AnsiBrightWhite,
    ];
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(try_from = "String")]
pub(crate) struct Rgb(pub(crate) u8, pub(crate) u8, pub(crate) u8);

impl TryFrom<String> for Rgb {
    type Error = &'static str;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        let hex = value.strip_prefix('#').ok_or("expected #RRGGBB")?;
        if hex.len() != 6 || !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err("expected #RRGGBB");
        }
        let value = u32::from_str_radix(hex, 16).map_err(|_| "expected #RRGGBB")?;
        Ok(Self((value >> 16) as u8, (value >> 8) as u8, value as u8))
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct Fonts {
    pub(crate) interface: Option<FontFamily>,
    pub(crate) editor: Option<FontFamily>,
    pub(crate) terminal: Option<FontFamily>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub(crate) enum FontFamily {
    Name(String),
    Platform(PlatformFonts),
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PlatformFonts {
    pub(crate) default: String,
    pub(crate) macos: Option<String>,
    pub(crate) windows: Option<String>,
    pub(crate) linux: Option<String>,
}

impl FontFamily {
    pub(crate) fn current(&self) -> &str {
        match self {
            Self::Name(name) => name,
            Self::Platform(names) => {
                let platform = if cfg!(target_os = "macos") {
                    &names.macos
                } else if cfg!(target_os = "windows") {
                    &names.windows
                } else {
                    &names.linux
                };
                platform.as_deref().unwrap_or(&names.default)
            }
        }
    }

    fn is_valid(&self) -> bool {
        match self {
            Self::Name(name) => valid_label(name, 80),
            Self::Platform(names) => [&names.macos, &names.windows, &names.linux]
                .into_iter()
                .filter_map(Option::as_ref)
                .chain([&names.default])
                .all(|name| valid_label(name, 80)),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "kebab-case")]
pub(crate) struct Metrics {
    pub(crate) control_radius: Option<f32>,
    pub(crate) dialog_radius: Option<f32>,
    pub(crate) icon_stroke: Option<f32>,
    pub(crate) backdrop_opacity: Option<f32>,
    pub(crate) diff_opacity: Option<f32>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Icon {
    pub(crate) path: String,
    #[serde(default)]
    pub(crate) fill: bool,
}

#[derive(Debug, Default, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct Variant {
    pub(crate) colors: BTreeMap<ColorRole, Rgb>,
    pub(crate) icons: BTreeMap<String, Icon>,
    pub(crate) fonts: Fonts,
    pub(crate) metrics: Metrics,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Definition {
    pub(crate) version: u32,
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) dark: Variant,
    pub(crate) light: Variant,
}

pub(crate) fn valid_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 48
        && value.as_bytes()[0].is_ascii_lowercase()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}

impl Definition {
    pub(crate) fn is_valid(&self) -> bool {
        self.version == 1
            && valid_id(&self.id)
            && valid_label(&self.name, 60)
            && self.dark.is_valid()
            && self.light.is_valid()
    }
}

impl Variant {
    fn is_valid(&self) -> bool {
        [
            &self.fonts.interface,
            &self.fonts.editor,
            &self.fonts.terminal,
        ]
        .into_iter()
        .flatten()
        .all(FontFamily::is_valid)
            && [
                (self.metrics.control_radius, 0.0..=24.0),
                (self.metrics.dialog_radius, 0.0..=24.0),
                (self.metrics.icon_stroke, 0.5..=4.0),
                (self.metrics.backdrop_opacity, 0.0..=1.0),
                (self.metrics.diff_opacity, 0.0..=1.0),
            ]
            .into_iter()
            .all(|(value, range)| value.is_none_or(|value| range.contains(&value)))
            && self.icons.len() <= 128
            && self
                .icons
                .iter()
                .all(|(name, icon)| valid_id(name) && icon.is_valid())
    }
}

impl Icon {
    pub(crate) fn segments(&self) -> svgtypes::SimplifyingPathParser<'_> {
        svgtypes::SimplifyingPathParser::from(self.path.as_str())
    }

    fn is_valid(&self) -> bool {
        if self.path.len() > 4096 {
            return false;
        }
        use svgtypes::SimplePathSegment::*;
        let point = |x: f64, y: f64| (-128.0..=128.0).contains(&x) && (-128.0..=128.0).contains(&y);
        let mut draws = false;
        for segment in self.segments() {
            let valid = match segment {
                Ok(MoveTo { x, y }) => point(x, y),
                Ok(LineTo { x, y }) => {
                    draws = true;
                    point(x, y)
                }
                Ok(Quadratic { x1, y1, x, y }) => {
                    draws = true;
                    point(x1, y1) && point(x, y)
                }
                Ok(CurveTo {
                    x1,
                    y1,
                    x2,
                    y2,
                    x,
                    y,
                }) => {
                    draws = true;
                    point(x1, y1) && point(x2, y2) && point(x, y)
                }
                Ok(ClosePath) => true,
                Err(_) => false,
            };
            if !valid {
                return false;
            }
        }
        draws
    }
}

fn valid_label(value: &str, max_chars: usize) -> bool {
    !value.is_empty()
        && value.trim() == value
        && value.chars().count() <= max_chars
        && value.chars().all(|character| !character.is_control())
}
