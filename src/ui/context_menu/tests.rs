use super::*;
use iced::widget::{button, column, text_input};

#[derive(Debug, Clone, PartialEq)]
enum Message {
    Close,
    Edit(String),
    Background,
}

#[test]
fn escape_closes_dialogs_and_context_menus_with_focused_inputs() {
    for modal in [false, true] {
        let base = iced::widget::container(button("Background").on_press(Message::Background))
            .width(Length::Fill)
            .height(Length::Fill)
            .into();
        let contents = || {
            column![
                text_input("Name", "Draft")
                    .id("input")
                    .on_input(Message::Edit)
            ]
            .into()
        };
        let view = if modal {
            crate::ui::modal::overlay(base, contents(), 300., Message::Close)
        } else {
            overlay(
                base,
                Point::new(20., 20.),
                Size::new(300., 80.),
                contents,
                Message::Close,
            )
        };
        let mut ui = iced_test::Simulator::with_size(iced::Settings::default(), (520., 400.), view);
        ui.click(iced::widget::Id::new("input")).unwrap();
        ui.simulate([Event::Keyboard(keyboard::Event::KeyPressed {
            key: keyboard::Key::Named(keyboard::key::Named::Escape),
            modified_key: keyboard::Key::Named(keyboard::key::Named::Escape),
            physical_key: keyboard::key::Physical::Code(keyboard::key::Code::Escape),
            location: keyboard::Location::Standard,
            modifiers: keyboard::Modifiers::NONE,
            repeat: false,
            text: None,
        })]);
        assert_eq!(ui.into_messages().collect::<Vec<_>>(), [Message::Close]);
    }
}

#[test]
fn a_short_menu_stays_next_to_its_anchor_near_the_window_bottom() {
    let view = overlay(
        container(space())
            .width(Length::Fill)
            .height(Length::Fill)
            .into(),
        Point::new(20., 355.),
        Size::new(180., 350.),
        || {
            container(space())
                .width(Length::Fill)
                .height(24)
                .id("short-menu")
                .into()
        },
        Message::Close,
    );
    let mut ui = iced_test::Simulator::with_size(iced::Settings::default(), (520., 400.), view);
    let bounds = ui
        .find(iced::widget::Id::new("short-menu"))
        .unwrap()
        .visible_bounds()
        .unwrap();
    assert_eq!(bounds.y, 355. + f32::from(SURFACE_PADDING));
}

#[test]
fn menu_icons_align_labels_and_preserve_disabled_actions() {
    use crate::i18n::Key;
    let menu = column![
        action(Key::FileCopy, "Copy".into()).on_press(Message::Background),
        action(Key::FormatCode, "Format".into()).on_press(Message::Edit("format".into())),
        action(Key::FileTrash, "Trash".into()),
    ];
    let mut ui = iced_test::Simulator::with_size(iced::Settings::default(), (260., 110.), menu);
    let labels =
        ["Copy", "Format", "Trash"].map(|label| ui.find(label).unwrap().visible_bounds().unwrap());
    assert!(labels.iter().all(|label| label.x == labels[0].x));
    assert!(labels[0].x > f32::from(ROW_PADDING[1]) + 16.);
    for label in ["Copy", "Format", "Trash"] {
        ui.click(label).unwrap();
    }
    assert_eq!(
        ui.into_messages().collect::<Vec<_>>(),
        [Message::Background, Message::Edit("format".into())]
    );
}

#[test]
fn destructive_menu_actions_keep_red_text_when_hovered_or_selected() {
    use crate::{i18n::Key, ui::theme};
    use iced::widget::button::Status;
    for theme in [Theme::Dark, Theme::Light] {
        for selected in [false, true] {
            for status in [
                Status::Active,
                Status::Hovered,
                Status::Pressed,
                Status::Disabled,
            ] {
                let normal = action_style(Key::FileCopy, selected)(&theme, status);
                for key in [
                    Key::FileTrash,
                    Key::DeleteSpace,
                    Key::RemoveProjectFromSpace,
                    Key::GitDelete,
                    Key::GitBulkDelete,
                    Key::GitHistoryDeleteTag,
                    Key::GitHistoryReset,
                ] {
                    let destructive = action_style(key, selected)(&theme, status);
                    assert_eq!(destructive.background, normal.background);
                    if status == Status::Disabled {
                        assert_eq!(destructive.text_color, normal.text_color);
                    } else {
                        assert_eq!(destructive.text_color, theme::RED.resolve(&theme));
                        assert_ne!(destructive.text_color, normal.text_color);
                    }
                }
            }
        }
    }
}
