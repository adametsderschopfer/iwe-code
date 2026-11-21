use super::*;
use crate::{
    app::SettingsTab,
    i18n::{I18n, Key, Language},
    modules::terminal::{Event as TerminalEvent, Status},
};
use std::{path::PathBuf, rc::Rc};

fn app() -> App {
    let mut app = App::new(Rc::new(I18n::new(Language::English)));
    app.workspace.create_space(
        "Work".into(),
        vec![PathBuf::from("first"), PathBuf::from("second")],
    );
    app
}

#[test]
fn module_panels_start_collapsed_and_git_header_preserves_module_state_when_collapsed() {
    for language in [Language::English, Language::Russian, Language::Spanish] {
        let mut app = app();
        app.i18n = Rc::new(I18n::new(language));
        app.sync_panels();
        for module in crate::modules::MODULES {
            assert!(app.module_enabled(module.id));
            assert!(!app.module_open(module.id));
        }
        assert_eq!(app.panes.len(), 1);
        let _ = app.update_module(Event::Toggle(ModuleId::Git));
        app.modules.git.query = "retained search".into();
        let source = app.preview().id;
        let mut ui =
            iced_test::Simulator::with_size(iced::Settings::default(), (1080., 720.), app.view());
        ui.click(iced::widget::Id::new("git-hide")).unwrap();
        let messages = ui.into_messages().collect::<Vec<_>>();
        assert!(messages.iter().any(|message| matches!(
            message,
            Message::Module(Event::Git(crate::modules::git::Event::Hide))
        )));
        for message in messages {
            let _ = app.update(message);
        }
        assert!(!app.module_open(ModuleId::Git));
        assert!(app.module_enabled(ModuleId::Git));
        assert_eq!(app.panes.len(), 1);
        assert_eq!(app.preview().id, source);
        let _ = app.update_module(Event::Toggle(ModuleId::Git));
        assert!(app.module_open(ModuleId::Git));
        assert_eq!(app.modules.git.query, "retained search");
    }
}

#[test]
fn deployment_panel_is_independent_and_preserves_source_width_when_hidden() {
    for language in [Language::English, Language::Russian, Language::Spanish] {
        let mut app = app();
        app.i18n = Rc::new(I18n::new(language));
        let source = app.preview().id;
        for visible in [true, false] {
            let _ = app.update_module(Event::Toggle(ModuleId::Deployment));
            assert_eq!(app.modules.deployment.open, visible);
            assert_eq!(app.preview().id, source);
            assert!(!app.modules.terminal.open);
            let mut ui = iced_test::Simulator::with_size(
                iced::Settings::default(),
                (1440., 920.),
                app.view(),
            );
            let source = ui
                .find(iced::widget::Id::new("source-preview"))
                .unwrap()
                .visible_bounds()
                .unwrap();
            let toggle = ui
                .find(iced::widget::Id::new("deployment-toggle"))
                .unwrap()
                .visible_bounds()
                .unwrap();
            assert!(source.x + source.width <= toggle.x);
            let panel = ui
                .find(iced::widget::Id::new("deployment-panel"))
                .ok()
                .and_then(|p| p.visible_bounds());
            assert_eq!(panel.is_some_and(|p| p.width > 0.), visible);
            if visible {
                assert!(panel.unwrap().x >= source.x + source.width);
            }
        }
    }
}

#[test]
fn disabled_terminal_rejects_events_and_reenable_starts_a_new_session() {
    let mut app = app();
    let _ = app.update_module(Event::Toggle(ModuleId::Terminal));
    let request = app.modules.terminal.active.unwrap();
    let _ = app.update_module(Event::SetEnabled(ModuleId::Terminal, false));
    assert!(app.modules.terminal.tabs.is_empty());
    assert_eq!(
        iced::advanced::subscription::into_recipes(app.module_subscription()).len(),
        0
    );
    let _ = app.update_module(Event::Terminal(TerminalEvent::Start));
    let _ = app.update_module(Event::Toggle(ModuleId::Terminal));
    assert!(app.modules.terminal.tabs.is_empty());
    let _ = app.update_module(Event::SetEnabled(ModuleId::Terminal, true));
    let _ = app.update_module(Event::Toggle(ModuleId::Terminal));
    let fresh = app.modules.terminal.active.unwrap();
    assert!(fresh > request);
    let _ = app.update_module(Event::Terminal(TerminalEvent::Started(request, Err(()))));
    assert_eq!(
        app.modules.terminal.active_tab().unwrap().status,
        Status::Starting
    );
}

#[test]
fn bottom_toolbar_buttons_stay_in_place_when_opening_switching_and_closing_panels() {
    use crate::domain::repository::{Remote, Repository};

    let mut app = app();
    app.repository = Some(
        Repository {
            root: app.workspace.directory().unwrap().clone(),
            branches: Vec::new(),
            commits: Vec::new(),
            next_history: None,
            remote: Remote::parse("https://github.com/example/project.git"),
            remotes: vec!["origin".into()],
            head: "main".into(),
        }
        .into(),
    );
    let buttons = [
        "nav-terminal",
        "nav-kubernetes",
        "nav-docker",
        "nav-git",
        "nav-pipelines",
    ];
    let mut closed_bounds = None;
    for (button, module, opened) in [
        ("nav-terminal", ModuleId::Terminal, true),
        ("nav-terminal", ModuleId::Terminal, false),
        ("nav-kubernetes", ModuleId::Kubernetes, true),
        ("nav-docker", ModuleId::Docker, true),
        ("nav-git", ModuleId::Git, true),
        ("nav-pipelines", ModuleId::Pipelines, true),
        ("nav-pipelines", ModuleId::Pipelines, false),
    ] {
        let mut ui = iced_test::Simulator::with_size(
            iced::Settings {
                default_font: crate::ui::theme::font(),
                ..iced::Settings::default()
            },
            (1080., 820.),
            app.view(),
        );
        let bounds = buttons.map(|id| {
            ui.find(iced::widget::Id::new(id))
                .unwrap()
                .visible_bounds()
                .unwrap()
        });
        assert_eq!(&bounds, closed_bounds.get_or_insert(bounds));
        let terminal = bounds[0];
        for pair in bounds.windows(2) {
            assert_eq!(pair[0].x, pair[1].x);
            assert!(pair[0].y + pair[0].height <= pair[1].y);
        }
        if app.panes.len() > 1 {
            let [files, bottom] = ["file-toolbar-divider", "bottom-toolbar-divider"].map(|id| {
                ui.find(iced::widget::Id::new(id))
                    .unwrap()
                    .visible_bounds()
                    .unwrap()
            });
            assert_eq!((files.x, files.width), (bottom.x, bottom.width));
            let last = bounds[4];
            assert!(
                (bottom.y + bottom.height
                    - last.y
                    - last.height
                    - crate::ui::theme::FILE_PANEL_TOOL_INSET)
                    .abs()
                    <= 1.
            );
        } else {
            let files = ui
                .find(iced::widget::Id::new("file-panel-project"))
                .unwrap()
                .visible_bounds()
                .unwrap();
            assert_eq!(files.x, terminal.x);
            assert!(files.y + files.height <= terminal.y);
        }
        ui.click(iced::widget::Id::new(button)).unwrap();
        let messages = ui.into_messages().collect::<Vec<_>>();
        assert!(messages.iter().any(|message| matches!(
            message,
            Message::Module(Event::Toggle(id)) if *id == module
        )));
        for message in messages {
            let _ = app.update(message);
        }
        assert_eq!(app.module_open(module), opened);
        assert_eq!(
            app.panes
                .iter()
                .any(|(_, panel)| matches!(panel, crate::app::Panel::Module(id) if *id == module)),
            opened
        );
    }
    let mut ui =
        iced_test::Simulator::with_size(iced::Settings::default(), (1080., 820.), app.view());
    let bounds = buttons.map(|id| {
        ui.find(iced::widget::Id::new(id))
            .unwrap()
            .visible_bounds()
            .unwrap()
    });
    assert_eq!(Some(bounds), closed_bounds);
}

#[test]
fn a_short_bottom_panel_keeps_the_last_button_visible_and_scrolls_to_the_first() {
    use iced::{
        mouse,
        widget::{Id, pane_grid},
    };

    let mut app = app();
    let _ = app.update_module(Event::Toggle(ModuleId::Docker));
    let split = *app.panes.layout().splits().next().unwrap();
    let _ = app.update(Message::Resize(pane_grid::ResizeEvent {
        split,
        ratio: 0.85,
    }));
    let mut ui =
        iced_test::Simulator::with_size(iced::Settings::default(), (1080., 540.), app.view());
    let last = ui
        .find(Id::new("nav-git"))
        .unwrap()
        .visible_bounds()
        .unwrap();
    assert_eq!(last.height, crate::ui::theme::FILE_PANEL_TOOL_SIZE);
    assert!(
        ui.find(Id::new("nav-terminal"))
            .unwrap()
            .visible_bounds()
            .is_none()
    );
    ui.point_at(last.center());
    ui.simulate([iced::Event::Mouse(mouse::Event::WheelScrolled {
        delta: mouse::ScrollDelta::Lines { x: 0., y: 12. },
    })]);
    let first = ui
        .click(Id::new("nav-terminal"))
        .unwrap()
        .visible_bounds()
        .unwrap();
    assert_eq!(first.height, crate::ui::theme::FILE_PANEL_TOOL_SIZE);
    assert!(
        ui.into_messages()
            .any(|message| matches!(message, Message::Module(Event::Toggle(ModuleId::Terminal))))
    );
}

#[test]
fn terminal_controls_hide_without_dropping_tabs_and_close_the_last_tab_with_its_panel() {
    fn click(app: &mut App, id: impl Into<iced::widget::Id>) -> Vec<Message> {
        let mut ui = iced_test::Simulator::with_size(
            iced::Settings {
                default_font: crate::ui::theme::font(),
                ..iced::Settings::default()
            },
            (1080., 720.),
            app.view(),
        );
        ui.click(id.into()).unwrap();
        let messages = ui.into_messages().collect::<Vec<_>>();
        for message in messages.iter().cloned() {
            let _ = app.update(message);
        }
        messages
    }
    let mut app = app();
    assert!(app.modules.terminal.tabs.is_empty());
    click(&mut app, "nav-terminal");
    assert_eq!(app.modules.terminal.tabs.len(), 1);
    let first = app.modules.terminal.active.unwrap();
    app.modules.terminal.tabs[0]
        .parser
        .process(b"retained-output");
    click(&mut app, "terminal-add-tab");
    assert_eq!(app.modules.terminal.tabs.len(), 2);
    let second = app.modules.terminal.active.unwrap();
    click(&mut app, "terminal-hide");
    assert!(!app.modules.terminal.open);
    assert_eq!(app.modules.terminal.tabs.len(), 2);
    click(&mut app, "nav-terminal");
    assert!(app.modules.terminal.open);
    assert_eq!(app.modules.terminal.tabs.len(), 2);
    assert_eq!(
        app.modules.terminal.tabs[0].parser.screen().contents(),
        "retained-output"
    );
    let messages = click(
        &mut app,
        iced::widget::Id::from(format!("terminal-close-tab-{first}")),
    );
    assert!(messages.iter().any(|event| matches!(event, Message::Module(Event::Terminal(TerminalEvent::Close(id))) if *id == first)));
    assert!(!messages.iter().any(|event| matches!(
        event,
        Message::Module(Event::Terminal(TerminalEvent::Select(_)))
    )));
    assert_eq!(app.modules.terminal.active, Some(second));
    assert!(app.modules.terminal.open);
    click(
        &mut app,
        iced::widget::Id::from(format!("terminal-close-tab-{second}")),
    );
    assert!(!app.modules.terminal.open);
    assert!(app.modules.terminal.tabs.is_empty());
    assert!(
        !app.panes
            .iter()
            .any(|(_, panel)| matches!(panel, crate::app::Panel::Module(ModuleId::Terminal)))
    );
    click(&mut app, "nav-terminal");
    assert_eq!(app.modules.terminal.tabs.len(), 1);
    assert!(app.modules.terminal.active.unwrap() > second);
}

#[test]
fn project_switch_invalidates_pending_terminal_and_closes_its_panel() {
    let mut app = app();
    let _ = app.load_directory();
    let _ = app.update_module(Event::Toggle(ModuleId::Terminal));
    let request = app.modules.terminal.active_tab().unwrap().request;
    let _ = app.update(Message::SelectProject(0, 1));
    let _ = app.update_module(Event::Terminal(TerminalEvent::Started(request, Err(()))));
    assert!(app.modules.terminal.tabs.is_empty());
    assert!(!app.modules.terminal.open);
}

#[test]
fn same_project_refresh_keeps_terminal_and_its_output() {
    let mut app = app();
    let _ = app.load_directory();
    let _ = app.update_module(Event::Toggle(ModuleId::Terminal));
    app.modules.terminal.tabs[0].status = Status::Exited;
    app.modules.terminal.tabs[0]
        .parser
        .process(b"retained-output");
    let request = app.modules.terminal.active_tab().unwrap().request;
    let _ = app.load_directory();
    assert!(app.modules.terminal.open);
    assert_eq!(app.modules.terminal.active_tab().unwrap().request, request);
    assert_eq!(
        app.modules
            .terminal
            .active_tab()
            .unwrap()
            .parser
            .screen()
            .contents(),
        "retained-output"
    );
}

#[test]
fn module_preferences_survive_roundtrip_and_old_settings_enable_every_module() {
    let mut app = app();
    app.workspace
        .preferences
        .disabled_modules
        .extend(["terminal".into(), "future-module".into()]);
    let json = serde_json::to_string(&app.workspace).unwrap();
    let restored: crate::domain::workspace::Workspace = serde_json::from_str(&json).unwrap();
    assert!(restored.preferences.disabled_modules.contains("terminal"));
    assert!(
        restored
            .preferences
            .disabled_modules
            .contains("future-module")
    );
    app.workspace = restored;
    let _ = app.update_module(Event::Toggle(ModuleId::Terminal));
    assert!(app.modules.terminal.tabs.is_empty());
    let legacy: crate::domain::workspace::Workspace =
        serde_json::from_str(r#"{"spaces":[],"preferences":{"code_size":16}}"#).unwrap();
    assert!(legacy.preferences.disabled_modules.is_empty());
    assert_eq!(legacy.preferences.code_size, 16);
    app.workspace = legacy;
    for module in crate::modules::MODULES {
        assert!(app.module_enabled(module.id));
    }
}

#[test]
fn module_settings_offer_activation_controls_in_all_languages_and_themes() {
    for language in [Language::English, Language::Russian, Language::Spanish] {
        for appearance in [
            crate::domain::workspace::Appearance::Dark,
            crate::domain::workspace::Appearance::Light,
        ] {
            let mut app = app();
            app.i18n = Rc::new(I18n::new(language));
            app.workspace.preferences.appearance = appearance;
            app.settings_tab = SettingsTab::Modules;
            let mut ui = iced_test::Simulator::with_size(
                iced::Settings {
                    default_font: crate::ui::theme::font(),
                    ..iced::Settings::default()
                },
                (780., 610.),
                crate::ui::screens::settings::view(&app),
            );
            for key in [Key::ModulesDescription, Key::SystemModule, Key::Terminal] {
                assert!(
                    ui.find(app.i18n.text(key))
                        .unwrap()
                        .visible_bounds()
                        .is_some()
                );
            }
            let terminal = ui
                .find(iced::widget::Id::new("module-terminal"))
                .unwrap()
                .visible_bounds()
                .unwrap();
            let deployment = ui
                .find(iced::widget::Id::new("module-deployment"))
                .unwrap()
                .visible_bounds()
                .unwrap();
            assert!(terminal.height <= 60.);
            assert!(deployment.height <= 60.);
            assert!((deployment.x - terminal.x).abs() < 1.);
            assert!((deployment.y - terminal.y - terminal.height).abs() < 1.);
            if let Some(output) = std::env::var_os("IWE_RENDER_DIR").map(PathBuf::from) {
                ui.snapshot(&app.theme())
                    .unwrap()
                    .matches_image(output.join(format!("modules-{language:?}-{appearance:?}")))
                    .unwrap();
            }
            assert!(
                ui.find(iced::widget::Id::new("module-terminal-enabled"))
                    .is_ok()
            );
            ui.click(iced::widget::Id::new("module-terminal-enabled"))
                .unwrap();
            assert!(ui.into_messages().any(|message| matches!(
                message,
                Message::Module(Event::SetEnabled(ModuleId::Terminal, false))
            )));
        }
    }
}

#[test]
fn terminal_panel_fits_the_minimum_workspace_in_all_languages_and_themes() {
    for language in [Language::English, Language::Russian, Language::Spanish] {
        for appearance in [
            crate::domain::workspace::Appearance::Dark,
            crate::domain::workspace::Appearance::Light,
        ] {
            let mut app = app();
            app.i18n = Rc::new(I18n::new(language));
            app.workspace.preferences.appearance = appearance;
            let _ = app.update_module(Event::Toggle(ModuleId::Terminal));
            for _ in 0..2 {
                let _ = app.update_module(Event::Terminal(TerminalEvent::Start));
            }
            for tab in &mut app.modules.terminal.tabs {
                tab.status = Status::Exited;
            }
            app.modules.terminal.tabs[2]
                .parser
                .process("\x1b[32mHello · Привет · Hola\x1b[0m\r\nproject> ".as_bytes());
            app.sync_panels();
            let mut ui = iced_test::Simulator::with_size(
                iced::Settings {
                    default_font: crate::ui::theme::font(),
                    ..iced::Settings::default()
                },
                (1080., 720.),
                app.view(),
            );
            let screen = ui
                .find(iced::widget::Id::new("terminal-screen"))
                .unwrap()
                .visible_bounds()
                .unwrap();
            assert!(screen.width > 0. && screen.height > 0.);
            assert!(screen.x >= 0. && screen.y >= 0.);
            assert!(screen.x + screen.width <= 1080. && screen.y + screen.height <= 720.);
            for id in ["terminal-hide", "terminal-add-tab", "terminal-close-tab-1"] {
                let bounds = ui
                    .find(iced::widget::Id::new(id))
                    .unwrap()
                    .visible_bounds()
                    .unwrap();
                assert!(bounds.x + bounds.width <= 1080.);
            }
            for request in 1..=3 {
                let tab = ui
                    .find(iced::widget::Id::from(format!("terminal-tab-{request}")))
                    .unwrap()
                    .visible_bounds()
                    .unwrap();
                let close = ui
                    .find(iced::widget::Id::from(format!(
                        "terminal-close-tab-{request}"
                    )))
                    .unwrap()
                    .visible_bounds()
                    .unwrap();
                assert!(close.x >= tab.x && close.y >= tab.y);
                assert!(close.x + close.width <= tab.x + tab.width);
                assert!(close.y + close.height <= tab.y + tab.height);
            }
            let last = ui
                .find(iced::widget::Id::new("terminal-tab-3"))
                .unwrap()
                .visible_bounds()
                .unwrap();
            let add = ui
                .find(iced::widget::Id::new("terminal-add-tab"))
                .unwrap()
                .visible_bounds()
                .unwrap();
            let gap = add.x - (last.x + last.width);
            assert!(
                (0. ..=10.).contains(&gap),
                "Plus must follow the last terminal tab: {gap}"
            );
            if let Some(output) = std::env::var_os("IWE_RENDER_DIR").map(PathBuf::from) {
                ui.snapshot(&app.theme())
                    .unwrap()
                    .matches_image(output.join(format!("terminal-{language:?}-{appearance:?}")))
                    .unwrap();
            }
        }
    }
}

#[test]
fn every_module_can_be_disabled_without_changing_saved_configuration_or_open_files() {
    let mut app = app();
    let source = app.preview().id;
    let deployment = serde_json::to_value(&app.workspace.deployment).unwrap();
    let spaces = serde_json::to_value(&app.workspace.spaces).unwrap();
    app.workspace.preferences.show_git = true;
    app.workspace.preferences.show_pipelines = true;
    app.workspace
        .preferences
        .disabled_modules
        .insert("future-module".into());
    for module in crate::modules::MODULES {
        assert!(app.module_enabled(module.id));
        let _ = app.update_module(Event::SetEnabled(module.id, false));
        let _ = app.update_module(Event::Toggle(module.id));
        assert!(!app.module_open(module.id));
    }
    assert_eq!(app.preview().id, source);
    assert_eq!(
        serde_json::to_value(&app.workspace.deployment).unwrap(),
        deployment
    );
    assert_eq!(serde_json::to_value(&app.workspace.spaces).unwrap(), spaces);
    assert!(app.workspace.preferences.show_git);
    assert!(app.workspace.preferences.show_pipelines);
    assert_eq!(app.panes.len(), 1);
    for subscription in [
        app.module_subscription(),
        app.pipeline_subscription(),
        app.jobs_subscription(),
        app.change_subscription(),
        app.project_change_subscription(),
    ] {
        assert!(iced::advanced::subscription::into_recipes(subscription).is_empty());
    }
    let mut ui =
        iced_test::Simulator::with_size(iced::Settings::default(), (1280., 800.), app.view());
    for id in [
        "nav-git",
        "nav-pipelines",
        "nav-terminal",
        "deployment-toggle",
        "file-panel-changes",
    ] {
        assert!(ui.find(iced::widget::Id::from(id)).is_err(), "{id}");
    }
    drop(ui);
    let restored = serde_json::from_str(&serde_json::to_string(&app.workspace).unwrap()).unwrap();
    app.workspace = restored;
    for module in crate::modules::MODULES {
        assert!(!app.module_enabled(module.id));
        let _ = app.update_module(Event::SetEnabled(module.id, true));
        assert!(app.module_enabled(module.id));
    }
    assert!(
        app.workspace
            .preferences
            .disabled_modules
            .contains("future-module")
    );
}

#[test]
fn disabled_modules_reject_commands_and_late_results_after_reenable() {
    use crate::{domain::Failure, modules::pipelines::PipelineState};
    let mut app = app();
    let generation = app.generation;
    let pipeline_request = app.modules.pipelines.pipeline_request;
    let _ = app.update_module(Event::SetEnabled(ModuleId::Pipelines, false));
    let _ = app.update(Message::RefreshPipelines);
    assert!(!app.modules.pipelines.pipeline_refreshing);
    let _ = app.update_module(Event::SetEnabled(ModuleId::Pipelines, true));
    let _ = app.update(Message::PipelinesLoaded(
        generation,
        pipeline_request,
        crate::modules::pipelines::transport::Loaded {
            runs: Err(Failure::Access),
            username: Some("obsolete".into()),
        },
    ));
    assert!(matches!(
        app.modules.pipelines.pipelines,
        PipelineState::Idle
    ));
    assert!(app.account_username.is_none());
    let _ = app.update_module(Event::SetEnabled(ModuleId::Git, false));
    let _ = app.update(git_message(crate::modules::git::Event::Search(
        "blocked".into(),
    )));
    let _ = app.update(Message::Git(crate::modules::git::actions::Message::Fetch));
    assert!(app.modules.git.query.is_empty());
    assert!(!app.modules.git.git_actions.pending);
    let _ = app.update_module(Event::SetEnabled(ModuleId::Deployment, false));
    let _ = app.update(deployment_message(
        crate::modules::deployment::Event::Toggle,
    ));
    let _ = app.update(deployment_message(
        crate::modules::deployment::Event::CurrentFile(false),
    ));
    assert!(!app.modules.deployment.open);
    assert!(!app.modules.deployment.busy);
}

#[test]
fn restored_disabled_modules_can_be_enabled_from_the_settings_window() {
    let mut saved = app().workspace;
    saved.preferences.disabled_modules = crate::modules::MODULES
        .iter()
        .map(|module| module.id.key().to_owned())
        .collect();
    let restored = serde_json::from_str(&serde_json::to_string(&saved).unwrap()).unwrap();
    let mut app = App::new(Rc::new(I18n::new(Language::English)));
    let _ = app.update(Message::Loaded(Ok((restored, None))));
    let _ = app.update(Message::Navigate(crate::app::Navigation::Settings));
    let window = app.windows.settings.unwrap();
    let click = |app: &mut App, id: String| {
        let mut ui = iced_test::Simulator::with_size(
            iced::Settings::default(),
            (1120., 760.),
            app.view_window(window),
        );
        ui.click(iced::widget::Id::from(id)).unwrap();
        let messages = ui.into_messages().collect::<Vec<_>>();
        assert!(!messages.is_empty());
        for message in messages {
            let _ = app.update(message);
        }
    };
    click(&mut app, "settings-tab-modules".into());
    assert_eq!(app.settings_tab, SettingsTab::Modules);
    for module in crate::modules::MODULES {
        assert!(!app.module_enabled(module.id));
        click(&mut app, format!("module-{}-enabled", module.id.key()));
        assert!(app.module_enabled(module.id));
    }
    assert!(app.workspace.preferences.disabled_modules.is_empty());
}

#[test]
fn module_panel_controls_emit_typed_events() {
    let mut app = app();
    app.workspace.preferences.show_git = true;
    app.sync_panels();
    let mut ui =
        iced_test::Simulator::with_size(iced::Settings::default(), (1280., 800.), app.view());
    ui.click(iced::widget::Id::new("git-refresh")).unwrap();
    assert!(ui.into_messages().any(|message| matches!(
        message,
        Message::Module(Event::Git(crate::modules::git::Event::Refresh))
    )));
}

#[test]
fn git_processing_appears_only_in_the_shared_footer_even_with_hidden_panel() {
    use iced::widget::Id;
    for language in [Language::English, Language::Russian, Language::Spanish] {
        let mut app = app();
        app.loaded = true;
        app.i18n = Rc::new(I18n::new(language));
        for state in 0..4 {
            app.loading = state == 0;
            app.modules.git.git_actions.pending = state == 1;
            app.modules.git.history_loading = state == 2;
            app.modules.git.changes_loading = state == 3;
            for open in [false, true] {
                if app.module_open(ModuleId::Git) != open {
                    let _ = app.update_module(Event::Toggle(ModuleId::Git));
                }
                let mut ui = iced_test::Simulator::with_size(
                    iced::Settings::default(),
                    (1080., 720.),
                    app.view(),
                );
                let bounds = ui
                    .find(Id::new("git-activity"))
                    .unwrap()
                    .visible_bounds()
                    .unwrap();
                assert!(bounds.y > 650.);
                drop(ui);
                let mut panel = iced_test::Simulator::with_size(
                    iced::Settings::default(),
                    (1080., 400.),
                    crate::ui::screens::git::view(&app),
                );
                for key in [Key::GitWorking, Key::LoadingHistory, Key::Loading] {
                    assert!(panel.find(app.i18n.text(key)).is_err());
                }
            }
        }
        app.loading = false;
        app.modules.git.git_actions.pending = false;
        app.modules.git.history_loading = false;
        app.modules.git.changes_loading = false;
        let mut ui =
            iced_test::Simulator::with_size(iced::Settings::default(), (1080., 720.), app.view());
        assert!(ui.find(Id::new("git-activity")).is_err());
    }
}

#[test]
fn cluster_and_registry_can_be_enabled_independently_and_share_only_the_image() {
    use crate::domain::images::ImageRef;
    let mut app = app();
    let _ = app.update_module(Event::SetEnabled(ModuleId::Docker, false));
    let _ = app.update_module(Event::Toggle(ModuleId::Kubernetes));
    assert!(app.module_open(ModuleId::Kubernetes));
    let image = ImageRef::parse("registry.example/team/web:v2").unwrap();
    let _ = app.update_module(Event::Kubernetes(kubernetes::Event::OpenRegistry(
        image.clone(),
    )));
    assert!(!app.module_open(ModuleId::Docker));
    let _ = app.update_module(Event::SetEnabled(ModuleId::Docker, true));
    let _ = app.update_module(Event::Kubernetes(kubernetes::Event::OpenRegistry(image)));
    assert!(app.module_open(ModuleId::Docker));
    assert!(app.modules.docker.registry_open);
    let _ = app.update_module(Event::SetEnabled(ModuleId::Kubernetes, false));
    assert!(app.module_open(ModuleId::Docker));
    let _ = app.update_module(Event::Toggle(ModuleId::Kubernetes));
    assert!(!app.module_open(ModuleId::Kubernetes));
}

/// Run only against a dedicated loopback registry and disposable cluster.
/// The fixture supplies an existing Deployment named sample in iwe-acceptance.
#[tokio::test]
#[ignore = "requires a disposable registry, image and Kubernetes cluster"]
async fn publish_choose_roll_out_and_reopen_the_runtime_manifest() {
    use crate::{
        domain::images::Registry,
        modules::{docker::Event as D, kubernetes::Event as K},
    };
    use iced::futures::StreamExt;
    use iced_test::runtime::{Action, task};
    use registry::Event as R;
    async fn send(app: &mut App, event: Event) -> Vec<Message> {
        let mut queue = std::collections::VecDeque::from([app.update_module(event)]);
        let mut replies = Vec::new();
        while let Some(task) = queue.pop_front() {
            if let Some(mut actions) = task::into_stream(task) {
                while let Some(action) = actions.next().await {
                    if let Action::Output(m) = action {
                        replies.push(m.clone());
                        queue.push_back(app.update(m));
                    }
                }
            }
        }
        replies
    }
    fn click(app: &App, id: ModuleId, label: &'static str, by_id: bool) -> Event {
        let mut ui = iced_test::Simulator::with_size(
            iced::Settings::default(),
            (1300., 850.),
            crate::ui::modules::panel(app, id),
        );
        if by_id {
            ui.click(iced::widget::Id::new(label)).unwrap();
        } else {
            ui.click(label.to_owned()).unwrap();
        }
        ui.into_messages()
            .find_map(|m| match m {
                Message::Module(e) => Some(e),
                _ => None,
            })
            .expect("enabled action")
    }
    let endpoint = std::env::var("IWE_TEST_REGISTRY").unwrap();
    let source = std::env::var("IWE_TEST_IMAGE").unwrap();
    assert!(endpoint.starts_with("http://localhost:"));
    assert!(source.starts_with("iwe-services-"));
    let cfg = std::path::PathBuf::from(std::env::var_os("KUBECONFIG").unwrap());
    assert_eq!(cfg.file_name().unwrap(), "test-kubeconfig");
    assert!(cfg.is_absolute());
    let context = std::env::var("DOCKER_CONTEXT").unwrap();
    assert!(!context.is_empty());
    let reg = Registry::new("Loopback", &endpoint, false).unwrap();
    let tmp = tempfile::tempdir().unwrap();
    let mut app = App::new(Rc::new(I18n::new(Language::English)));
    app.save_blocked = true;
    app.workspace
        .create_space("Live".into(), vec![tmp.path().to_owned()]);
    app.workspace.preferences.registries = vec![reg.clone()];
    send(&mut app, Event::Toggle(ModuleId::Docker)).await;
    send(&mut app, Event::Docker(D::Context(context))).await;
    let e = click(&app, ModuleId::Docker, "docker-kind-images", true);
    send(&mut app, e).await;
    send(&mut app, Event::Docker(D::Select(source))).await;
    let e = click(&app, ModuleId::Docker, "Publish", false);
    send(&mut app, e).await;
    for e in [
        R::RepoInput("iwe/demo".into()),
        R::VersionInput("candidate".into()),
        R::Review,
    ] {
        send(&mut app, Event::Docker(D::Registry(e))).await;
    }
    let e = click(&app, ModuleId::Docker, "Publish", false);
    let published = send(&mut app, e).await;
    assert!(published.iter().any(|m| matches!(
        m,
        Message::Module(Event::Docker(D::Registry(R::Transferred(_, Ok(())))))
    )));
    assert!(published.iter().any(|m|matches!(m,Message::Module(Event::Docker(D::Registry(R::Listed(_,Ok(p))))) if p.names.iter().any(|n|n=="candidate"))));
    send(&mut app, Event::Toggle(ModuleId::Kubernetes)).await;
    let rows = send(
        &mut app,
        Event::Kubernetes(K::Namespace("iwe-acceptance".into())),
    )
    .await;
    let resource = rows
        .into_iter()
        .find_map(|m| match m {
            Message::Module(Event::Kubernetes(K::Listed(_, Ok(rows)))) => rows
                .into_iter()
                .find(|r| r.name == "sample" && r.ns == "iwe-acceptance"),
            _ => None,
        })
        .unwrap();
    send(&mut app, Event::Kubernetes(K::Select(resource))).await;
    send(&mut app, Event::Kubernetes(K::Container("web".into()))).await;
    let e = click(&app, ModuleId::Kubernetes, "Change image", false);
    send(&mut app, e).await;
    let e = click(
        &app,
        ModuleId::Kubernetes,
        "Choose from Docker registry",
        false,
    );
    send(&mut app, e).await;
    assert!(app.modules.docker.registry.picking());
    let results = send(
        &mut app,
        Event::Docker(D::Registry(R::Version("candidate".into()))),
    )
    .await;
    let pin = results
        .into_iter()
        .find_map(|m| match m {
            Message::Module(Event::Docker(D::Registry(R::Inspected(_, Ok(m))))) => Some(m.digest),
            _ => None,
        })
        .unwrap();
    let e = click(&app, ModuleId::Docker, "Use this digest", false);
    send(&mut app, e).await;
    assert!(app.module_open(ModuleId::Kubernetes));
    let e = click(&app, ModuleId::Kubernetes, "Review", false);
    send(&mut app, e).await;
    let e = click(&app, ModuleId::Kubernetes, "Confirm", false);
    let rollout = send(&mut app, e).await;
    assert!(rollout.iter().any(|m|matches!(m,Message::Module(Event::Kubernetes(K::Status(_,Ok(s)))) if s.contains("successfully rolled out"))));
    let e = click(&app, ModuleId::Kubernetes, "Related Pods", false);
    let pods = send(&mut app, e).await;
    let pods = pods
        .into_iter()
        .find_map(|m| match m {
            Message::Module(Event::Kubernetes(K::Listed(_, Ok(rows)))) => Some(rows),
            _ => None,
        })
        .unwrap();
    let mut runtime = None;
    for pod in pods.into_iter().filter(|p| p.status == "Running") {
        let details = send(&mut app, Event::Kubernetes(K::Select(pod))).await;
        let Some(container) = details.into_iter().find_map(|m| match m {
            Message::Module(Event::Kubernetes(K::Details(_, Ok(d)))) => d
                .containers
                .iter()
                .find(|c| c.name == "web" && c.image.ends_with(&pin))
                .cloned(),
            _ => None,
        }) else {
            continue;
        };
        runtime = Some(
            container
                .running
                .or_else(|| crate::domain::images::ImageRef::parse(&container.image))
                .unwrap(),
        );
        break;
    }
    let runtime = runtime.expect("updated running Pod");
    let e = click(&app, ModuleId::Kubernetes, "Open image in registry", false);
    assert!(matches!(&e,Event::Kubernetes(K::OpenRegistry(i)) if i==&runtime));
    let opened = send(&mut app, e).await;
    assert!(opened.iter().any(|m|matches!(m,Message::Module(Event::Docker(D::Registry(R::Inspected(_,Ok(m))))) if m.digest==runtime.version)));
    println!("Published {pin}; reopened {}", runtime.version);
}
