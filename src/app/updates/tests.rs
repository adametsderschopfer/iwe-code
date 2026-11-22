use super::*;
use crate::i18n::{I18n, Language};
use std::rc::Rc;

fn app() -> App {
    App::new(Rc::new(I18n::new(Language::English)))
}

#[test]
fn automatic_checks_throttle_focus_and_retry_failures_after_fifteen_minutes() {
    let now = SystemTime::UNIX_EPOCH + Duration::from_secs(10_000);
    let mut schedule = Schedule::default();
    assert!(schedule.due(Trigger::Periodic, now));
    schedule.last_attempt = Some(now);
    assert!(!schedule.due(Trigger::Focused, now + Duration::from_secs(899)));
    assert!(schedule.due(Trigger::Focused, now + RETRY_INTERVAL));
    assert!(!schedule.due(Trigger::Periodic, now + RETRY_INTERVAL));
    assert!(schedule.due(Trigger::Periodic, now + CHECK_INTERVAL));
    schedule.retry = true;
    assert!(!schedule.due(Trigger::Periodic, now + Duration::from_secs(899)));
    assert!(schedule.due(Trigger::Periodic, now + RETRY_INTERVAL));
    assert!(schedule.due(Trigger::Periodic, now - Duration::from_secs(1)));
}

fn offer(version: &str) -> Offer {
    Offer {
        release: Release {
            version: version.into(),
            tag: format!("v{version}"),
            asset: "test".into(),
            size: 1,
            sha256: String::new(),
            manifest: vec![],
            signature: vec![],
        },
        notes: vec!["Fixture release note".into()],
    }
}

#[test]
fn automatic_checks_respect_cooldown_and_offer_each_version_once() {
    let mut app = app();
    assert!(app.update_release(Event::Check).units() > 0);
    let _ = app.update_release(Event::Checked(Ok(None)));
    assert_eq!(
        app.update_release(Event::Automatic(Trigger::Focused))
            .units(),
        0
    );
    assert!(app.update_release(Event::Check).units() > 0);
    let _ = app.update_release(Event::Checked(Ok(Some(offer("2.0.0")))));
    assert!(app.update_dialog.open);
    let _ = app.update_release(Event::DismissDialog);

    for result in [Err(Error::Network), Ok(Some(offer("2.0.0")))] {
        app.update_schedule.last_attempt = None;
        assert!(
            app.update_release(Event::Automatic(Trigger::Focused))
                .units()
                > 0
        );
        let _ = app.update_release(Event::Checked(result));
        assert_eq!(app.updates.version(), Some("2.0.0"));
        assert!(!app.update_dialog.open);
    }

    assert!(app.update_release(Event::Check).units() > 0);
    let _ = app.update_release(Event::Checked(Ok(Some(offer("2.1.0")))));
    assert!(app.update_dialog.open);
    assert_eq!(app.updates.version(), Some("2.1.0"));
}

#[test]
fn checks_ignore_duplicate_requests_and_obsolete_completions() {
    let mut app = app();
    assert!(app.update_release(Event::Check).units() > 0);
    assert_eq!(app.update_release(Event::Check).units(), 0);
    let _ = app.update_release(Event::Checked(Ok(Some(offer("2.0.0")))));
    let _ = app.update_release(Event::Checked(Ok(None)));
    assert_eq!(app.updates.version(), Some("2.0.0"));
    assert!(app.update_release(Event::Check).units() > 0);
    assert!(app.update_release(Event::Download).units() > 0);
    assert_eq!(app.update_release(Event::Download).units(), 0);
    let _ = app.update_release(Event::Checked(Ok(Some(offer("2.1.0")))));
    assert!(matches!(app.updates, State::Downloading(_)));
    assert_eq!(app.updates.version(), Some("2.0.0"));
    let _ = app.update_release(Event::Downloaded(Err(Error::Invalid)));
    assert!(matches!(app.updates, State::Available(_)));
    assert!(app.update_dialog.download_error.is_some());
    assert!(app.update_release(Event::Download).units() > 0);
    assert!(app.update_dialog.download_error.is_none());
}

#[test]
fn download_schedules_install_only_when_the_user_keeps_it_selected() {
    for (selected, dismissed) in [(false, false), (true, false), (true, true)] {
        let mut app = app();
        app.updates = State::Available(offer("2.0.0").release);
        let _ = app.update_release(Event::OpenDialog);
        let _ = app.update_release(Event::InstallAfterDownload(selected));
        assert!(app.update_release(Event::Download).units() > 0);
        if dismissed {
            let _ = app.update_release(Event::DismissDialog);
        }
        let task = app.update_release(Event::Downloaded(Ok(Arc::new(Prepared::fixture()))));
        assert!(matches!(app.updates, State::Ready(_)));
        assert_eq!(task.units() > 0, selected && !dismissed);
        assert!(!app.update_dialog.install_after_download);
    }
}

#[test]
fn update_dialog_and_header_actions_fit_in_all_languages() {
    use crate::i18n::Key;
    use iced::widget::Id;

    for language in [Language::English, Language::Russian, Language::Spanish] {
        let mut app = App::new(Rc::new(I18n::new(language)));
        app.loaded = true;
        let _ = app.update_release(Event::Check);
        let _ = app.update_release(Event::Checked(Ok(Some(offer("2.0.0")))));
        let mut ui =
            iced_test::Simulator::with_size(iced::Settings::default(), (860., 520.), app.view());
        for label in [
            app.i18n.text(Key::UpdatePrompt),
            app.i18n.text(Key::UpdateInstallAfterDownload),
            app.i18n.text(Key::DownloadUpdate),
            env!("CARGO_PKG_VERSION").into(),
            "2.0.0".into(),
            "Fixture release note".into(),
        ] {
            assert!(ui.find(label).unwrap().visible_bounds().is_some());
        }
        ui.click(app.i18n.text(Key::UpdateLater)).unwrap();
        for message in ui.into_messages() {
            let _ = app.update(message);
        }
        assert!(!app.update_dialog.open);

        let mut ui =
            iced_test::Simulator::with_size(iced::Settings::default(), (860., 520.), app.view());
        let update = ui
            .find(Id::new("header-update"))
            .unwrap()
            .visible_bounds()
            .unwrap();
        let settings = ui
            .find(Id::new("nav-settings"))
            .unwrap()
            .visible_bounds()
            .unwrap();
        assert!(update.x >= settings.x + settings.width);
        assert!(update.y + update.height <= crate::ui::theme::WORKSPACE_HEADER_HEIGHT);
        ui.click(Id::new("header-update")).unwrap();
        for message in ui.into_messages() {
            let _ = app.update(message);
        }
        assert!(app.update_dialog.open);

        let mut ui =
            iced_test::Simulator::with_size(iced::Settings::default(), (860., 520.), app.view());
        ui.click(app.i18n.text(Key::DownloadUpdate)).unwrap();
        for message in ui.into_messages() {
            let _ = app.update(message);
        }
        assert!(matches!(app.updates, State::Downloading(_)));
        let _ = app.update_release(Event::Downloaded(Ok(Arc::new(Prepared::fixture()))));
        let mut ui =
            iced_test::Simulator::with_size(iced::Settings::default(), (860., 520.), app.view());
        assert!(
            ui.find(app.i18n.text(Key::InstallUpdate))
                .unwrap()
                .visible_bounds()
                .is_some()
        );
    }
}

#[test]
fn failed_handoff_keeps_the_app_open_and_reports_the_failure() {
    let mut app = app();
    app.updates = State::Installing(Arc::new(Prepared::fixture()));
    let task = app.update(Message::Update(Event::InstallFailed(Error::Install)));
    assert_eq!(task.units(), 0);
    assert!(matches!(app.updates, State::Failed(Error::Install)));
    assert!(!app.notifications.items.is_empty());
}

#[test]
fn installing_waits_for_the_latest_workspace_save_and_preserves_failed_saves() {
    use crate::domain::Failure;
    for success in [true, false] {
        let mut app = app();
        app.window_id = Some(iced::window::Id::unique());
        app.updates = State::Ready(Arc::new(Prepared::fixture()));
        // A save already in flight must finish before the latest snapshot is written.
        app.saving = true;
        let _ = app.update(Message::Update(Event::Install));
        assert!(matches!(app.updates, State::Saving(_)));
        assert!(app.save_pending);
        let first = app.update(Message::Saved(Ok(())));
        assert!(first.units() > 0);
        assert!(matches!(app.updates, State::Saving(_)));
        assert!(app.saving);
        assert!(!app.save_pending);
        let next = app.update(Message::Saved(if success {
            Ok(())
        } else {
            Err(Failure::Storage)
        }));
        if success {
            assert!(matches!(app.updates, State::Installing(_)));
            assert!(next.units() > 0);
        } else {
            assert!(matches!(app.updates, State::Failed(Error::Install)));
            assert_eq!(next.units(), 0);
        }
    }
}

#[tokio::test]
async fn update_handoff_keeps_terminal_completions_and_blocks_user_input() {
    use crate::{
        app::modules::Event as ModuleEvent,
        modules::terminal::{Event as T, Program, Status},
    };
    use iced::futures::{StreamExt, stream};
    use iced_test::runtime::{Action, task};

    let mut app = app();
    let dir = tempfile::tempdir().unwrap();
    app.workspace
        .create_space("Work".into(), vec![dir.path().into()]);
    #[cfg(unix)]
    let (executable, args) = (
        "/bin/sh",
        vec!["-c", "read answer; printf 'RESULT-%s\\n' \"$answer\""],
    );
    #[cfg(windows)]
    let (executable, args) = {
        std::fs::write(
            dir.path().join("test.cmd"),
            "@echo off\r\nset /p answer=\r\necho RESULT-%answer%\r\n",
        )
        .unwrap();
        ("cmd.exe", vec!["/D", "/Q", "/C", "test.cmd"])
    };
    let terminal = |event| Message::Module(ModuleEvent::Terminal(event));
    let start = app.update(terminal(T::StartProgram {
        program: Program::Docker,
        executable: executable.into(),
        args: args.into_iter().map(str::to_owned).collect(),
        title: "fixture".into(),
    }));
    let request = app.modules.terminal.active.unwrap();
    let update = Arc::new(Prepared::fixture());
    app.updates = State::Saving(update.clone());
    let mut actions = task::into_stream(start).unwrap();
    while let Some(action) = actions.next().await {
        if let Action::Output(message) = action {
            let _ = app.update(message);
        }
    }
    assert_eq!(
        app.modules.terminal.active_tab().unwrap().status,
        Status::Running
    );
    let _ = app.update(terminal(T::Input(request, b"blocked\r".to_vec())));
    app.updates = State::Idle;
    let _ = app.update(terminal(T::Input(request, b"allowed\r".to_vec())));
    app.updates = State::Installing(update);
    let _ = app.update(terminal(T::Close(request)));
    let mut recipes =
        iced::advanced::subscription::into_recipes(app.modules.terminal.subscription());
    let mut output = recipes.pop().unwrap().stream(Box::pin(stream::pending()));
    tokio::time::timeout(Duration::from_secs(15), async {
        while let Some(event) = output.next().await {
            let _ = app.update(terminal(event));
        }
    })
    .await
    .unwrap();
    let tab = app.modules.terminal.active_tab().unwrap();
    assert_eq!(tab.status, Status::Exited);
    assert!(tab.parser.screen().contents().contains("RESULT-allowed"));
}

#[test]
fn installed_version_notes_use_the_saved_language_and_open_the_exact_release_page() {
    let ledger = r#"[
        {"version":"1.0.0","items":[{"text":{"en":"Previous release"}}]},
        {"version":"2.0.0","items":[{"text":{"en":"Edit colors","ru":"Правка цветов","es":"Editar colores"}}]},
        {"version":"unreleased","items":[{"text":{"en":"Future release"}}]}
    ]"#;
    use crate::i18n::Key;

    for (language, expected) in [
        (Language::English, "Edit colors"),
        (Language::Russian, "Правка цветов"),
        (Language::Spanish, "Editar colores"),
    ] {
        let mut app = App::new(Rc::new(I18n::new(language)));
        app.loaded = true;
        app.whats_new = Some(WhatsNew::from_ledger("2.0.0", ledger));
        let mut ui =
            iced_test::Simulator::with_size(iced::Settings::default(), (900., 600.), app.view());
        assert!(ui.find(expected).unwrap().visible_bounds().is_some());
        assert!(ui.find("Previous release").is_err());
        assert!(ui.find("Future release").is_err());
        ui.click(app.i18n.text(Key::UpdateReleasePage)).unwrap();
        ui.tap_key(iced::keyboard::Key::Named(
            iced::keyboard::key::Named::Escape,
        ));
        let mut messages = ui.into_messages();
        assert!(matches!(
            messages.next(),
            Some(Message::OpenUrl(url)) if url == "https://iwe-code.ru/docs/release-notes/2.0.0/"
        ));
        for message in messages {
            let _ = app.update(message);
        }
        assert!(app.whats_new.is_none());
    }
    assert_eq!(
        WhatsNew::from_ledger("3.0.0", ledger)
            .notes(Language::English, std::env::consts::OS)
            .count(),
        0
    );
}

#[test]
fn current_package_contains_release_notes() {
    let current = WhatsNew::current();
    for language in [Language::English, Language::Russian, Language::Spanish] {
        assert!(
            ["macos", "windows", "linux"]
                .into_iter()
                .flat_map(|platform| current.notes(language, platform))
                .any(|note| !note.trim().is_empty())
        );
    }
}

#[test]
fn notes_include_shared_changes_and_only_matching_platforms() {
    let ledger = r#"[{"version":"2.0.0","items":[
        {"text":{"en":"Shared"}},
        {"platforms":["macos"],"text":{"en":"Mac only"}},
        {"platforms":["windows","linux"],"text":{"en":"Windows and Linux"}}
    ]}]"#;
    let release = WhatsNew::from_ledger("2.0.0", ledger);
    for (platform, expected) in [
        ("macos", ["Shared", "Mac only"]),
        ("windows", ["Shared", "Windows and Linux"]),
        ("linux", ["Shared", "Windows and Linux"]),
    ] {
        assert_eq!(
            release
                .notes(Language::English, platform)
                .collect::<Vec<_>>(),
            expected
        );
    }
}
