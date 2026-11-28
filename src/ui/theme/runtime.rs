//! Carry the installed palette and icon shapes through Iced's theme boundary.

mod typography;

pub(super) use typography::interface_override;
use typography::{Typography, with_typography};
pub(crate) use typography::{
    code_font, font, selected_code_font, selected_terminal_font, semibold, system_font,
};

use super::{BACKGROUND, BLUE, GREEN, RED, TEXT, YELLOW};
use crate::domain::{
    theme::{ColorRole, Definition, ISLANDS_ID, Metrics, Variant},
    workspace::Appearance,
};
use ::iced::{
    Color, Point, Theme as NativeTheme,
    theme::{Base, Mode, Palette, Style, palette},
    widget::canvas::Path,
};
use std::{
    collections::BTreeMap,
    sync::{
        Arc, LazyLock,
        atomic::{AtomicU64, Ordering},
    },
};

static DEFAULT: LazyLock<Definition> = LazyLock::new(|| {
    let definition: Definition =
        serde_json::from_str(include_str!("../../../assets/themes/iwe.json"))
            .expect("The embedded Iwe theme is valid JSON");
    assert!(definition.is_valid());
    assert_eq!(definition.dark.colors.len(), ColorRole::COUNT);
    assert_eq!(definition.light.colors.len(), ColorRole::COUNT);
    definition
});

static DARK: LazyLock<VariantStyle> =
    LazyLock::new(|| VariantStyle::new(&DEFAULT.dark, None, true, 0));
static LIGHT: LazyLock<VariantStyle> =
    LazyLock::new(|| VariantStyle::new(&DEFAULT.light, None, false, 1));

#[derive(Debug, Clone)]
pub(crate) enum Theme {
    Dark,
    Light,
    Installed(Arc<VariantStyle>),
}

impl PartialEq for Theme {
    fn eq(&self, other: &Self) -> bool {
        self.revision() == other.revision()
    }
}

#[derive(Debug)]
pub(crate) struct InstalledTheme {
    pub(crate) label: String,
    dark: Theme,
    light: Theme,
}

impl InstalledTheme {
    pub(crate) fn from_definition(definition: Definition) -> Self {
        static NEXT_REVISION: AtomicU64 = AtomicU64::new(2);
        let dark = NEXT_REVISION.fetch_add(2, Ordering::Relaxed);
        Self {
            dark: Theme::Installed(Arc::new(VariantStyle::new(
                &DEFAULT.dark,
                Some(&definition.dark),
                true,
                dark,
            ))),
            light: Theme::Installed(Arc::new(VariantStyle::new(
                &DEFAULT.light,
                Some(&definition.light),
                false,
                dark + 1,
            ))),
            label: definition.name,
        }
    }

    pub(crate) fn variant(&self, appearance: Appearance) -> Theme {
        match appearance {
            Appearance::Dark => self.dark.clone(),
            Appearance::Light => self.light.clone(),
        }
    }
}

#[derive(Debug)]
pub(crate) struct VariantStyle {
    revision: u64,
    native: NativeTheme,
    colors: [Color; ColorRole::COUNT],
    typography: Typography,
    metrics: Metrics,
    icons: BTreeMap<String, IconShape>,
}

#[derive(Debug)]
pub(crate) struct IconShape {
    pub(crate) path: Path,
    pub(crate) fill: bool,
}

impl VariantStyle {
    fn new(default: &Variant, custom: Option<&Variant>, dark: bool, revision: u64) -> Self {
        let mut colors = [Color::BLACK; ColorRole::COUNT];
        for (role, rgb) in default
            .colors
            .iter()
            .chain(custom.into_iter().flat_map(|v| &v.colors))
        {
            colors[*role as usize] = Color::from_rgb8(rgb.0, rgb.1, rgb.2);
        }
        let native = match custom {
            None if dark => NativeTheme::Dark,
            None => NativeTheme::Light,
            Some(_) => NativeTheme::custom_with_fn(
                format!("Iwe theme {revision}"),
                Palette {
                    background: colors[BACKGROUND.0 as usize],
                    text: colors[TEXT.0 as usize],
                    primary: colors[BLUE.0 as usize],
                    success: colors[GREEN.0 as usize],
                    danger: colors[RED.0 as usize],
                    warning: colors[YELLOW.0 as usize],
                },
                |palette| palette::Extended {
                    is_dark: dark,
                    ..palette::Extended::generate(palette)
                },
            ),
        };
        Self {
            revision,
            native,
            colors,
            typography: Typography::new(&default.fonts, custom.map(|v| &v.fonts)),
            metrics: custom.map_or(default.metrics, |v| v.metrics),
            icons: default
                .icons
                .iter()
                .chain(custom.into_iter().flat_map(|v| &v.icons))
                .map(|(name, icon)| {
                    (
                        name.clone(),
                        IconShape {
                            path: Path::new(|path| {
                                use svgtypes::SimplePathSegment::*;
                                let point = |x: f64, y: f64| Point::new(x as f32, y as f32);
                                for segment in icon.segments().flatten() {
                                    match segment {
                                        MoveTo { x, y } => path.move_to(point(x, y)),
                                        LineTo { x, y } => path.line_to(point(x, y)),
                                        Quadratic { x1, y1, x, y } => {
                                            path.quadratic_curve_to(point(x1, y1), point(x, y));
                                        }
                                        CurveTo {
                                            x1,
                                            y1,
                                            x2,
                                            y2,
                                            x,
                                            y,
                                        } => {
                                            path.bezier_curve_to(
                                                point(x1, y1),
                                                point(x2, y2),
                                                point(x, y),
                                            );
                                        }
                                        ClosePath => path.close(),
                                    }
                                }
                            }),
                            fill: icon.fill,
                        },
                    )
                })
                .collect(),
        }
    }
}

impl Theme {
    pub(crate) fn builtin(appearance: Appearance) -> Self {
        match appearance {
            Appearance::Dark => Self::Dark,
            Appearance::Light => Self::Light,
        }
    }

    pub(crate) fn islands(appearance: Appearance) -> Self {
        static ISLANDS: LazyLock<InstalledTheme> = LazyLock::new(|| {
            let definition: Definition =
                serde_json::from_str(include_str!("../../../assets/themes/islands.json"))
                    .expect("The embedded Islands theme is valid JSON");
            assert!(definition.is_valid());
            assert_eq!(definition.id, ISLANDS_ID);
            InstalledTheme::from_definition(definition)
        });
        ISLANDS.variant(appearance)
    }

    fn variant(&self) -> &VariantStyle {
        match self {
            Self::Dark => &DARK,
            Self::Light => &LIGHT,
            Self::Installed(style) => style,
        }
    }

    pub(crate) fn native(&self) -> &NativeTheme {
        &self.variant().native
    }

    pub(crate) fn revision(&self) -> u64 {
        self.variant().revision
    }

    pub(crate) fn color(&self, role: ColorRole) -> Color {
        self.variant().colors[role as usize]
    }

    pub(crate) fn icon(&self, name: &str) -> Option<&IconShape> {
        self.variant().icons.get(name)
    }

    fn metric(&self, field: impl Fn(&Metrics) -> Option<f32>) -> f32 {
        let defaults = if self.native().extended_palette().is_dark {
            &DEFAULT.dark
        } else {
            &DEFAULT.light
        };
        field(&self.variant().metrics)
            .or_else(|| field(&defaults.metrics))
            .expect("The embedded Iwe theme defines every metric")
    }

    pub(crate) fn control_radius(&self) -> f32 {
        self.metric(|metrics| metrics.control_radius)
    }

    pub(crate) fn dialog_radius(&self) -> f32 {
        self.metric(|metrics| metrics.dialog_radius)
    }

    pub(crate) fn icon_stroke(&self) -> f32 {
        self.metric(|metrics| metrics.icon_stroke)
    }

    pub(crate) fn backdrop_opacity(&self) -> f32 {
        self.metric(|metrics| metrics.backdrop_opacity)
    }

    pub(crate) fn diff_opacity(&self) -> f32 {
        self.metric(|metrics| metrics.diff_opacity)
    }

    pub(crate) fn interface_font(&self) -> ::iced::Font {
        self.variant()
            .typography
            .interface
            .unwrap_or_else(system_font)
    }

    pub(crate) fn with_typography<R>(&self, view: impl FnOnce() -> R) -> R {
        with_typography(self.variant().typography, view)
    }
}

impl Base for Theme {
    fn default(mode: Mode) -> Self {
        match mode {
            Mode::Dark => Self::Dark,
            _ => Self::Light,
        }
    }

    fn mode(&self) -> Mode {
        self.native().mode()
    }

    fn base(&self) -> Style {
        self.native().base()
    }

    fn palette(&self) -> Option<Palette> {
        Some(self.native().palette())
    }

    fn name(&self) -> &str {
        self.native().name()
    }
}
