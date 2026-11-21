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
            view(&items, &appearance, theme::code_font(), 14),
        );
        assert!(ui.find(Id::new("markdown-preview-pane")).is_ok());
        assert!(ui.find("let x = 42;").is_ok());
        assert!(ui.find("sample()").is_ok());
        assert!(ui.find("1.").is_ok());
        assert!(ui.find("2.").is_ok());
    }
}
