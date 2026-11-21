use super::*;
use crate::i18n::{I18n, Language};

fn add(state: &mut State) -> u64 {
    let _ = state.update(Event::Start, Some(Path::new("project")));
    state.active.unwrap()
}

#[test]
fn local_titles_retain_their_number_when_renamed_closed_or_restored() {
    let ru = I18n::new(Language::Russian);
    let mut state = State::default();
    let dir = Some(Path::new("project/Тестовый каталог"));
    let _ = state.update(Event::Start, dir);
    let first = state.active.unwrap();
    let _ = state.update(Event::Start, dir);
    let second = state.active.unwrap();
    assert_eq!(state.tabs[0].title.label(&ru), "Терминал 1");
    assert_eq!(state.tabs[1].title.label(&ru), "Терминал 2");

    let rename = |state: &mut State, id, text: &str, finish| {
        for action in [
            TitleAction::Menu(iced::Point::ORIGIN),
            TitleAction::Rename,
            TitleAction::Edit(text.into()),
            finish,
        ] {
            let _ = state.update(Event::Title(id, action), None);
        }
    };
    rename(&mut state, second, "workers", TitleAction::Save);
    assert_eq!(state.tabs[1].title.label(&ru), "workers");
    rename(&mut state, second, "discarded", TitleAction::Dismiss);
    assert_eq!(state.tabs[1].title.label(&ru), "workers");
    let _ = state.update(Event::Start, dir);
    assert_eq!(state.tabs[2].title.label(&ru), "Терминал 3");
    let _ = state.update(Event::Close(first), None);
    rename(&mut state, second, "", TitleAction::Save);
    assert_eq!(state.tabs[0].title.label(&ru), "Терминал 2");
    assert_eq!(state.tabs[1].title.label(&ru), "Терминал 3");
}

#[test]
fn new_tabs_keep_independent_screens_sizes_and_active_selection() {
    let mut state = State::default();
    let first = add(&mut state);
    state.tabs[0].parser.process(b"first-output");
    let second = add(&mut state);
    state.tabs[1].parser.process(b"second-output");
    assert_ne!(first, second);
    let _ = state.update(Event::Resize(first, 12, 60), None);
    assert_eq!(state.tabs[0].parser.screen().size(), (12, 60));
    assert_eq!(state.tabs[1].parser.screen().size(), (24, 80));
    assert_eq!(state.active, Some(second));
    let _ = state.update(Event::Select(first), None);
    assert_eq!(
        state.active_tab().unwrap().parser.screen().contents(),
        "first-output"
    );
    let _ = state.update(Event::Select(second), None);
    assert_eq!(
        state.active_tab().unwrap().parser.screen().contents(),
        "second-output"
    );
}

#[test]
fn local_numbers_follow_their_project_and_remote_titles_keep_container_names() {
    let en = I18n::new(Language::English);
    let ru = I18n::new(Language::Russian);
    let mut state = State::default();
    let first = Some(Path::new("first"));
    let second = Some(Path::new("second"));
    let _ = state.update(Event::Start, first);
    for (program, title) in [
        (Program::Docker, "worker"),
        (Program::Docker, "worker"),
        (Program::Kubernetes, "pod/api"),
    ] {
        let _ = state.update(
            Event::StartProgram {
                program,
                title: title.into(),
                executable: "unused".into(),
                args: vec![],
            },
            first,
        );
    }
    state.switch_project(first, second);
    let _ = state.update(Event::Start, second);
    assert_eq!(state.tabs[0].title.label(&ru), "Терминал 1");
    state.switch_project(second, first);
    let _ = state.update(Event::Start, first);
    let labels: Vec<_> = state.tabs.iter().map(|tab| tab.title.label(&en)).collect();
    assert_eq!(
        labels,
        [
            "Terminal 1",
            "worker",
            "worker (2)",
            "pod/api",
            "Terminal 2"
        ]
    );
    assert_eq!(state.active_tab().unwrap().title.label(&ru), "Терминал 2");
}

#[test]
fn hiding_keeps_tabs_output_and_pending_sessions() {
    let mut state = State::default();
    let first = add(&mut state);
    state.tabs[0].parser.process(b"retained");
    let second = add(&mut state);
    let _ = state.update(Event::Hide, None);
    assert!(!state.open);
    assert_eq!(state.tabs.len(), 2);
    assert_eq!(state.active, Some(second));
    assert_eq!(state.tabs[0].request, first);
    assert_eq!(state.tabs[0].status, Status::Starting);
    assert_eq!(state.tabs[0].parser.screen().contents(), "retained");
}

#[test]
fn closing_tabs_preserves_another_selection_and_hides_only_after_the_last_tab() {
    let mut state = State::default();
    let first = add(&mut state);
    let second = add(&mut state);
    let third = add(&mut state);
    let _ = state.update(Event::Close(first), None);
    assert_eq!(state.active, Some(third));
    assert!(state.open);
    let _ = state.update(Event::Close(third), None);
    assert_eq!(state.active, Some(second));
    assert!(state.open);
    let _ = state.update(Event::Close(second), None);
    assert!(state.tabs.is_empty());
    assert_eq!(state.active, None);
    assert!(!state.open);
}

#[test]
fn closed_tabs_and_reset_sessions_cannot_receive_late_results() {
    let mut state = State::default();
    let closed = add(&mut state);
    let current = add(&mut state);
    let _ = state.update(Event::Close(closed), None);
    let _ = state.update(Event::Started(closed, Err(())), None);
    let _ = state.update(Event::Resize(closed, 1, 1), None);
    assert_eq!(state.active_tab().unwrap().status, Status::Starting);
    assert_eq!(state.active_tab().unwrap().parser.screen().size(), (24, 80));
    let _ = state.update(Event::Started(current, Err(())), None);
    assert_eq!(state.active_tab().unwrap().status, Status::Failed);
    state.reset();
    let next = add(&mut state);
    assert!(next > current);
    assert_eq!(
        state
            .active_tab()
            .unwrap()
            .title
            .label(&I18n::new(Language::English)),
        "Terminal 1"
    );
    let _ = state.update(Event::Started(current, Err(())), None);
    assert_eq!(state.active_tab().unwrap().status, Status::Starting);
}

#[test]
fn delayed_close_and_clipboard_events_cannot_change_the_new_projects_tab() {
    let a = Some(Path::new("first"));
    let b = Some(Path::new("second"));
    let mut state = State::default();
    let closed = add(&mut state);
    state.switch_project(a, b);
    let current = add(&mut state);
    let _ = state.update(Event::Close(closed), None);
    let _ = state.update(Event::Pasted(closed, Some("late input".into())), None);
    let _ = state.update(Event::Started(closed, Err(())), None);
    assert_eq!(state.active, Some(current));
    assert_eq!(state.active_tab().unwrap().status, Status::Starting);
    state.switch_project(b, a);
    assert!(state.tabs.is_empty());
    assert!(!state.open);
}

#[tokio::test]
async fn live_command_accepts_input_and_collects_output_in_a_background_project() {
    use iced::futures::{StreamExt, stream};
    use iced_test::runtime::{Action, task};
    use std::time::Duration;

    async fn receive_until(state: &mut State, request: u64, marker: &str) {
        let mut sources = iced::advanced::subscription::into_recipes(state.subscription());
        assert_eq!(sources.len(), 1);
        let mut events = sources.pop().unwrap().stream(Box::pin(stream::pending()));
        tokio::time::timeout(Duration::from_secs(15), async {
            while let Some(event) = events.next().await {
                let _ = state.update(event, None);
                if state
                    .tab_mut(request)
                    .unwrap()
                    .parser
                    .screen()
                    .contents()
                    .contains(marker)
                {
                    return;
                }
            }
            panic!("session ended before {marker}");
        })
        .await
        .unwrap();
    }

    let first = tempfile::tempdir().unwrap();
    let second = tempfile::tempdir().unwrap();
    let mut state = State::default();
    #[cfg(unix)]
    let command = {
        std::fs::write(
            first.path().join("command.sh"),
            "printf 'COMMAND-READY\\n'; read value; printf 'RESULT-%s\\n' \"$value\"; read hold",
        )
        .unwrap();
        b"/bin/sh ./command.sh\r".to_vec()
    };
    #[cfg(windows)]
    let command = {
        std::fs::write(first.path().join("command.cmd"), "@echo off\r\necho COMMAND-READY\r\nset /p value=\r\necho RESULT-%value%\r\nset /p hold=\r\n").unwrap();
        b"command.cmd\r\n".to_vec()
    };
    let start = state.update(Event::Start, Some(first.path()));
    let request = state.active.unwrap();
    let mut actions = task::into_stream(start).unwrap();
    while let Some(action) = actions.next().await {
        if let Action::Output(event) = action {
            let _ = state.update(event, None);
        }
    }
    let _ = state.update(Event::Input(request, command), None);
    receive_until(&mut state, request, "COMMAND-READY").await;
    state.switch_project(Some(first.path()), Some(second.path()));
    assert!(state.tabs.is_empty());
    let _ = state.update(Event::Start, Some(second.path()));
    let other = state.active.unwrap();
    let _ = state.update(Event::Input(request, b"background\r".to_vec()), None);
    receive_until(&mut state, request, "RESULT-background").await;
    assert_eq!(state.active, Some(other));
    assert!(
        state
            .active_tab()
            .unwrap()
            .parser
            .screen()
            .contents()
            .is_empty()
    );
    state.switch_project(Some(second.path()), Some(first.path()));
    let tab = state.active_tab().unwrap();
    assert_eq!(tab.request, request);
    assert_eq!(tab.status, Status::Running);
    assert!(tab.parser.screen().contents().contains("RESULT-background"));
    state.switch_project(Some(first.path()), Some(second.path()));
    if let Some(stopped) = state.lifetime.clone().shutdown() {
        stopped.await;
    }
    let mut sources = iced::advanced::subscription::into_recipes(state.subscription());
    let mut events = sources.pop().unwrap().stream(Box::pin(stream::pending()));
    tokio::time::timeout(Duration::from_secs(10), async {
        while let Some(event) = events.next().await {
            let _ = state.update(event, None);
        }
    })
    .await
    .unwrap();
    assert_eq!(state.tab_mut(request).unwrap().status, Status::Exited);
}

#[test]
fn mounted_focus_requests_apply_once_and_reject_obsolete_controls() {
    let mut state = State::default();
    let first = add(&mut state);
    let _ = state.active_tab_mut().unwrap().focus_screen();
    let _ = state.update(Event::Search(first, SearchAction::Open), None);
    let _ = state.update(Event::FocusReady(first, FocusTarget::Screen), None);
    assert_eq!(
        state.active_tab().unwrap().pending_focus,
        Some(FocusTarget::Search)
    );
    let second = add(&mut state);
    let _ = state.active_tab_mut().unwrap().focus_screen();
    let _ = state.update(Event::FocusReady(first, FocusTarget::Search), None);
    assert_eq!(
        state.active_tab().unwrap().pending_focus,
        Some(FocusTarget::Screen)
    );
    let _ = state.update(Event::FocusReady(second, FocusTarget::Screen), None);
    assert_eq!(state.active_tab().unwrap().pending_focus, None);
    let _ = state.update(Event::FocusReady(second, FocusTarget::Screen), None);
    assert_eq!(state.active_tab().unwrap().pending_focus, None);
}
