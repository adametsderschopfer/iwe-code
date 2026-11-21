use super::*;
use iced::keyboard::{Key, Modifiers, key::Named};

#[test]
fn tab_context_menu_and_rename_field_emit_actions_for_the_right_tab() {
    use crate::{i18n::Language, modules::terminal::state::TitleAction};
    use iced::widget::Id;

    for language in [Language::English, Language::Russian, Language::Spanish] {
        let i18n = I18n::new(language);
        let preferences = Default::default();
        let mut terminal = TerminalState::default();
        let _ = terminal.update(Event::Start, Some(std::path::Path::new("one")));
        let first = terminal.active.unwrap();
        let _ = terminal.update(Event::Start, Some(std::path::Path::new("two")));
        let mut ui = iced_test::Simulator::with_size(
            iced::Settings::default(),
            (900., 300.),
            view(&terminal, &i18n, &preferences, true),
        );
        let point = ui
            .find(Id::from(format!("terminal-tab-{first}")))
            .unwrap()
            .visible_bounds()
            .unwrap()
            .center();
        ui.point_at(point);
        ui.simulate([iced::Event::Mouse(mouse::Event::ButtonPressed(
            mouse::Button::Right,
        ))]);
        let menu = ui
            .into_messages()
            .find(|e| {
                matches!(e,
            Event::Title(id, TitleAction::Menu(anchor)) if *id == first && *anchor == point)
            })
            .unwrap();
        let _ = terminal.update(menu, None);
        let base = view(&terminal, &i18n, &preferences, true);
        let mut ui = iced_test::Simulator::with_size(
            iced::Settings::default(),
            (900., 300.),
            tab_menu(&terminal, &i18n, base, |e| e),
        );
        ui.click(Id::new("terminal-rename-action")).unwrap();
        let rename = ui
            .into_messages()
            .find(|e| matches!(e, Event::Title(id, TitleAction::Rename) if *id == first))
            .unwrap();
        let _ = terminal.update(rename, None);
        assert_eq!(terminal.active, Some(first));
        for key in [Named::Enter, Named::Escape] {
            let mut ui = iced_test::Simulator::with_size(
                iced::Settings::default(),
                (900., 300.),
                view(&terminal, &i18n, &preferences, true),
            );
            ui.click(Id::from(format!("terminal-rename-{first}")))
                .unwrap();
            ui.tap_key(Key::Named(key));
            assert!(ui.into_messages().any(|e| match e {
                Event::Title(id, TitleAction::Save) => id == first && key == Named::Enter,
                Event::Title(id, TitleAction::Dismiss) => id == first && key == Named::Escape,
                _ => false,
            }));
        }
    }
}

#[test]
fn terminal_input_supports_unicode_interrupts_and_cursor_modes() {
    assert_eq!(
        input::physical_control(keyboard::key::Physical::Code(keyboard::key::Code::KeyC)),
        Some(vec![3])
    );
    assert_eq!(
        input::encode(
            &Key::Character("ж".into()),
            Some("Ж"),
            Modifiers::SHIFT,
            false
        ),
        Some("Ж".as_bytes().to_vec())
    );
    assert_eq!(
        input::encode(&Key::Character("c".into()), None, Modifiers::CTRL, false),
        Some(vec![3])
    );
    assert_eq!(
        input::encode(&Key::Named(Named::ArrowUp), None, Modifiers::empty(), false),
        Some(b"\x1b[A".to_vec())
    );
    assert_eq!(
        input::encode(&Key::Named(Named::ArrowUp), None, Modifiers::empty(), true),
        Some(b"\x1bOA".to_vec())
    );
    assert_eq!(
        input::encode(&Key::Named(Named::Tab), None, Modifiers::SHIFT, false),
        Some(b"\x1b[Z".to_vec())
    );
    assert!(input::encode(&Key::Character("w".into()), None, Modifiers::LOGO, false).is_none());
}

#[test]
fn escape_sequences_update_screen_colors_cursor_and_scrollback() {
    let mut parser = vt100::Parser::new(3, 12, 20);
    parser.process(b"\x1b[31mred\x1b[0m\r\nline2\r\nline3\r\nline4");
    assert!(parser.screen().contents().contains("line4"));
    parser.screen_mut().set_scrollback(20);
    assert!(parser.screen().contents().contains("red"));
    assert_eq!(
        parser.screen().cell(0, 0).unwrap().fgcolor(),
        vt100::Color::Idx(1)
    );
    parser.screen_mut().set_scrollback(0);
    parser.process(b"\x1b[2J\x1b[Hnew");
    assert_eq!(parser.screen().contents(), "new");
    assert_eq!(parser.screen().cursor_position(), (0, 3));
}

#[test]
fn selection_copies_cells_without_escape_sequences_or_wide_continuations() {
    let mut parser = vt100::Parser::new(3, 12, 0);
    parser.process("\x1b[32mab界\r\ncd".as_bytes());
    let screen = Screen {
        screen: parser.screen(),
        search: &Search::default(),
        pending_focus: None,
        request: 1,
        revision: 0,
        size: 14.,
        line_height: 1.4,
        font: theme::code_font(),
        interactive: true,
        running: true,
    };
    let state = State {
        anchor: Some((0, 1)),
        selection: Some((1, 1)),
        ..State::default()
    };
    assert_eq!(screen.selected(&state), "b界\ncd");
}

#[test]
fn terminal_screen_revisions_track_resize_and_real_scrollback_changes() {
    let mut terminal = TerminalState::default();
    let _ = terminal.update(Event::Start, Some(std::path::Path::new("project")));
    let request = terminal.active.unwrap();
    let revision = terminal.active_tab().unwrap().revision;
    let _ = terminal.update(Event::Resize(request, 24, 80), None);
    let _ = terminal.update(Event::Scroll(request, 0), None);
    assert_eq!(terminal.active_tab().unwrap().revision, revision);
    let _ = terminal.update(Event::Resize(request, 3, 12), None);
    assert_ne!(terminal.active_tab().unwrap().revision, revision);
    terminal.tabs[0].parser.process(b"1\r\n2\r\n3\r\n4");
    let revision = terminal.active_tab().unwrap().revision;
    let _ = terminal.update(Event::Scroll(request, 2), None);
    assert!(terminal.active_tab().unwrap().parser.screen().scrollback() > 0);
    assert_ne!(terminal.active_tab().unwrap().revision, revision);
    let revision = terminal.active_tab().unwrap().revision;
    let _ = terminal.update(Event::Hide, None);
    assert_eq!(terminal.active_tab().unwrap().revision, revision);
}

#[tokio::test]
async fn terminal_redraw_reuses_geometry_and_invalidates_each_visual_change() {
    use iced::widget::canvas::Program;
    use iced_test::core::renderer::Headless;
    let renderer = Renderer::new(theme::font(), 16.into(), Some("tiny-skia"))
        .await
        .unwrap();
    let mut parser = vt100::Parser::new(3, 12, 0);
    parser.process(b"cached screen");
    let mut screen = Screen {
        screen: parser.screen(),
        search: &Search::default(),
        pending_focus: None,
        request: 1,
        revision: 0,
        size: 14.,
        line_height: 1.4,
        font: theme::code_font(),
        interactive: true,
        running: true,
    };
    let mut state = State::default();
    let mut bounds = Rectangle::with_size(Size::new(240., 120.));
    for _ in 0..100 {
        screen.draw(
            &state,
            &renderer,
            &Theme::Dark,
            bounds,
            mouse::Cursor::Unavailable,
        );
    }
    assert_eq!(state.paints.get(), 1);
    for change in 0..12 {
        let mut appearance = Theme::Dark;
        match change {
            0 => screen.revision += 1,
            1 => screen.request += 1,
            2 => state.focused = true,
            3 => {
                state.anchor = Some((0, 0));
                state.selection = Some((0, 3));
            }
            4 => screen.interactive = false,
            5 => screen.size = 16.,
            6 => appearance = Theme::Light,
            7 => {}
            8 => bounds.width += 40.,
            9 => {
                screen.interactive = true;
                screen.running = false;
                state.selection = None;
            }
            10 => screen.line_height = 1.8,
            11 => screen.font = iced::Font::with_name("JetBrains Mono"),
            _ => unreachable!(),
        }
        let before = state.paints.get();
        screen.draw(
            &state,
            &renderer,
            &appearance,
            bounds,
            mouse::Cursor::Unavailable,
        );
        assert_eq!(state.paints.get(), before + 1, "change {change}");
        screen.draw(
            &state,
            &renderer,
            &appearance,
            bounds,
            mouse::Cursor::Unavailable,
        );
        assert_eq!(state.paints.get(), before + 1);
    }
}

#[test]
fn terminal_canvas_sends_keys_only_after_a_focus_click() {
    let parser = vt100::Parser::new(24, 80, 0);
    let screen = Screen {
        screen: parser.screen(),
        search: &Search::default(),
        pending_focus: None,
        request: 42,
        revision: 0,
        size: 14.,
        line_height: 1.4,
        font: theme::code_font(),
        interactive: true,
        running: true,
    };
    let content = container(
        canvas::Canvas::new(screen)
            .width(Length::Fill)
            .height(Length::Fill),
    )
    .id("input-screen")
    .width(Length::Fill)
    .height(Length::Fill);
    let mut ui = iced_test::Simulator::with_size(iced::Settings::default(), (800., 500.), content);
    ui.tap_key(keyboard::Key::Character("x".into()));
    ui.click(iced::widget::Id::new("input-screen")).unwrap();
    ui.tap_key(keyboard::Key::Character("y".into()));
    let events = ui.into_messages().collect::<Vec<_>>();
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, Event::Input(_, bytes) if bytes == b"x"))
    );
    assert!(events.iter().any(|event| matches!(event, Event::Focus(42))));
    assert!(
        events
            .iter()
            .any(|event| matches!(event, Event::Input(42, bytes) if bytes == b"y"))
    );
}

#[test]
fn named_space_is_sent_to_the_shell() {
    assert_eq!(
        input::encode(&Key::Named(Named::Space), None, Modifiers::empty(), false),
        Some(vec![b' '])
    );
}

#[test]
fn terminal_line_spacing_changes_grid_dimensions() {
    let parser = vt100::Parser::new(10, 20, 0);
    let mut screen = Screen {
        screen: parser.screen(),
        search: &Search::default(),
        pending_focus: None,
        request: 1,
        revision: 0,
        size: 14.,
        line_height: 1.4,
        font: theme::code_font(),
        interactive: true,
        running: true,
    };
    let bounds = Rectangle::with_size(Size::new(500., 500.));
    let (rows, columns) = screen.dimensions(bounds);
    screen.line_height = 2.;
    let (larger_rows, larger_columns) = screen.dimensions(bounds);
    assert!(larger_rows < rows);
    assert_eq!(larger_columns, columns);
}

fn press(key: keyboard::Key, code: keyboard::key::Code, modifiers: Modifiers) -> iced::Event {
    iced::Event::Keyboard(keyboard::Event::KeyPressed {
        modified_key: key.clone(),
        key,
        physical_key: keyboard::key::Physical::Code(code),
        location: keyboard::Location::Standard,
        modifiers,
        repeat: false,
        text: None,
    })
}

#[test]
fn search_floats_inside_the_screen_and_only_intercepts_clicks_on_its_panel() {
    use crate::i18n::Language;
    use iced::widget::Id;

    let i18n = I18n::new(Language::English);
    let preferences = Default::default();
    for width in [320., 900.] {
        let mut terminal = TerminalState::default();
        let _ = terminal.update(Event::Start, Some(std::path::Path::new("project")));
        let request = terminal.active.unwrap();
        let mut closed = iced_test::Simulator::with_size(
            iced::Settings::default(),
            (width, 300.),
            view(&terminal, &i18n, &preferences, true),
        );
        let screen = closed
            .find(Id::new("terminal-screen"))
            .unwrap()
            .visible_bounds()
            .unwrap();
        drop(closed);
        let _ = terminal.update(Event::Search(request, SearchAction::Open), None);
        let mut ui = iced_test::Simulator::with_size(
            iced::Settings::default(),
            (width, 300.),
            view(&terminal, &i18n, &preferences, true),
        );
        assert_eq!(
            ui.find(Id::new("terminal-screen")).unwrap().visible_bounds(),
            Some(screen)
        );
        let panel = ui
            .find(Id::new("terminal-search-panel"))
            .unwrap()
            .visible_bounds()
            .unwrap();
        assert_eq!(panel.y, screen.y + 6.);
        assert_eq!(panel.x + panel.width, screen.x + screen.width - 8.);
        assert_eq!(panel.width, 360_f32.min(screen.width - 16.));
        let press = iced::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left));
        let release = iced::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left));
        ui.point_at(Point::new(panel.x + 2., panel.y + 2.));
        ui.simulate([press.clone(), release.clone()]);
        ui.point_at(Point::new(screen.x + 2., screen.y + screen.height - 2.));
        ui.simulate([press, release]);
        let count = ui
            .into_messages()
            .filter(|event| matches!(event, Event::Focus(id) if *id == request))
            .count();
        assert_eq!(count, 1);
    }
}

#[test]
fn focused_terminal_captures_find_and_preserves_shell_ctrl_f() {
    let parser = vt100::Parser::new(24, 80, 2);
    let search = Search::default();
    let screen = Screen {
        screen: parser.screen(),
        search: &search,
        pending_focus: None,
        request: 42,
        revision: 0,
        size: 14.,
        line_height: 1.4,
        font: theme::code_font(),
        interactive: true,
        running: true,
    };
    let mut ui = iced_test::Simulator::new(container(focus::view(screen)).id("test-screen"));
    ui.click(iced::widget::Id::new("test-screen")).unwrap();
    let find = if cfg!(target_os = "macos") {
        Modifiers::LOGO
    } else {
        Modifiers::CTRL | Modifiers::SHIFT
    };
    let key = Key::Character("а".into());
    ui.simulate([press(key.clone(), keyboard::key::Code::KeyF, find)]);
    ui.simulate([press(key, keyboard::key::Code::KeyF, Modifiers::CTRL)]);
    let events = ui.into_messages().collect::<Vec<_>>();
    assert!(
        events
            .iter()
            .any(|e| matches!(e, Event::Search(42, SearchAction::Open)))
    );
    assert!(
        events
            .iter()
            .any(|e| matches!(e, Event::Input(42, bytes) if bytes == &[6]))
    );
}

#[test]
fn search_keys_reach_only_the_focused_search_field_in_each_language() {
    use crate::i18n::Language;
    for language in [Language::English, Language::Russian, Language::Spanish] {
        let i18n = I18n::new(language);
        let search = Search::default();
        let controls = search::bar(&i18n, 42, &search, true);
        let mut ui = iced_test::Simulator::new(iced::widget::column![
            controls,
            theme::compact_field("other", "")
                .id("other")
                .on_input(|_| Event::Start),
        ]);
        ui.click(iced::widget::Id::from(super::super::state::search_id(42)))
            .unwrap();
        for modifier in [Modifiers::NONE, Modifiers::SHIFT] {
            ui.simulate([press(
                Key::Named(Named::Enter),
                keyboard::key::Code::Enter,
                modifier,
            )]);
        }
        ui.tap_key(Key::Named(Named::Escape));
        ui.click(iced::widget::Id::new("other")).unwrap();
        ui.tap_key(Key::Named(Named::Enter));
        let events = ui.into_messages().collect::<Vec<_>>();
        assert!(
            matches!(
                &events[..],
                [
                    Event::Search(42, SearchAction::Move(true)),
                    Event::Search(42, SearchAction::Move(false)),
                    Event::Search(42, SearchAction::Close),
                ]
            ),
            "{events:?}"
        );
    }
}

#[test]
fn a_new_canvas_accepts_keys_after_its_first_redraw_without_a_mouse_click() {
    let parser = vt100::Parser::new(24, 80, 2);
    let search = Search::default();
    let screen = Screen {
        screen: parser.screen(),
        search: &search,
        pending_focus: Some(FocusTarget::Screen),
        request: 92,
        revision: 0,
        size: 14.,
        line_height: 1.4,
        font: theme::code_font(),
        interactive: true,
        running: true,
    };
    let mut ui = iced_test::Simulator::new(focus::view(screen));
    ui.simulate([iced::Event::Window(window::Event::RedrawRequested(
        iced::time::Instant::now(),
    ))]);
    ui.tap_key(Key::Character("x".into()));
    let events = ui.into_messages().collect::<Vec<_>>();
    assert!(
        events
            .iter()
            .any(|e| matches!(e, Event::FocusReady(92, FocusTarget::Screen)))
    );
    assert!(
        events
            .iter()
            .any(|e| matches!(e, Event::Input(92, bytes) if bytes == b"x"))
    );
}
