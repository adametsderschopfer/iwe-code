use super::*;

fn luminance(color: Color) -> f32 {
    let linear = |channel: f32| {
        if channel <= 0.04045 {
            channel / 12.92
        } else {
            ((channel + 0.055) / 1.055).powf(2.4)
        }
    };
    linear(color.r) * 0.2126 + linear(color.g) * 0.7152 + linear(color.b) * 0.0722
}

fn contrast(foreground: Color, background: Color) -> f32 {
    let a = luminance(foreground);
    let b = luminance(background);
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}

#[test]
fn source_editor_stays_borderless_when_focused() {
    use iced::widget::text_editor::Status::Focused;
    for theme in [Theme::Light, Theme::Dark] {
        for is_hovered in [false, true] {
            assert_eq!(code_editor(&theme, Focused { is_hovered }).border.width, 0.);
        }
    }
}

#[test]
fn field_focus_and_validation_remain_visible_in_both_themes() {
    use iced::widget::text_input::Status::*;
    let focused = Focused { is_hovered: false };
    for theme in [Theme::Light, Theme::Dark] {
        let normal = validated_input(false);
        let invalid = validated_input(true);
        let active = normal(&theme, Active);
        let hovered = normal(&theme, Hovered);
        let valid_focus = normal(&theme, focused);
        let invalid_focus = invalid(&theme, focused);
        assert_ne!(active.border, hovered.border);
        assert_ne!(hovered.border.color, valid_focus.border.color);
        assert_ne!(valid_focus.border.color, invalid_focus.border.color);
        assert_ne!(
            invalid(&theme, Active).border,
            invalid(&theme, Hovered).border
        );
        assert!(valid_focus.border.width > active.border.width);
        assert!(invalid_focus.border.width > invalid(&theme, Active).border.width);
        for field in [valid_focus, invalid_focus] {
            let iced::Background::Color(background) = field.background else {
                panic!("Expected a solid field");
            };
            assert!(contrast(field.border.color, background) >= 3.);
            assert!(contrast(field.value, background) >= 4.5);
        }
        let disabled = normal(&theme, Disabled);
        assert_ne!(active.value, disabled.value);
        assert_ne!(active.background, disabled.background);
        assert_eq!(disabled, invalid(&theme, Disabled));
    }
}

#[test]
fn button_roles_remain_distinct_and_readable_through_interaction() {
    use button::Status::*;
    for theme in [Theme::Light, Theme::Dark] {
        let panel = DIALOG.resolve(&theme);
        for status in [Active, Hovered, Pressed] {
            let primary = primary(&theme, status);
            let secondary = secondary(&theme, status);
            let danger = danger(&theme, status);
            assert_ne!(primary.background, secondary.background);
            assert_ne!(danger.background, primary.background);
            assert_ne!(danger.background, secondary.background);
            for action in [primary, secondary, danger] {
                let iced::Background::Color(fill) = action.background.unwrap_or(panel.into())
                else {
                    panic!("Expected a solid button");
                };
                assert!(contrast(action.text_color, fill) >= 4.5);
            }
        }
        for role in [primary, secondary, danger] {
            assert_ne!(role(&theme, Active), role(&theme, Hovered));
            assert_ne!(role(&theme, Active), role(&theme, Disabled));
        }
    }
}

#[test]
fn checkboxes_and_round_switches_respond_to_pointer_and_availability() {
    for theme in [Theme::Light, Theme::Dark] {
        for checked in [false, true] {
            use iced_checkbox::Status as Check;
            let active = checkbox_style(
                &theme,
                Check::Active {
                    is_checked: checked,
                },
            );
            let hovered = checkbox_style(
                &theme,
                Check::Hovered {
                    is_checked: checked,
                },
            );
            let disabled = checkbox_style(
                &theme,
                Check::Disabled {
                    is_checked: checked,
                },
            );
            assert_ne!(active, hovered);
            assert_ne!(active, disabled);

            use toggler::Status as Switch;
            let active = toggle_style(
                &theme,
                Switch::Active {
                    is_toggled: checked,
                },
            );
            let hovered = toggle_style(
                &theme,
                Switch::Hovered {
                    is_toggled: checked,
                },
            );
            let disabled = toggle_style(
                &theme,
                Switch::Disabled {
                    is_toggled: checked,
                },
            );
            assert_ne!(active.background, hovered.background);
            assert_ne!(active.background, disabled.background);
            for state in [active, hovered, disabled] {
                assert!(state.border_radius.is_none(), "Iced draws a round switch");
            }
        }
    }
}

#[test]
fn light_text_remains_readable_on_neutral_surfaces() {
    for background in [BACKGROUND, CODE_BACKGROUND, SIDEBAR, ELEVATED] {
        for foreground in [
            TEXT,
            MUTED,
            FAINT,
            BLUE,
            GREEN,
            RED,
            YELLOW,
            FILE_ADDED,
            FILE_MODIFIED,
        ] {
            assert!(
                contrast(foreground.light, background.light) >= 4.5,
                "{foreground:?} on {background:?}"
            );
        }
    }
}

#[test]
fn syntax_colors_remain_readable_in_light_diff_rows() {
    for change in [
        crate::ui::code::LineChange::Added,
        crate::ui::code::LineChange::Modified,
        crate::ui::code::LineChange::Removed,
    ] {
        let color = unified_color(change).resolve(&Theme::Light);
        let iced::Background::Color(row) =
            unified_row(Some(change))(&Theme::Light).background.unwrap()
        else {
            panic!("Solid diff row");
        };
        let highlight = change_fill(unified_color(change), &Theme::Light, 0.30);
        let opacity = row.a + (1. - row.a) * highlight.a;
        let background = Color {
            r: color.r * opacity + CODE_BACKGROUND.light.r * (1. - opacity),
            g: color.g * opacity + CODE_BACKGROUND.light.g * (1. - opacity),
            b: color.b * opacity + CODE_BACKGROUND.light.b * (1. - opacity),
            a: 1.,
        };
        for foreground in [
            syntax::TEXT,
            syntax::KEYWORD,
            syntax::FUNCTION,
            syntax::STRING,
            syntax::COMMENT,
            syntax::NUMBER,
            syntax::PARAMETER,
            syntax::FIELD,
            syntax::ESCAPE,
            syntax::DECORATOR,
        ] {
            assert!(
                contrast(foreground.light, background) >= 4.5,
                "{change:?}: {foreground:?} on {background:?}"
            );
        }
    }
}
