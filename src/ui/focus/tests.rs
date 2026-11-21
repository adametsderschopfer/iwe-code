use super::form;
use crate::ui::theme;
use iced::{
    keyboard::key::Named,
    widget::{Id, column},
};

#[test]
fn tab_moves_between_fields_only_inside_the_focused_form() {
    let panel = form(
        column![
            theme::text_field("", "").id("first").on_input(|_| "first"),
            theme::text_field("", "")
                .id("second")
                .on_input(|_| "second"),
        ]
        .into(),
    );
    let view = column![
        panel,
        theme::text_field("", "")
            .id("outside")
            .on_input(|_| "outside"),
    ];
    let mut ui = iced_test::simulator(view);

    ui.click(Id::new("outside")).unwrap();
    ui.tap_key(Named::Tab);
    ui.typewrite("x");
    ui.click(Id::new("first")).unwrap();
    ui.tap_key(Named::Tab);
    ui.typewrite("y");
    ui.tap_key(Named::Tab);
    ui.typewrite("z");

    assert_eq!(
        ui.into_messages().collect::<Vec<_>>(),
        ["outside", "second", "first"]
    );
}
