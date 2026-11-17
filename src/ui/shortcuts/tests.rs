use super::*;
use crate::domain::shortcuts::Command;

#[test]
fn physical_keys_preserve_shortcuts_across_layouts_and_modifier_changes() {
    let binding = Command::Copy.default_binding();
    assert!(matches(
        &binding,
        &Key::Character("с".into()),
        key::Physical::Code(key::Code::KeyC),
        Modifiers::COMMAND
    ));
    assert!(!matches(
        &binding,
        &Key::Character("c".into()),
        key::Physical::Code(key::Code::KeyC),
        Modifiers::COMMAND | Modifiers::SHIFT
    ));
    // Logical characters must not create aliases for another physical shortcut.
    assert!(!matches(
        &binding,
        &Key::Character("c".into()),
        key::Physical::Code(key::Code::KeyK),
        Modifiers::COMMAND
    ));
}

#[test]
fn window_zoom_supports_shifted_plus_and_layout_independent_keys() {
    use iced::keyboard::{
        Key, Modifiers,
        key::{Code, Physical},
    };
    let command = Modifiers::COMMAND;
    assert_eq!(
        super::zoom_delta(
            &Key::Character("+".into()),
            Physical::Code(Code::Equal),
            command | Modifiers::SHIFT
        ),
        Some(10)
    );
    assert_eq!(
        super::zoom_delta(
            &Key::Character("-".into()),
            Physical::Code(Code::Minus),
            command
        ),
        Some(-10)
    );
    assert_eq!(
        super::zoom_delta(
            &Key::Character("0".into()),
            Physical::Code(Code::Digit0),
            command
        ),
        Some(0)
    );
    assert_eq!(
        super::zoom_delta(
            &Key::Character("+".into()),
            Physical::Code(Code::Equal),
            Modifiers::empty()
        ),
        None
    );
    assert_eq!(
        super::zoom_delta(
            &Key::Character("+".into()),
            Physical::Code(Code::Equal),
            command | Modifiers::ALT
        ),
        None
    );
}
