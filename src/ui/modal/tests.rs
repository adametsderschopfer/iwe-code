use super::overlay;
use iced::{
    Event, Length,
    keyboard::{self, key::Named},
    widget::{Id, column, container, scrollable, space, text_editor, text_input},
};
use iced_test::{Simulator, simulator::press_key};

#[derive(Debug, Clone, PartialEq)]
enum Message {
    Edit(usize, String),
    Behind,
    Dismiss,
}

fn tab(backward: bool) -> Event {
    let mut event = press_key(keyboard::Key::Named(Named::Tab), None);
    if backward && let Event::Keyboard(keyboard::Event::KeyPressed { modifiers, .. }) = &mut event {
        *modifiers = keyboard::Modifiers::SHIFT;
    }
    event
}

#[test]
fn tab_cycles_between_editable_dialog_fields_and_reveals_the_focused_field() {
    let editor = text_editor::Content::new();
    let first = text_input("", "").on_input(|value| Message::Edit(1, value));
    let last = text_input("", "")
        .id("last")
        .on_input(|value| Message::Edit(2, value));
    let body = scrollable(column![
        first,
        text_input("Read only", ""),
        text_editor(&editor).height(48.),
        space().height(100),
        last
    ])
    .height(64.);
    let base = super::super::context_menu::on_key(
        container(text_input("Background", "").on_input(|_| Message::Behind)).center(Length::Fill),
        |_| Some(Message::Behind),
    );
    let view = overlay(base, body.into(), 300., Message::Dismiss);
    let mut ui = Simulator::with_size(iced::Settings::default(), (450., 300.), view);
    let last_visible = |ui: &mut Simulator<'_, Message>| {
        ui.find(Id::new("last")).unwrap().visible_bounds().is_some()
    };
    assert!(!last_visible(&mut ui));
    ui.typewrite("x");
    ui.simulate([tab(false)]);
    ui.typewrite("a");
    ui.simulate([tab(false)]);
    assert!(last_visible(&mut ui));
    ui.typewrite("b");
    ui.simulate([tab(true)]);
    ui.typewrite("c");
    ui.simulate([tab(true)]);
    ui.typewrite("d");
    ui.simulate([tab(false)]);
    ui.typewrite("e");
    ui.simulate([press_key(keyboard::Key::Named(Named::Escape), None)]);
    let messages = ui.into_messages().collect::<Vec<_>>();
    assert_eq!(
        messages,
        [
            Message::Edit(1, "a".into()),
            Message::Edit(2, "b".into()),
            Message::Edit(1, "ac".into()),
            Message::Edit(2, "bd".into()),
            Message::Edit(1, "ace".into()),
            Message::Dismiss
        ]
    );
}
