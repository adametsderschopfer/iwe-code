use super::*;
use crate::{
    domain::{theme::ColorRole, workspace::Workspace},
    i18n::{I18n, Language},
    ui::theme,
};
use iced::{Color, Font};
use std::rc::Rc;

fn definition() -> Definition {
    serde_json::from_slice(
        br##"{
        "version": 1,
        "id": "test-theme",
        "name": "Test Theme",
        "dark": {
            "colors": {
                "background": "#101825",
                "syntax-keyword": "#ddaa99",
                "ansi-red": "#ba5e52"
            },
            "fonts": { "interface": "monospace", "editor": "serif" },
            "metrics": { "control-radius": 9 },
            "icons": { "settings": { "path": "M 3 3 H 17 V 17 H 3 Z", "fill": true } }
        },
        "light": { "colors": { "background": "#d5ddff" } }
    }"##,
    )
    .unwrap()
}

#[test]
fn workspace_theme_applies_both_variants_and_survives_metadata_reload() {
    let mut app = App::new(Rc::new(I18n::new(Language::English)));
    let definition = definition();
    assert!(definition.is_valid());
    let _ = app.update(Message::Themes(Event::Loaded(vec![definition.clone()])));
    let _ = app.update(Message::Themes(Event::Imported(Ok(Some(Box::new(
        definition.clone(),
    ))))));
    let dark = app.theme();
    assert_eq!(
        theme::application_style(&dark).background_color,
        Color::from_rgb8(16, 24, 37)
    );
    assert_eq!(
        theme::syntax::KEYWORD.resolve(&dark),
        Color::from_rgb8(221, 170, 153)
    );
    assert_eq!(
        dark.color(ColorRole::AnsiRed),
        Color::from_rgb8(186, 94, 82)
    );
    assert_eq!(dark.control_radius(), 9.);
    assert!(dark.icon("settings").is_some());
    dark.with_typography(|| {
        assert_eq!(theme::font(), Font::MONOSPACE);
        assert_eq!(theme::code_font().family, iced::font::Family::Serif);
    });
    assert_eq!(theme::font(), theme::Theme::Dark.interface_font());

    let _ = app.update(Message::SetAppearance(Appearance::Light));
    let bytes = serde_json::to_vec(&app.workspace).unwrap();
    let mut restored = App::new(Rc::new(I18n::new(Language::English)));
    restored.workspace = serde_json::from_slice::<Workspace>(&bytes).unwrap();
    assert_eq!(restored.theme(), Theme::Light);
    let _ = restored.update(Message::Themes(Event::Loaded(vec![definition])));
    let light = restored.theme();
    assert_eq!(
        theme::application_style(&light).background_color,
        Color::from_rgb8(213, 221, 255)
    );
    assert_eq!(
        theme::TEXT.resolve(&light),
        theme::TEXT.resolve(&Theme::Light)
    );
    assert!(light.icon("settings").is_none());
    let _ = restored.update(Message::Themes(Event::Select(BUILTIN_ID.to_owned())));
    assert_eq!(restored.theme(), Theme::Light);
    assert!(restored.workspace.preferences.theme.is_none());
}

#[test]
fn bundled_islands_is_available_without_import_and_preserves_appearance() {
    let i18n = Rc::new(I18n::new(Language::English));
    let mut app = App::new(Rc::clone(&i18n));
    assert_eq!(app.theme(), Theme::Dark);

    let _ = app.update(Message::Themes(Event::Select(ISLANDS_ID.to_owned())));
    assert_eq!(app.workspace.preferences.theme.as_deref(), Some(ISLANDS_ID));
    let dark = app.theme();
    assert_ne!(
        theme::application_style(&dark),
        theme::application_style(&Theme::Dark)
    );
    dark.with_typography(|| {
        assert_eq!(theme::code_font(), Font::with_name("JetBrains Mono"));
    });

    let _ = app.update(Message::SetAppearance(Appearance::Light));
    let mut restored = App::new(i18n);
    restored.workspace =
        serde_json::from_slice(&serde_json::to_vec(&app.workspace).unwrap()).unwrap();
    assert_eq!(restored.theme(), Theme::islands(Appearance::Light));
    assert_ne!(restored.theme(), dark);

    let _ = restored.update(Message::Themes(Event::Select(BUILTIN_ID.to_owned())));
    assert_eq!(restored.theme(), Theme::Light);
}

#[test]
fn theme_settings_offer_import_in_every_language() {
    for language in [Language::English, Language::Russian, Language::Spanish] {
        let mut app = App::new(Rc::new(I18n::new(language)));
        app.loaded = true;
        let _ = app.update(Message::Themes(Event::Loaded(vec![definition()])));
        let _ = app.open_settings();
        let window = app.windows.settings.unwrap();
        let size = super::super::windows::settings_settings().min_size.unwrap();
        let mut ui = iced_test::Simulator::with_size(
            iced::Settings::default(),
            size,
            app.view_window(window),
        );
        assert!(ui.find(app.i18n.text(Key::Theme)).is_ok());
        let picker = ui
            .find(iced::widget::Id::new("settings-theme-select"))
            .unwrap()
            .visible_bounds()
            .unwrap();
        let button = ui
            .click(iced::widget::Id::new("settings-theme-import"))
            .unwrap()
            .visible_bounds()
            .unwrap();
        assert!(picker.x + picker.width <= button.x);
        assert!(button.x + button.width <= size.width);
        assert!(ui.into_messages().any(|event| matches!(
            crate::app::modules::host_message(event),
            Message::Themes(Event::Import)
        )));
    }
}

#[test]
fn changing_the_theme_refreshes_an_existing_icon_in_the_same_appearance() {
    use crate::ui::icons::{Icon, icon};
    let original = definition();
    let mut replacement = original.clone();
    let shape = replacement.dark.icons.get_mut("settings").unwrap();
    shape.path = "M 3 3 H 7 V 7 H 3 Z".to_owned();
    let first = InstalledTheme::from_definition(original).variant(Appearance::Dark);
    let second = InstalledTheme::from_definition(replacement).variant(Appearance::Dark);

    let mut ui = iced_test::Simulator::with_size(
        iced::Settings::default(),
        (20., 20.),
        icon::<()>(Icon::Settings, theme::TEXT),
    );
    let output = tempfile::tempdir().unwrap();
    let path = output.path().join("settings-icon");
    assert!(ui.snapshot(&first).unwrap().matches_image(&path).unwrap());
    assert!(!ui.snapshot(&second).unwrap().matches_image(&path).unwrap());
}
