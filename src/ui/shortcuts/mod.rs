//! Translate physical keyboard events into persistent shortcut bindings.

use crate::domain::shortcuts::Binding;
use iced::keyboard::{Key, Modifiers, key};

pub(crate) fn binding(key: &Key, physical: key::Physical, modifiers: Modifiers) -> Option<Binding> {
    let name = match physical {
        key::Physical::Code(code) => {
            let code = format!("{code:?}");
            code.strip_prefix("Key")
                .or_else(|| code.strip_prefix("Digit"))
                .unwrap_or(&code)
                .to_owned()
        }
        _ => match key {
            Key::Character(value) if value.is_ascii() && value.len() == 1 => {
                value.to_ascii_uppercase()
            }
            _ => return None,
        },
    };
    Some(Binding {
        key: name,
        control: modifiers.control(),
        super_key: modifiers.logo(),
        alt: modifiers.alt(),
        shift: modifiers.shift(),
    })
}

pub(crate) fn matches(
    binding: &Binding,
    key: &Key,
    physical: key::Physical,
    modifiers: Modifiers,
) -> bool {
    self::binding(key, physical, modifiers).as_ref() == Some(binding)
}

pub(crate) fn modifier(key: &Key) -> bool {
    matches!(
        key,
        Key::Named(
            key::Named::Control
                | key::Named::Alt
                | key::Named::Shift
                | key::Named::Super
                | key::Named::Meta
        )
    )
}

/// Application zoom is available regardless of the focused text surface.
pub(crate) fn zoom_delta(key: &Key, physical: key::Physical, modifiers: Modifiers) -> Option<i16> {
    if !modifiers.command() || modifiers.alt() || (cfg!(target_os = "macos") && modifiers.control())
    {
        return None;
    }
    match (key, physical) {
        (Key::Character(value), _) if value == "+" || value == "=" => Some(10),
        (Key::Character(value), _) if value == "-" => Some(-10),
        (Key::Character(value), _) if value == "0" => Some(0),
        (_, key::Physical::Code(key::Code::Equal | key::Code::NumpadAdd)) => Some(10),
        (_, key::Physical::Code(key::Code::Minus | key::Code::NumpadSubtract)) => Some(-10),
        (_, key::Physical::Code(key::Code::Digit0 | key::Code::Numpad0)) => Some(0),
        _ => None,
    }
}

#[cfg(test)]
mod tests;
