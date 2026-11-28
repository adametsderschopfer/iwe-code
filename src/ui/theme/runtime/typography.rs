//! Resolve font families before Iced lays out the window.

use crate::domain::{
    theme::{FontFamily, Fonts},
    workspace::CodeFont,
};
use ::iced::{Font, font};
use std::{
    cell::Cell,
    collections::HashSet,
    sync::{Mutex, OnceLock},
};

#[derive(Debug, Clone, Copy, Default)]
pub(in crate::ui::theme) struct Typography {
    pub(super) interface: Option<Font>,
    pub(super) editor: Option<Font>,
    pub(super) terminal: Option<Font>,
}

impl Typography {
    pub(super) fn new(default: &Fonts, custom: Option<&Fonts>) -> Self {
        let resolve = |field: fn(&Fonts) -> Option<&FontFamily>| {
            custom
                .and_then(field)
                .and_then(|family| named_font(family.current()))
                .or_else(|| field(default).and_then(|family| named_font(family.current())))
        };
        Self {
            interface: resolve(|fonts| fonts.interface.as_ref()),
            editor: resolve(|fonts| fonts.editor.as_ref()),
            terminal: resolve(|fonts| fonts.terminal.as_ref()),
        }
    }
}

fn named_font(name: &str) -> Option<Font> {
    use font::Family;
    let family = match name {
        "sans-serif" => Family::SansSerif,
        "serif" => Family::Serif,
        "monospace" => Family::Monospace,
        "cursive" => Family::Cursive,
        "fantasy" => Family::Fantasy,
        name => {
            // Iced requires family names to live for the process lifetime.
            static NAMES: OnceLock<Mutex<HashSet<&'static str>>> = OnceLock::new();
            let mut names = NAMES.get_or_init(Mutex::default).lock().ok()?;
            let interned = match names.get(name) {
                Some(&name) => name,
                None if names.len() < 1024 => {
                    let name: &'static str = Box::leak(name.to_owned().into_boxed_str());
                    names.insert(name);
                    name
                }
                None => return None,
            };
            Family::Name(interned)
        }
    };
    Some(Font {
        family,
        ..Font::DEFAULT
    })
}

thread_local! {
    // Widget constructors need fonts before the renderer receives its theme.
    static VIEW: Cell<Typography> = const {
        Cell::new(Typography {
            interface: None,
            editor: None,
            terminal: None,
        })
    };
}

pub(in crate::ui::theme) fn with_typography<R>(fonts: Typography, view: impl FnOnce() -> R) -> R {
    VIEW.with(|current| {
        struct Restore<'a>(&'a Cell<Typography>, Typography);
        impl Drop for Restore<'_> {
            fn drop(&mut self) {
                self.0.set(self.1);
            }
        }
        let _restore = Restore(current, current.replace(fonts));
        view()
    })
}

pub(in crate::ui::theme) fn interface_override() -> Option<Font> {
    VIEW.with(Cell::get).interface
}

pub(crate) fn system_font() -> Font {
    super::DARK.typography.interface.unwrap_or(Font::DEFAULT)
}

pub(crate) fn font() -> Font {
    interface_override().unwrap_or_else(system_font)
}

pub(crate) fn semibold() -> Font {
    Font {
        weight: font::Weight::Semibold,
        ..font()
    }
}

pub(crate) fn code_font() -> Font {
    VIEW.with(Cell::get)
        .editor
        .or(super::DARK.typography.editor)
        .unwrap_or(Font::MONOSPACE)
}

pub(crate) fn selected_code_font(choice: CodeFont) -> Font {
    selected_font(choice, code_font)
}

pub(crate) fn selected_terminal_font(choice: CodeFont) -> Font {
    selected_font(choice, || {
        VIEW.with(Cell::get)
            .terminal
            .or(super::DARK.typography.terminal)
            .unwrap_or(Font::MONOSPACE)
    })
}

fn selected_font(choice: CodeFont, default: impl FnOnce() -> Font) -> Font {
    match choice {
        CodeFont::System => default(),
        CodeFont::JetBrainsMono => Font::with_name("JetBrains Mono"),
        CodeFont::FiraCode => Font::with_name("Fira Code"),
    }
}
