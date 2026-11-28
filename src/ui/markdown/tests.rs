use super::*;
use iced::widget::Id;

#[test]
fn preview_displays_fenced_and_indented_code_in_both_themes() {
    let items: Vec<_> = markdown::parse(
        "# Heading\n\n> A quote\n\n1. *First*\n2. **Second**\n\n```rust\nlet x = 42;\n```\n\n    sample()\n",
    )
    .collect();
    for appearance in [Theme::Dark, Theme::Light] {
        let mut ui = iced_test::Simulator::with_size(
            iced::Settings::default(),
            (320., 550.),
            view(
                &items,
                &appearance,
                theme::code_font(),
                14,
                (0, 0),
                &Shortcuts::default(),
            ),
        );
        assert!(ui.find(Id::new("markdown-preview-pane")).is_ok());
        assert!(ui.find("let x = 42;").is_ok());
        assert!(ui.find("sample()").is_ok());
        assert!(ui.find("1.").is_ok());
        assert!(ui.find("2.").is_ok());
    }
}

#[test]
fn reading_column_centers_on_wide_panes_and_wraps_in_narrow_panes() {
    let heading = "A heading with room to breathe";
    let paragraph = "A paragraph keeps its left margin and wraps with the document column.";
    let items: Vec<_> = markdown::parse(&format!(
        "# {heading}\n\n{paragraph}\n\n```\nexample\n```\n"
    ))
    .collect();
    let measure = |width| {
        let mut ui = iced_test::Simulator::with_size(
            iced::Settings::default(),
            (width, 550.),
            view(
                &items,
                &Theme::Dark,
                theme::code_font(),
                14,
                (0, 0),
                &Shortcuts::default(),
            ),
        );
        let document = ui
            .find(Id::new("markdown-document"))
            .unwrap()
            .visible_bounds()
            .unwrap();
        let code = ui.find("example").unwrap().visible_bounds().unwrap();
        (document, code)
    };

    let (wide, wide_code) = measure(1280.);
    let (narrow, narrow_code) = measure(320.);

    assert!((600. ..750.).contains(&wide.width));
    assert!((wide.center_x() - 640.).abs() < 1.);
    assert!(narrow.x > 0. && narrow.x + narrow.width < 320.);
    assert!(narrow.width < wide.width);
    assert!((wide_code.x - wide.x - (narrow_code.x - narrow.x)).abs() < 1.);
    assert!(narrow_code.y > wide_code.y + 30.);
}

fn shortcut(key: &str) -> iced::Event {
    use iced::{Event, keyboard};
    let mut event = iced_test::simulator::press_key(keyboard::Key::Character(key.into()), None);
    if let Event::Keyboard(keyboard::Event::KeyPressed { modifiers, .. }) = &mut event {
        *modifiers = keyboard::Modifiers::COMMAND;
    }
    event
}

#[test]
fn selection_copies_rendered_text_across_blocks_and_preserves_link_clicks() {
    use iced::{Event, Point, mouse};

    let items: Vec<_> = markdown::parse(
        "# Привет\n\n[link](https://example.com) **mundo**\n\n```\nline one\nline two\n```",
    )
    .collect();
    let mut ui = iced_test::Simulator::with_size(
        iced::Settings::default(),
        (540., 550.),
        view(
            &items,
            &Theme::Dark,
            theme::code_font(),
            14,
            (1, 0),
            &Shortcuts::default(),
        ),
    );

    let heading = ui.find("Привет").unwrap().visible_bounds().unwrap();
    let paragraph = ui.find("link mundo").unwrap().visible_bounds().unwrap();
    let press = Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left));
    let release = Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left));

    ui.point_at(Point::new(paragraph.x + 3., paragraph.center_y()));
    ui.simulate([press.clone(), release.clone()]);
    ui.point_at(Point::new(heading.x + 0.5, heading.center_y()));
    ui.simulate([press.clone()]);
    let end = Point::new(paragraph.x + paragraph.width - 0.5, paragraph.center_y());
    ui.point_at(end);
    ui.simulate([
        Event::Mouse(mouse::Event::CursorMoved { position: end }),
        release.clone(),
        shortcut("c"),
    ]);

    ui.point_at(Point::new(paragraph.x + 1., paragraph.center_y()));
    ui.simulate([press]);
    let end = Point::new(heading.x + 0.5, heading.center_y());
    ui.point_at(end);
    ui.simulate([
        Event::Mouse(mouse::Event::CursorMoved { position: end }),
        release,
        shortcut("a"),
        shortcut("c"),
    ]);

    assert_eq!(
        ui.into_messages().collect::<Vec<_>>(),
        [
            Action::Link("https://example.com".into()),
            Action::Copy("Привет\nlink mundo".into()),
            Action::Copy("Привет\nlink mundo\nline one\nline two".into()),
        ]
    );
}
