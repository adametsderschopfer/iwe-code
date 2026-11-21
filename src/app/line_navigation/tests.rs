use super::*;
use crate::i18n::{I18n, Language};
use std::{rc::Rc, sync::Arc};

fn app() -> App {
    let mut app = App::new(Rc::new(I18n::new(Language::English)));
    app.preview_mut().document = Some(Arc::new(Document::new(
        "one\ntwo\nthree\nfour\nfive\nsix\nseven\neight".into(),
        "txt",
    )));
    app
}

#[test]
fn pointer_hover_changes_only_the_enabled_dialog_button_in_both_themes() {
    use iced::{Point, Rectangle, Theme, mouse};
    use iced_test::Simulator;

    let mut app = app();
    let _ = app.update_line_navigation(Event::Open);
    app.line_navigation.as_mut().unwrap().loading = true;
    for theme in [Theme::Dark, Theme::Light] {
        let mut ui = Simulator::with_size(iced::Settings::default(), (640., 400.), app.view());
        let labels = [Key::Cancel, Key::Loading].map(|key| {
            ui.find(app.i18n.text(key))
                .unwrap()
                .visible_bounds()
                .unwrap()
        });
        let sample = |ui: &mut Simulator<'_, Message>, cursor: Point| {
            ui.point_at(cursor);
            ui.simulate([iced::Event::Mouse(mouse::Event::CursorMoved {
                position: cursor,
            })]);
            let directory = tempfile::tempdir().unwrap();
            ui.snapshot(&theme)
                .unwrap()
                .matches_image(directory.path().join("dialog"))
                .unwrap();
            let png = directory
                .path()
                .read_dir()
                .unwrap()
                .next()
                .unwrap()
                .unwrap()
                .path();
            let image = image::open(png).unwrap().into_rgba8();
            let scale = image.width() as f32 / 640.;
            // Sample the button padding beside each label to exclude glyphs and borders.
            labels.map(|label: Rectangle| {
                *image.get_pixel(
                    ((label.x - 4.) * scale) as u32,
                    (label.center_y() * scale) as u32,
                )
            })
        };
        let normal = sample(&mut ui, Point::ORIGIN);
        let cancel_hover = sample(&mut ui, labels[0].center());
        let disabled_hover = sample(&mut ui, labels[1].center());
        assert!(
            normal[0][0].abs_diff(cancel_hover[0][0]) >= 12,
            "{theme:?} Cancel fill changed from {:?} to {:?}",
            normal[0],
            cancel_hover[0]
        );
        assert_eq!(normal[1], cancel_hover[1], "{theme:?} disabled neighbor");
        assert_eq!(normal, disabled_hover, "{theme:?} disabled hover");
    }
}

#[test]
fn navigation_reveals_and_selects_an_existing_line_without_changing_source() {
    let mut app = app();
    let fingerprint = app.preview().document.as_ref().unwrap().fingerprint;
    let _ = app.update_line_navigation(Event::Open);
    let _ = app.update_line_navigation(Event::Changed("8".into()));
    assert!(app.update_line_navigation(Event::Submit).units() > 0);
    assert!(app.line_navigation.is_none());
    assert!(app.preview().scroll_y > 0.);
    assert_eq!(app.preview().selection.lane(), Some(Lane::Unified));
    let document = app.preview().document.as_ref().unwrap();
    assert!(
        app.preview()
            .selection
            .highlights_line(document, Lane::Unified, 7)
    );
    assert!(
        !app.preview()
            .selection
            .highlights_line(document, Lane::Unified, 6)
    );
    assert!(
        !app.preview()
            .selection
            .highlights_line(document, Lane::Before, 7)
    );
    assert_eq!(
        app.preview().document.as_ref().unwrap().fingerprint,
        fingerprint
    );
}

#[test]
fn invalid_and_missing_lines_keep_the_dialog_open_without_scrolling() {
    let mut app = app();
    let _ = app.update_line_navigation(Event::Open);
    for value in ["0", "-1", "abc", ""] {
        let _ = app.update_line_navigation(Event::Changed(value.into()));
        let _ = app.update_line_navigation(Event::Submit);
        assert_eq!(
            app.line_navigation.as_ref().unwrap().error,
            Some(Key::InvalidLineNumber)
        );
    }
    let _ = app.update_line_navigation(Event::Changed("999".into()));
    let _ = app.update_line_navigation(Event::Submit);
    assert_eq!(
        app.line_navigation.as_ref().unwrap().error,
        Some(Key::LineNumberNotFound)
    );
    assert_eq!(app.preview().scroll_y, 0.);
}

#[test]
fn navigation_highlights_empty_lines_and_clears_the_previous_target() {
    let mut app = app();
    app.preview_mut().document = Some(Arc::new(Document::new("one\n\nthree".into(), "txt")));
    assert!(app.reveal_source_line(2));
    let document = app.preview().document.clone().unwrap();
    assert!(
        app.preview()
            .selection
            .highlights_line(&document, Lane::Unified, 1)
    );
    assert!(app.reveal_source_line(3));
    assert!(
        !app.preview()
            .selection
            .highlights_line(&document, Lane::Unified, 1)
    );
    assert!(
        app.preview()
            .selection
            .highlights_line(&document, Lane::Unified, 2)
    );
    app.preview_mut().selection.update(Action::Clear, &document);
    assert!(
        !app.preview()
            .selection
            .highlights_line(&document, Lane::Unified, 2)
    );
}

#[test]
fn clicking_source_clears_the_navigation_highlight() {
    let mut app = app();
    assert!(app.reveal_source_line(1));
    let document = app.preview().document.clone().unwrap();
    app.preview_mut().selection.update(
        Action::Start(
            Lane::Unified,
            crate::ui::code::selection::Position { row: 0, byte: 0 },
        ),
        &document,
    );
    assert!(
        !app.preview()
            .selection
            .highlights_line(&document, Lane::Unified, 0)
    );
}
