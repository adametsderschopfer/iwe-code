use super::*;

#[test]
fn matches_history_and_soft_wrapped_wide_and_combining_cells() {
    let mut parser = vt100::Parser::new(2, 6, 8);
    parser.process("1234界é\u{301}x\r\nother\r\n1234界é\u{301}x".as_bytes());
    assert!(!parser.screen().contents().contains("other"));
    let mut search = Search {
        open: true,
        ..Search::default()
    };
    search.set_query("界é\u{301}".into(), parser.screen_mut());
    assert_eq!(search.count(), (1, 2));
    assert!(parser.screen().scrollback() > 0);
    for (row, col) in [(0, 4), (0, 5), (1, 0)] {
        assert_eq!(search.highlight(parser.screen(), row, col), Some(true));
    }
    assert_eq!(search.highlight(parser.screen(), 1, 1), None);
    search.navigate(false, parser.screen_mut());
    assert_eq!(search.count(), (2, 2));
    assert_eq!(parser.screen().scrollback(), 0);
    search.navigate(true, parser.screen_mut());
    assert_eq!(search.count(), (1, 2));
    assert!(parser.screen().scrollback() > 0);
}

#[test]
fn search_tracks_appended_rewritten_and_evicted_rows_and_restores_the_viewport() {
    let mut parser = vt100::Parser::new(2, 16, 3);
    parser.process(b"a item item\r\nb item\r\nc\r\nd\r\ne");
    let mut search = Search {
        open: true,
        ..Search::default()
    };
    search.set_query("item".into(), parser.screen_mut());
    assert_eq!(search.count(), (1, 3));
    assert_eq!(search.highlight(parser.screen(), 0, 7), Some(false));
    search.navigate(true, parser.screen_mut());
    search.navigate(true, parser.screen_mut());
    assert_eq!(search.count(), (3, 3));
    let previous = parser.screen().scrollback();
    search.process(&mut parser, b"\r\nf item");
    let expected_viewport = parser.screen().scrollback();
    search.refresh(parser.screen_mut(), previous);
    assert_eq!(search.count(), (1, 2));
    assert_eq!(parser.screen().scrollback(), expected_viewport);
    search.process(&mut parser, b"\r\x1b[2Kchanged");
    search.refresh(parser.screen_mut(), expected_viewport);
    assert_eq!(search.count(), (1, 1));
    search.process(&mut parser, b"\r\n1\r\n2\r\n3\r\n4\r\n5");
    search.refresh(parser.screen_mut(), expected_viewport);
    assert_eq!(search.count(), (0, 0));
    assert_eq!(search.highlight(parser.screen(), 0, 0), None);
}

#[test]
fn terminal_tabs_retain_their_queries_and_reject_late_search_input() {
    use crate::modules::terminal::{Event, State};
    let mut state = State::default();
    let root = std::path::Path::new("project");
    let _ = state.update(Event::Start, Some(root));
    let first = state.active.unwrap();
    let _ = state.update(Event::Search(first, Action::Open), Some(root));
    let _ = state.update(
        Event::Search(first, Action::Query("alpha".into())),
        Some(root),
    );
    let _ = state.update(Event::Search(first, Action::Close), Some(root));
    assert!(!state.active_tab().unwrap().search.open);
    let _ = state.update(Event::Start, Some(root));
    let second = state.active.unwrap();
    assert!(state.active_tab().unwrap().search.query.is_empty());
    let _ = state.update(Event::Search(second, Action::Open), Some(root));
    let _ = state.update(
        Event::Search(second, Action::Query("beta".into())),
        Some(root),
    );
    let _ = state.update(Event::Select(first), Some(root));
    let _ = state.update(
        Event::Search(second, Action::Query("late".into())),
        Some(root),
    );
    let _ = state.update(Event::Search(first, Action::Open), Some(root));
    assert_eq!(state.active_tab().unwrap().search.query, "alpha");
    let _ = state.update(Event::Select(second), Some(root));
    assert_eq!(state.active_tab().unwrap().search.query, "beta");
}

#[test]
fn search_ignores_control_sequences_and_unwritten_padding() {
    let mut parser = vt100::Parser::new(2, 10, 3);
    parser.process(b"\x1b[32mred red\x1b[0m");
    let mut search = Search {
        open: true,
        ..Search::default()
    };
    search.set_query(" ".into(), parser.screen_mut());
    assert_eq!(search.count(), (1, 1));
    for query in ["32m", "red\n", ""] {
        search.set_query(query.into(), parser.screen_mut());
        assert_eq!(search.count(), (0, 0));
    }
}

#[test]
fn retaining_a_full_history_preserves_the_selected_visible_match_after_eviction() {
    let mut parser = vt100::Parser::new(3, 20, 3);
    parser.process(b"match\r\nmatch\r\nmatch\r\nmatch\r\nmatch\r\nmatch");
    let mut search = Search {
        open: true,
        ..Search::default()
    };
    search.set_query("match".into(), parser.screen_mut());
    for _ in 0..4 {
        search.navigate(true, parser.screen_mut());
    }
    assert_eq!(search.count(), (5, 6));
    parser.screen_mut().set_scrollback(0);
    search.process(&mut parser, b"\r\nseven");
    search.refresh(parser.screen_mut(), 0);
    assert_eq!(search.count(), (4, 5));
    assert_eq!(search.highlight(parser.screen(), 0, 0), Some(true));
    assert_eq!(search.highlight(parser.screen(), 1, 0), Some(false));
}

#[test]
fn narrowing_the_terminal_keeps_matches_at_the_end_of_retained_rows_visible() {
    let mut parser = vt100::Parser::new(2, 15, 4);
    parser.process(b"abcdefghijMATCH\r\nsecond\r\nthird");
    parser.screen_mut().set_size(2, 6);
    let mut search = Search {
        open: true,
        ..Search::default()
    };
    search.set_query("MATCH".into(), parser.screen_mut());
    let screen = parser.screen();
    assert_eq!(search.count(), (1, 1));
    assert_eq!(screen.scrollback(), 1);
    assert_eq!(search.first_column(screen), 9);
    assert_eq!(search.highlight(screen, 0, 14), Some(true));
}

#[test]
fn search_counts_every_retained_match_at_the_terminal_history_limit() {
    let mut parser = vt100::Parser::new(3, 24, 2000);
    parser.process(b"evicted\r\n");
    for _ in 0..2004 {
        parser.process(b"match match\r\n");
    }
    let mut search = Search {
        open: true,
        ..Search::default()
    };
    search.set_query("match".into(), parser.screen_mut());
    assert_eq!(search.count(), (1, 4004));
    assert_eq!(parser.screen().scrollback(), 2000);
    search.set_query("evicted".into(), parser.screen_mut());
    assert_eq!(search.count(), (0, 0));
}
