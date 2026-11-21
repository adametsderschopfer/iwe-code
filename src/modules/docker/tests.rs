use super::*;

#[test]
fn inventories_keep_image_tags_and_support_compose_arrays() {
    let rows = adapter::parse(
        Kind::Images,
        r#"{"ID":"abc","Repository":"org/image","Tag":"latest","Size":"5 MB"}"#,
    )
    .unwrap();
    assert_eq!(rows[0].id, "org/image:latest");
    assert_eq!(
        model::arguments(Kind::Images, Action::Remove, Some(&rows[0]), "", "").unwrap(),
        ["image", "rm", "--", "org/image:latest"]
    );
    assert!(adapter::parse(Kind::Compose, r#"[{"Name":"stack","Status":"running(2)"}]"#).is_ok());
    assert!(adapter::parse(Kind::Networks, "invalid").is_err());
}

#[test]
fn operations_validate_arguments_and_require_explicit_confirmation() {
    assert!(model::arguments(Kind::Containers, Action::Create, None, "--privileged", "").is_none());
    assert!(model::arguments(Kind::Compose, Action::Up, None, "a file.yaml", "").is_some());
    assert!(
        model::arguments(
            Kind::Networks,
            Action::AddContext,
            None,
            "remote",
            "ssh://user:secret@host"
        )
        .is_none()
    );
    let items = adapter::parse(
        Kind::Containers,
        r#"{"ID":"abc","Names":"api","State":"running","Status":"Up"}"#,
    )
    .unwrap();
    assert!(Action::actions(Kind::Containers, &items[0], true).contains(&Action::Remove));
    assert!(!Action::actions(Kind::Containers, &items[0], false).contains(&Action::Terminal));
    let mut state = State {
        items: Arc::new(items),
        context: Some("remote".into()),
        selected: Some("abc".into()),
        ..Default::default()
    };
    let _ = state.update(Event::Act(Action::Stop), Path::new("."), true);
    assert!(state.busy.is_none());
    assert_eq!(state.draft.as_ref().unwrap().action, Action::Stop);
    let _ = state.update(Event::Dismiss, Path::new("."), true);
    assert!(state.draft.is_none());
}

#[test]
fn container_terminal_uses_its_name_for_the_tab_and_its_id_for_execution() {
    let rows = adapter::parse(
        Kind::Containers,
        r#"{"ID":"abc","Names":"worker","State":"running","Status":"Up"}"#,
    )
    .unwrap();
    let mut state = State {
        open: true,
        items: Arc::new(rows),
        context: Some("local".into()),
        selected: Some("abc".into()),
        ..Default::default()
    };
    let _ = state.update(Event::Act(Action::Terminal), Path::new("."), true);
    let request = state.request;
    assert!(state.take_shell(request + 1).is_none());
    let (title, args) = state.take_shell(request).unwrap();
    assert_eq!(title, "worker");
    assert_eq!(
        args,
        [
            "--context",
            "local",
            "exec",
            "--interactive",
            "--tty",
            "--",
            "abc",
            "sh"
        ]
    );
    assert!(state.take_shell(request).is_none());
}

#[test]
fn obsolete_completions_cannot_repopulate_a_reset_panel() {
    let mut state = State::default();
    let _ = state.connections(Path::new("."));
    let request = state.request;
    state.reset();
    let _ = state.update(
        Event::Connected(request, Ok((vec!["remote".into()], "remote".into()))),
        Path::new("."),
        true,
    );
    assert!(state.context.is_none());
    assert!(state.busy.is_none());
}

/// Run against an explicitly selected test daemon and its disposable fixture image.
#[tokio::test]
#[ignore = "requires IWE_DOCKER_TEST_CONTEXT and a disposable image with a long-running default command"]
async fn live_resource_lifecycle() {
    let context = std::env::var("IWE_DOCKER_TEST_CONTEXT").unwrap();
    let image = std::env::var("IWE_DOCKER_TEST_IMAGE").unwrap();
    let name = std::env::var("IWE_DOCKER_TEST_NAME").unwrap();
    assert!(name.starts_with("iwe-test-") && model::operand(&name));
    let root = tempfile::tempdir().unwrap();
    let root = root.path();
    async fn invoke(
        root: &Path,
        context: &str,
        kind: Kind,
        action: Action,
        item: Option<&Item>,
        value: &str,
        extra: &str,
    ) -> String {
        let args = model::arguments(kind, action, item, value, extra).unwrap();
        adapter::run(
            Some(context),
            &args,
            root,
            action.writes(),
            action == Action::Logs,
        )
        .await
        .unwrap_or_else(|error| panic!("{kind:?} {action:?}: {error:?}"))
    }
    async fn resource(root: &Path, context: &str, kind: Kind, name: &str) -> Item {
        let list = adapter::run(Some(context), &kind.list(), root, false, false)
            .await
            .unwrap();
        adapter::parse(kind, &list)
            .unwrap()
            .into_iter()
            .find(|item| item.name == name)
            .unwrap()
    }
    let contexts = adapter::contexts(root).await.unwrap();
    assert!(contexts.0.contains(&context));
    let fixture = resource(root, &context, Kind::Images, &image).await;
    let tag = format!("{name}:test");
    invoke(
        root,
        &context,
        Kind::Images,
        Action::Tag,
        Some(&fixture),
        &tag,
        "",
    )
    .await;
    let mut options = container::Options::default();
    options.set(container::Field::Environment, "IWE_TEST_MODE=local".into());
    options.set(container::Field::Ports, "127.0.0.1::8080".into());
    let args = options.arguments(&tag, &name).unwrap();
    adapter::run(Some(&context), &args, root, true, false)
        .await
        .unwrap();
    let container = resource(root, &context, Kind::Containers, &name).await;
    invoke(
        root,
        &context,
        Kind::Containers,
        Action::Start,
        Some(&container),
        "",
        "",
    )
    .await;
    let (images, image_key) = relations::open_image(&context, root, &container)
        .await
        .unwrap();
    assert_eq!(image_key, tag);
    let digest = &images.iter().find(|row| row.id == tag).unwrap().detail;
    assert!(
        relations::running(&context, root, digest)
            .await
            .unwrap()
            .iter()
            .any(|row| row.id == container.id && row.name == name)
    );
    for kind in [Kind::Networks, Kind::Volumes] {
        invoke(root, &context, kind, Action::Create, None, &name, "").await;
        let item = resource(root, &context, kind, &name).await;
        assert!(
            invoke(root, &context, kind, Action::Inspect, Some(&item), "", "")
                .await
                .contains(&name)
        );
        if kind == Kind::Networks {
            invoke(
                root,
                &context,
                kind,
                Action::Connect,
                Some(&item),
                &container.id,
                "",
            )
            .await;
            assert!(
                invoke(root, &context, kind, Action::Inspect, Some(&item), "", "")
                    .await
                    .contains(&container.id)
            );
            invoke(
                root,
                &context,
                kind,
                Action::Disconnect,
                Some(&item),
                &container.id,
                "",
            )
            .await;
        }
        invoke(root, &context, kind, Action::Remove, Some(&item), "", "").await;
    }
    for action in [
        Action::Logs,
        Action::Inspect,
        Action::Pause,
        Action::Resume,
        Action::Restart,
        Action::Stop,
        Action::Remove,
    ] {
        let output = invoke(
            root,
            &context,
            Kind::Containers,
            action,
            Some(&container),
            "",
            "",
        )
        .await;
        if action == Action::Inspect {
            assert!(output.contains("IWE_TEST_MODE=local") && output.contains("8080/tcp"));
        }
        if action == Action::Logs {
            assert!(!output.is_empty());
        }
    }
    let dockerfile = root.join("Dockerfile");
    let build_tag = format!("{name}:built");
    std::fs::write(&dockerfile, "FROM scratch\nLABEL iwe.fixture=true\n").unwrap();
    invoke(
        root,
        &context,
        Kind::Images,
        Action::Build,
        None,
        dockerfile.to_str().unwrap(),
        &build_tag,
    )
    .await;
    let built = resource(root, &context, Kind::Images, &build_tag).await;
    invoke(
        root,
        &context,
        Kind::Images,
        Action::Remove,
        Some(&built),
        "",
        "",
    )
    .await;
    let compose = root.join("fixture.yaml");
    std::fs::write(
        &compose,
        format!(
            "name: {name}\nservices:\n  probe:\n    image: {tag}\n    container_name: {name}\n"
        ),
    )
    .unwrap();
    invoke(
        root,
        &context,
        Kind::Compose,
        Action::Up,
        None,
        compose.to_str().unwrap(),
        "",
    )
    .await;
    let stack = resource(root, &context, Kind::Compose, &name).await;
    let member = resource(root, &context, Kind::Containers, &name).await;
    let network = resource(root, &context, Kind::Networks, &format!("{name}_default")).await;
    let graph = graph::load(&context, root, &stack.name, Arc::default())
        .await
        .unwrap();
    let member = graph::Id(Kind::Containers, member.id);
    for (source, target) in [
        (graph::Id(Kind::Compose, stack.name.clone()), member.clone()),
        (graph::Id(Kind::Images, digest.clone()), member.clone()),
        (member, graph::Id(Kind::Networks, network.id)),
    ] {
        assert!(
            graph
                .edges
                .iter()
                .any(|&(a, b)| graph.nodes[a].id == source && graph.nodes[b].id == target)
        );
    }
    for action in [
        Action::Inspect,
        Action::Logs,
        Action::Stop,
        Action::Start,
        Action::Restart,
        Action::Remove,
    ] {
        invoke(root, &context, Kind::Compose, action, Some(&stack), "", "").await;
    }
    let tagged = resource(root, &context, Kind::Images, &tag).await;
    invoke(
        root,
        &context,
        Kind::Images,
        Action::Remove,
        Some(&tagged),
        "",
        "",
    )
    .await;
}

#[test]
fn log_dialog_keeps_the_context_target_and_rejects_late_reads() {
    use crate::i18n::{I18n, Language};
    use iced::widget::{Id, space};
    let root = Path::new(".");
    let items = adapter::parse(
        Kind::Containers,
        r#"[{"ID":"a","Names":"first"},{"ID":"b","Names":"second"}]"#,
    )
    .unwrap();
    let mut state = State {
        open: true,
        context: Some("local".into()),
        items: Arc::new(items),
        selected: Some("a".into()),
        query: "s".into(),
        ..Default::default()
    };
    state.filter();
    let i18n = I18n::new(Language::Russian);
    let _ = state.update(Event::Menu("b".into(), Point::new(10., 30.)), root, true);
    let mut menu = iced_test::Simulator::with_size(
        iced::Settings::default(),
        (720., 500.),
        overlay(
            &state,
            &i18n,
            true,
            space()
                .width(iced::Length::Fill)
                .height(iced::Length::Fill)
                .into(),
            |e| e,
        ),
    );
    menu.click(i18n.text(Key::DockerLogs)).unwrap();
    for e in menu.into_messages() {
        let _ = state.update(e, root, true);
    }
    assert!(state.has_dialog());
    let dialog = state.output.as_ref().unwrap();
    assert_eq!(dialog.request.name, "second");
    assert_eq!(dialog.request.action, Action::Logs);
    assert!(matches!(dialog.content, output::Content::Loading { .. }));
    let id = state.request;
    let _ = state.update(Event::Tick, root, true);
    let _ = state.update(Event::Refresh, root, true);
    assert_eq!(state.request, id);
    let error = Error::Stopped;
    let _ = state.update(Event::OutputLoaded(id, Err(error)), root, true);
    let mut window = iced_test::Simulator::with_size(
        iced::Settings::default(),
        (640., 430.),
        overlay(
            &state,
            &i18n,
            true,
            space()
                .width(iced::Length::Fill)
                .height(iced::Length::Fill)
                .into(),
            |e| e,
        ),
    );
    assert!(window.find(i18n.text(view::error_key(error))).is_ok());
    window.click(Id::new("docker-output-refresh")).unwrap();
    for event in window.into_messages() {
        let _ = state.update(event, root, true);
    }
    assert!(state.request > id);
    assert_eq!(state.output.as_ref().unwrap().request.name, "second");
    let retry = state.request;
    let _ = state.update(Event::Dismiss, root, true);
    assert!(!state.has_dialog());
    let _ = state.update(Event::Act(Action::Inspect), root, true);
    let _ = state.update(
        Event::OutputLoaded(retry, Ok(Arc::new(Output::new("obsolete".into())))),
        root,
        true,
    );
    assert_eq!(state.output.as_ref().unwrap().request.name, "first");
    assert!(matches!(
        state.output.as_ref().unwrap().content,
        output::Content::Loading { .. }
    ));
    let _ = state.update(Event::Search("background".into()), root, true);
    let _ = state.update(Event::Dismiss, root, true);
    assert_eq!(state.query, "s");
    assert_eq!(state.selected.as_deref(), Some("a"));
    assert!(state.connection_error.is_none() && state.error.is_none() && state.busy.is_none());
}

#[test]
fn properties_open_localized_scrollable_output_with_bounded_pages() {
    use crate::i18n::{I18n, Language};
    use iced::widget::{Id, space};
    let root = Path::new(".");
    for language in [Language::English, Language::Russian, Language::Spanish] {
        let i18n = I18n::new(language);
        let items = adapter::parse(Kind::Compose, r#"[{"Name":"demo"}]"#).unwrap();
        let mut state = State {
            open: true,
            kind: Kind::Compose,
            context: Some("local".into()),
            items: Arc::new(items),
            selected: Some("demo".into()),
            ..Default::default()
        };
        for action in [Action::Inspect, Action::Logs] {
            let mut properties = iced_test::Simulator::with_size(
                iced::Settings::default(),
                (800., 480.),
                panel(&state, &i18n, true),
            );
            properties.click(i18n.text(action.key())).unwrap();
            for e in properties.into_messages() {
                let _ = state.update(e, root, true);
            }
            let id = state.request;
            let _ = state.update(
                Event::OutputLoaded(id, Ok(Arc::new(Output::new(String::new())))),
                root,
                true,
            );
            let mut window = iced_test::Simulator::with_size(
                iced::Settings::default(),
                (410., 280.),
                overlay(
                    &state,
                    &i18n,
                    true,
                    space()
                        .width(iced::Length::Fill)
                        .height(iced::Length::Fill)
                        .into(),
                    |e| e,
                ),
            );
            let empty = if action == Action::Logs {
                Key::DockerNoLogs
            } else {
                Key::DockerNoDetails
            };
            assert!(
                window
                    .find(i18n.text(empty))
                    .unwrap()
                    .visible_bounds()
                    .is_some()
            );
            window.click(Id::new("docker-output-refresh")).unwrap();
            for e in window.into_messages() {
                let _ = state.update(e, root, true);
            }
            let text = "entry\n".repeat(100) + "last line";
            let _ = state.update(
                Event::OutputLoaded(state.request, Ok(Arc::new(Output::new(text)))),
                root,
                true,
            );
            for _ in 0..2 {
                let _ = state.update(Event::OutputPage(true), root, true);
            }
            let mut window = iced_test::Simulator::with_size(
                iced::Settings::default(),
                (410., 280.),
                overlay(
                    &state,
                    &i18n,
                    true,
                    space()
                        .width(iced::Length::Fill)
                        .height(iced::Length::Fill)
                        .into(),
                    |e| e,
                ),
            );
            assert!(window.find("2 / 2").unwrap().visible_bounds().is_some());
            assert!(window.find("last line").unwrap().visible_bounds().is_some());
            window.click(Id::new("docker-output-close")).unwrap();
            for e in window.into_messages() {
                let _ = state.update(e, root, true);
            }
            assert!(!state.has_dialog());
            assert_eq!(state.selected.as_deref(), Some("demo"));
        }
    }
}

#[test]
fn large_output_pages_preserve_unicode_and_every_line() {
    let value = "ав".repeat(20_000) + "\n" + &"message\n".repeat(207);
    let out = Output::new(value.clone());
    assert!(out.offsets.len() > 3);
    assert_eq!(
        (0..out.offsets.len())
            .map(|i| out.page(i).unwrap())
            .collect::<String>(),
        value
    );
}

#[test]
fn populated_panels_and_confirmation_forms_fit_all_languages() {
    use crate::i18n::{I18n, Language};
    for language in [Language::English, Language::Russian, Language::Spanish] {
        let i18n = I18n::new(language);
        let items = adapter::parse(
            Kind::Containers,
            r#"{"ID":"abc","Names":"api","Image":"example","State":"running","Status":"Up"}"#,
        )
        .unwrap();
        let mut state = State {
            open: true,
            items: Arc::new(items),
            context: Some("local".into()),
            contexts: vec!["local".into()],
            ..Default::default()
        };
        state.filter();
        let _ = state.update(Event::Select("abc".into()), Path::new("."), true);
        let mut ui = iced_test::Simulator::with_size(
            iced::Settings::default(),
            (800., 340.),
            panel(&state, &i18n, true),
        );
        ui.click(i18n.text(Key::DockerStop)).unwrap();
        for event in ui.into_messages() {
            let _ = state.update(event, Path::new("."), true);
        }
        assert!(state.busy.is_none());
        let mut ui = iced_test::Simulator::with_size(
            iced::Settings::default(),
            (1080., 720.),
            overlay(&state, &i18n, true, panel(&state, &i18n, true), |event| {
                event
            }),
        );
        assert!(
            ui.find(i18n.text(Key::DockerConfirm))
                .unwrap()
                .visible_bounds()
                .is_some()
        );
        ui.click(i18n.text(Key::Cancel)).unwrap();
        for event in ui.into_messages() {
            let _ = state.update(event, Path::new("."), true);
        }
        assert!(state.draft.is_none());
    }
}

#[test]
fn create_container_preserves_option_boundaries_and_quoted_arguments() {
    use container::{Field, Options};
    let mut options = Options::default();
    options.set(Field::Ports, "127.0.0.1:8080:80 9000:9000".into());
    options.set(
        Field::Mounts,
        "'type=bind,src=/local files,dst=/data,readonly'".into(),
    );
    options.set(Field::Environment, "'MODE=two words'".into());
    options.set(Field::Command, "echo 'two words' --flag".into());
    assert_eq!(
        options.arguments("sample:local", "test").unwrap(),
        [
            "container",
            "create",
            "--name",
            "test",
            "--publish",
            "127.0.0.1:8080:80",
            "--publish",
            "9000:9000",
            "--mount",
            "type=bind,src=/local files,dst=/data,readonly",
            "--env",
            "MODE=two words",
            "--",
            "sample:local",
            "echo",
            "two words",
            "--flag"
        ]
    );
    options.set(Field::Ports, "'8080".into());
    assert!(options.arguments("sample:local", "test").is_none());
}

#[test]
fn resource_sections_reflow_and_keep_the_filter_when_returning_to_a_narrow_list() {
    use crate::i18n::{I18n, Language};
    use iced::widget::Id;
    for language in [Language::English, Language::Russian, Language::Spanish] {
        let i18n = I18n::new(language);
        let items = adapter::parse(
            Kind::Containers,
            r#"{"ID":"abc","Names":"api","Image":"sample","State":"running","Status":"Up"}"#,
        )
        .unwrap();
        let mut state = State {
            open: true,
            items: Arc::new(items),
            query: "ap".into(),
            selected: Some("abc".into()),
            ..Default::default()
        };
        state.filter();
        let mut wide = iced_test::Simulator::with_size(
            iced::Settings::default(),
            (850., 280.),
            panel(&state, &i18n, true),
        );
        let [nav, list, details] = ["docker-navigation", "docker-inventory", "docker-properties"]
            .map(|name| wide.find(Id::new(name)).unwrap().visible_bounds().unwrap());
        assert_eq!(nav.x, 0.);
        assert_eq!(nav.x + nav.width + 1., list.x);
        assert_eq!(list.x + list.width + 1., details.x);
        assert_eq!(details.x + details.width, 850.);
        assert_eq!(nav.y + nav.height, 280.);
        assert_eq!(nav.y, details.y);
        let filter = wide
            .find(Id::new("docker-filter"))
            .unwrap()
            .visible_bounds()
            .unwrap();
        assert!((filter.center_y() - list.y - 17.).abs() < 1.);
        assert_eq!(nav.width, 42.);
        assert!((details.width - (850. - 44.) * 0.3).abs() < 1.);
        assert!(wide.find(i18n.text(Key::DockerActions)).is_ok());
        drop(wide);
        let mut narrow = iced_test::Simulator::with_size(
            iced::Settings::default(),
            (390., 250.),
            panel(&state, &i18n, true),
        );
        let narrow_nav = narrow
            .find(Id::new("docker-navigation"))
            .unwrap()
            .visible_bounds()
            .unwrap();
        assert_eq!(narrow_nav.width, nav.width);
        assert!(narrow.find(Id::new("docker-inventory")).is_err());
        narrow.click(Id::new("docker-back")).unwrap();
        for event in narrow.into_messages() {
            let _ = state.update(event, Path::new("."), true);
        }
        assert!(state.selected.is_none());
        assert_eq!(state.query, "ap");
        let mut narrow = iced_test::Simulator::with_size(
            iced::Settings::default(),
            (390., 250.),
            panel(&state, &i18n, true),
        );
        assert!(narrow.find(Id::new("docker-properties")).is_err());
        assert!(
            narrow
                .find(Id::new("docker-filter"))
                .unwrap()
                .visible_bounds()
                .is_some()
        );
        narrow.click("api").unwrap();
        assert!(
            narrow
                .into_messages()
                .any(|event| matches!(event, Event::Select(id) if id == "abc"))
        );
    }
}

#[test]
fn docker_navigation_shows_only_icons_and_selects_each_resource() {
    use crate::i18n::{I18n, Language};
    use iced::widget::Id;
    let i18n = I18n::new(Language::English);
    let state = State::default();
    for width in [390., 850.] {
        let mut ui = iced_test::Simulator::with_size(
            iced::Settings::default(),
            (width, 300.),
            panel(&state, &i18n, true),
        );
        let nav = ui
            .find(Id::new("docker-navigation"))
            .unwrap()
            .visible_bounds()
            .unwrap();
        for (index, kind) in Kind::ALL.into_iter().enumerate() {
            if kind != state.kind {
                assert!(ui.find(i18n.text(kind.key())).is_err());
            }
            // Start at the center of the first button and advance one row per resource.
            ui.point_at(iced::Point::new(
                nav.center_x(),
                nav.y + 19. + index as f32 * 32.,
            ));
            let _ = ui.simulate(iced_test::simulator::click());
        }
        assert_eq!(
            ui.into_messages()
                .filter_map(|event| match event {
                    Event::Kind(kind) => Some(kind),
                    _ => None,
                })
                .collect::<Vec<_>>(),
            Kind::ALL,
        );
    }
}

#[test]
fn container_form_keeps_its_actions_reachable_below_scrollable_settings() {
    use crate::i18n::{I18n, Language};
    use iced::widget::{Id, space};
    for language in [Language::English, Language::Russian, Language::Spanish] {
        let i18n = I18n::new(language);
        let mut state = State {
            open: true,
            context: Some("local".into()),
            ..Default::default()
        };
        let _ = state.update(Event::Act(Action::Create), Path::new("."), true);
        let _ = state.update(Event::Value("sample:latest".into()), Path::new("."), true);
        for (width, height) in [(420., 320.), (940., 760.)] {
            let mut ui = iced_test::Simulator::with_size(
                iced::Settings::default(),
                (width, height),
                overlay(
                    &state,
                    &i18n,
                    true,
                    space()
                        .width(iced::Length::Fill)
                        .height(iced::Length::Fill)
                        .into(),
                    |e| e,
                ),
            );
            let submit = ui
                .find(Id::new("docker-submit"))
                .unwrap()
                .visible_bounds()
                .unwrap();
            assert!(submit.height >= 22. && submit.y + submit.height <= height - 24.);
            if height > 700. {
                assert!(
                    ui.find(i18n.text(Key::DockerRuntime))
                        .unwrap()
                        .visible_bounds()
                        .is_some()
                );
                assert!(
                    ui.find(i18n.text(Key::DockerImage))
                        .unwrap()
                        .visible_bounds()
                        .is_some()
                );
            }
            ui.click(Id::new("docker-submit")).unwrap();
            assert!(
                ui.into_messages()
                    .any(|event| matches!(event, Event::Submit))
            );
        }
    }
}

#[test]
fn section_dividers_meet_the_header_and_filter_corners_match_the_ui_kit() {
    use crate::i18n::{I18n, Language};
    use iced::widget::Id;

    let state = State::default();
    let i18n = I18n::new(Language::English);
    for theme in [iced::Theme::Dark, iced::Theme::Light] {
        let mut ui = iced_test::Simulator::with_size(
            iced::Settings::default(),
            (880., 320.),
            panel(&state, &i18n, true),
        );
        let directory = tempfile::tempdir().unwrap();
        ui.snapshot(&theme)
            .unwrap()
            .matches_image(directory.path().join("panel"))
            .unwrap();
        let png = std::fs::read_dir(directory.path())
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        let image = image::open(png).unwrap().into_rgba8();
        let scale = image.width() as f32 / 880.;
        let pixel = |x: f32, y: f32| *image.get_pixel((x * scale) as u32, (y * scale) as u32);
        let nav = ui
            .find(Id::new("docker-navigation"))
            .unwrap()
            .visible_bounds()
            .unwrap();
        let x = nav.x + nav.width;
        let line = pixel(x, nav.y + 1.);
        assert_eq!(line, pixel(x, nav.y + nav.height - 1.));
        assert_eq!(line, pixel(x + 2., nav.y - 1.));
        assert_ne!(line, pixel(x - 1., nav.y + 1.));
        assert_ne!(line, pixel(x + 1., nav.y + 1.));
        let id = "docker-filter";
        {
            let area = ui.find(Id::new(id)).unwrap().visible_bounds().unwrap();
            for y in [area.y + 0.5, area.y + area.height - 0.5] {
                let edge = pixel(area.center_x(), y);
                assert_ne!(edge, pixel(area.x - 2., y), "{id} has a visible border");
                let outside = pixel(area.x - 2., y);
                let radius = crate::ui::theme::CONTROL_RADIUS;
                for offset in [0.5, area.width - 0.5] {
                    assert_eq!(pixel(area.x + offset, y), outside, "{id} rounded corner");
                }
                for offset in [radius + 1., area.width - radius - 1.] {
                    assert_eq!(pixel(area.x + offset, y), edge, "{id} straight edge");
                }
            }
        }
    }
}

#[test]
fn daemon_errors_distinguish_local_sockets_remote_connections_and_permissions() {
    for (message, expected) in [
        (
            "cannot connect to the docker daemon at unix:///var/run/docker.sock. is the docker daemon running?",
            Error::Stopped,
        ),
        (
            "failed to connect to the docker api at unix:///run/docker.sock: dial unix /run/docker.sock: no such file or directory",
            Error::Stopped,
        ),
        (
            "dial unix /home/user/.colima/default/docker.sock: connect: connection refused",
            Error::Stopped,
        ),
        (
            "error during connect: open //./pipe/dockerdesktoplinuxengine: the system cannot find the file specified",
            Error::Stopped,
        ),
        (
            "failed to connect to the docker api at ssh://server: dial unix /run/docker.sock: no such file or directory",
            Error::Unavailable,
        ),
        (
            "error during connect: tcp 10.0.0.1:2376: connection refused",
            Error::Unavailable,
        ),
        (
            "dial unix /run/docker.sock: permission denied",
            Error::Denied,
        ),
        ("no such container: api", Error::NotFound),
    ] {
        assert_eq!(
            adapter::command_error(message, false),
            expected,
            "{message}"
        );
    }
}

#[test]
fn switching_resource_lists_restores_cached_rows_and_ignores_an_obsolete_refresh() {
    let root = Path::new(".");
    let containers = Arc::new(
        adapter::parse(
            Kind::Containers,
            r#"{"ID":"api","Names":"api","State":"running"}"#,
        )
        .unwrap(),
    );
    let mut state = State {
        open: true,
        context: Some("local".into()),
        ..State::default()
    };
    let _ = state.update(Event::Listed(0, Ok(containers.clone())), root, false);
    let _ = state.update(Event::Kind(Kind::Images), root, false);
    let obsolete = state.request;
    assert!(state.items.is_empty());
    assert!(state.loading_inventory());

    let i18n = crate::i18n::I18n::new(crate::i18n::Language::English);
    let mut ui = iced_test::Simulator::with_size(
        iced::Settings::default(),
        (900., 420.),
        panel(&state, &i18n, false),
    );
    let list = ui
        .find(iced::widget::Id::new("docker-inventory"))
        .unwrap()
        .visible_bounds()
        .unwrap();
    let label = ui
        .find(i18n.text(Key::Loading))
        .unwrap()
        .visible_bounds()
        .unwrap();
    assert!((label.center_x() - list.center_x()).abs() <= 1.);
    assert!((label.center_y() - list.center_y()).abs() < 22.);
    drop(ui);

    let _ = state.update(Event::Kind(Kind::Containers), root, false);
    let _ = state.update(Event::Listed(obsolete, Ok(Arc::default())), root, false);
    assert!(Arc::ptr_eq(&state.items, &containers));
    assert_eq!(state.filtered, [0]);
    assert!(state.busy.is_none());
    let current = state.request;
    let _ = state.update(Event::Tick, root, false);
    assert_eq!(state.request, current);
    let _ = state.update(Event::Listed(current, Ok(Arc::default())), root, false);
    assert!(state.items.is_empty());
    state.change_connection("other".into());
    assert!(state.inventories.is_empty());
}

#[test]
fn registry_dialog_keeps_the_list_and_discards_unsaved_edits_on_cancel() {
    use crate::i18n::{I18n, Language};
    let root = Path::new(".");
    let reg = crate::domain::images::Registry::default();
    let mut state = State {
        open: true,
        profiles: vec![reg.clone()],
        ..State::default()
    };
    let _ = state.update(Event::Registry(registry::Event::Open), root, false);
    let i18n = I18n::new(Language::English);
    for edit in [false, true] {
        let _ = state.update(Event::Registry(registry::Event::Edit(edit)), root, false);
        assert!(state.has_dialog());
        let mut ui = iced_test::Simulator::with_size(
            iced::Settings::default(),
            (900., 560.),
            overlay(&state, &i18n, false, panel(&state, &i18n, false), |e| e),
        );
        assert!(ui.find(i18n.text(Key::RegistryCatalog)).is_ok());
        assert!(ui.find(i18n.text(Key::RegistrySave)).is_ok());
        drop(ui);
        let _ = state.update(
            Event::Registry(registry::Event::Name("temporary".into())),
            root,
            false,
        );
        let _ = state.update(Event::Registry(registry::Event::Cancel), root, false);
        assert!(!state.has_dialog());
        assert_eq!(state.registry.selected.as_ref(), Some(&reg));
        assert_eq!(state.profiles, [reg.clone()]);
    }
}

#[test]
fn disconnected_inventory_retries_and_restores_resources_after_docker_starts() {
    use crate::i18n::{I18n, Language};
    let item = adapter::parse(Kind::Volumes, r#"{"Name":"saved","Driver":"local"}"#).unwrap();
    let mut state = State {
        open: true,
        kind: Kind::Volumes,
        context: Some("desktop".into()),
        items: Arc::new(item.clone()),
        selected: Some("saved".into()),
        ..Default::default()
    };
    state.filter();
    let root = Path::new(".");
    let _ = state.update(Event::Listed(0, Err(Error::Stopped)), root, false);
    assert!(state.filtered.is_empty() && state.selected.is_none());
    for lang in [Language::English, Language::Russian, Language::Spanish] {
        let i18n = I18n::new(lang);
        let mut panel = iced_test::Simulator::with_size(
            iced::Settings::default(),
            (440., 260.),
            panel(&state, &i18n, false),
        );
        assert!(
            panel
                .find(i18n.text(Key::DockerStopped))
                .unwrap()
                .visible_bounds()
                .is_some()
        );
    }
    let _ = state.update(Event::Act(Action::Create), root, false);
    assert!(state.draft.is_none());
    let _ = state.update(Event::Tick, root, false);
    assert_eq!(state.busy, Some(Key::Loading));
    assert_eq!(state.connection_error, Some(Error::Stopped));
    let _ = state.update(Event::Listed(0, Ok(Arc::default())), root, false);
    assert_eq!(state.connection_error, Some(Error::Stopped));
    let _ = state.update(
        Event::Listed(state.request, Ok(Arc::new(item))),
        root,
        false,
    );
    assert_eq!(state.connection_error, None);
    assert_eq!(state.filtered.len(), 1);
}

#[test]
fn context_menu_keeps_properties_and_targets_the_clicked_container_for_removal() {
    use crate::i18n::{I18n, Language};
    let items = adapter::parse(Kind::Containers, "[{\"ID\":\"a\",\"Names\":\"first\",\"State\":\"exited\"},{\"ID\":\"b\",\"Names\":\"second\",\"State\":\"running\"}]").unwrap();
    let mut state = State {
        open: true,
        items: Arc::new(items),
        context: Some("local".into()),
        ..Default::default()
    };
    let root = Path::new(".");
    for original in [None, Some("a")] {
        state.selected = original.map(String::from);
        let _ = state.update(Event::Menu("b".into(), Point::new(30., 30.)), root, true);
        assert_eq!(state.selected.as_deref(), original);
        let i18n = I18n::new(Language::Russian);
        let mut ui = iced_test::Simulator::with_size(
            iced::Settings::default(),
            (780., 460.),
            overlay(&state, &i18n, true, panel(&state, &i18n, true), |e| e),
        );
        let label = ui
            .find(iced::widget::Id::new("docker-menu-remove"))
            .unwrap()
            .visible_bounds()
            .unwrap();
        ui.point_at(label.center());
        let _ = ui.simulate(iced_test::simulator::click());
        for e in ui.into_messages() {
            let _ = state.update(e, root, true);
        }
        let draft = state.draft.as_ref().unwrap();
        assert_eq!(draft.target.as_deref(), Some("b"));
        assert_eq!(
            draft
                .arguments(state.kind, state.find(draft.target.as_deref()))
                .unwrap(),
            ["container", "rm", "--force", "--", "b"]
        );
        let _ = state.update(Event::Dismiss, root, true);
        assert_eq!(state.selected.as_deref(), original);
    }
    assert_eq!(
        model::arguments(
            Kind::Containers,
            Action::Remove,
            state.find(Some("a")),
            "",
            ""
        )
        .unwrap(),
        ["container", "rm", "--", "a"]
    );
}

#[test]
fn dragging_the_properties_divider_resizes_the_pane_without_changing_selection() {
    use crate::i18n::{I18n, Language};
    use iced::{mouse, widget::Id};
    let i18n = I18n::new(Language::English);
    let mut state = State {
        items: Arc::new(
            adapter::parse(
                Kind::Containers,
                r#"{"ID":"c","Names":"web","State":"running"}"#,
            )
            .unwrap(),
        ),
        selected: Some("c".into()),
        ..Default::default()
    };
    state.filter();
    let mut ui = iced_test::Simulator::with_size(
        iced::Settings::default(),
        (1000., 360.),
        panel(&state, &i18n, true),
    );
    let before = ui
        .find(Id::new("docker-properties"))
        .unwrap()
        .visible_bounds()
        .unwrap();
    let start = Point::new(before.x + 1., before.center_y());
    let end = Point::new(start.x - 70., start.y);
    ui.point_at(start);
    ui.simulate([iced::Event::Mouse(mouse::Event::ButtonPressed(
        mouse::Button::Left,
    ))]);
    ui.point_at(end);
    ui.simulate([
        iced::Event::Mouse(mouse::Event::CursorMoved { position: end }),
        iced::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)),
    ]);
    for event in ui.into_messages() {
        let _ = state.update(event, Path::new("."), true);
    }
    let mut ui = iced_test::Simulator::with_size(
        iced::Settings::default(),
        (1000., 360.),
        panel(&state, &i18n, true),
    );
    let after = ui
        .find(Id::new("docker-properties"))
        .unwrap()
        .visible_bounds()
        .unwrap();
    assert!((after.width - before.width - 70.).abs() < 1.);
    assert_eq!(state.selected.as_deref(), Some("c"));
}

#[test]
fn image_links_follow_the_digest_when_tags_move_and_exclude_descendants() {
    let rows = adapter::parse(Kind::Images, r#"[{"ID":"new","Repository":"app","Tag":"latest"},{"ID":"old","Repository":"app","Tag":"saved"},{"ID":"old","Repository":"alt","Tag":"v1"}]"#).unwrap();
    assert_eq!(
        relations::matching_image(&rows, "new", "app").unwrap().name,
        "app:latest"
    );
    assert_eq!(
        relations::matching_image(&rows, "old", "app:saved")
            .unwrap()
            .name,
        "app:saved"
    );
    assert_eq!(
        relations::matching_image(&rows, "old", "app:latest")
            .unwrap()
            .detail,
        "old"
    );
    assert!(relations::matching_image(&rows, "missing", "app:latest").is_none());
    let list = relations::exact_running(
        "a old running /web\nb old paused /paused\nc child running /other\nd old exited /ended",
        "old",
    )
    .unwrap();
    assert_eq!(
        list,
        [relations::Running {
            id: "a".into(),
            name: "web".into()
        }]
    );
}
#[test]
fn following_a_container_image_reveals_its_page_and_links_back_to_running_containers() {
    use crate::i18n::{I18n, Language};
    use iced::widget::Id;
    let mut state = State {
        open: true,
        context: Some("local".into()),
        items: Arc::new(
            adapter::parse(
                Kind::Containers,
                r#"{"ID":"a","Names":"web","Image":"app:v1","State":"running"}"#,
            )
            .unwrap(),
        ),
        selected: Some("a".into()),
        query: "web".into(),
        ..Default::default()
    };
    state.filter();
    let root = Path::new(".");
    for lang in [Language::English, Language::Russian, Language::Spanish] {
        let i18n = I18n::new(lang);
        let mut ui = iced_test::Simulator::with_size(
            iced::Settings::default(),
            (1000., 460.),
            panel(&state, &i18n, true),
        );
        ui.click(Id::new("docker-open-image")).unwrap();
        assert!(
            ui.into_messages()
                .any(|message| matches!(message, Event::OpenImage))
        );
    }
    let old = state.items.clone();
    let _ = state.update(Event::OpenImage, root, true);
    let json = (0..51)
        .map(|i| format!(r#"{{"ID":"sha{i:02}","Repository":"img","Tag":"{i:02}"}}"#))
        .collect::<Vec<_>>()
        .join("\n");
    let items = Arc::new(adapter::parse(Kind::Images, &json).unwrap());
    let _ = state.update(
        Event::Linked(state.request, Kind::Images, Ok((items, "img:49".into()))),
        root,
        true,
    );
    assert_eq!(state.page, 1);
    assert_eq!(state.query, "");
    assert_eq!(state.selected().unwrap().detail, "sha49");
    let i18n = I18n::new(Language::English);
    let mut ui = iced_test::Simulator::with_size(
        iced::Settings::default(),
        (1000., 460.),
        panel(&state, &i18n, true),
    );
    ui.click(i18n.text(Key::DockerContainers)).unwrap();
    for e in ui.into_messages() {
        let _ = state.update(e, root, true);
    }
    assert!(state.show_running);
    let _ = state.update(
        Event::RunningLoaded(
            state.request,
            Ok(Arc::new(vec![relations::Running {
                id: "a".into(),
                name: "web".into(),
            }])),
        ),
        root,
        true,
    );
    let mut ui = iced_test::Simulator::with_size(
        iced::Settings::default(),
        (1000., 460.),
        panel(&state, &i18n, true),
    );
    ui.click("web").unwrap();
    for e in ui.into_messages() {
        let _ = state.update(e, root, true);
    }
    let req = state.request;
    let _ = state.update(
        Event::Linked(req, Kind::Containers, Ok((old, "a".into()))),
        root,
        true,
    );
    assert_eq!(state.kind, Kind::Containers);
    assert_eq!(state.selected().unwrap().name, "web");
    assert!(!state.show_running);
    let _ = state.update(
        Event::RunningLoaded(req - 1, Ok(Arc::default())),
        root,
        true,
    );
    assert!(state.running.is_none());
}

#[test]
fn selecting_active_docker_tabs_and_rows_preserves_the_current_view() {
    let rows = adapter::parse(Kind::Images, r#"{"ID":"s","Repository":"app","Tag":"v2"}"#).unwrap();
    let mut state = State {
        kind: Kind::Images,
        selected: Some("app:v2".into()),
        items: Arc::new(rows),
        context: Some("remote".into()),
        contexts: vec!["remote".into()],
        show_running: true,
        query: "app".into(),
        request: 12,
        ..Default::default()
    };
    for event in [
        Event::Context("remote".into()),
        Event::Kind(Kind::Images),
        Event::Select("app:v2".into()),
        Event::ShowRunning(true),
    ] {
        let _ = state.update(event, Path::new("."), true);
        assert_eq!(state.request, 12);
        assert!(state.show_running && state.busy.is_none());
        assert_eq!(state.selected.as_deref(), Some("app:v2"));
        assert_eq!(state.query, "app");
    }
}

#[test]
fn scrolled_resource_popups_use_the_window_pointer_and_the_clicked_row() {
    use crate::i18n::{I18n, Language};
    use iced::{mouse, widget::Id};
    let json = (0..35)
        .map(|i| format!(r#"{{"ID":"id{i:02}","Names":"row{i:02}","State":"running"}}"#))
        .collect::<Vec<_>>()
        .join("\n");
    let mut state = State {
        open: true,
        items: Arc::new(adapter::parse(Kind::Containers, &json).unwrap()),
        ..Default::default()
    };
    state.filter();
    let i18n = I18n::new(Language::English);
    let mut ui = iced_test::Simulator::with_size(
        iced::Settings::default(),
        (680., 350.),
        panel(&state, &i18n, true),
    );
    let bounds = ui
        .find(Id::new("docker-rows"))
        .unwrap()
        .visible_bounds()
        .unwrap();
    let point = Point::new(bounds.center_x(), bounds.y + 80.);
    ui.point_at(point);
    ui.simulate([iced::Event::Mouse(mouse::Event::WheelScrolled {
        delta: mouse::ScrollDelta::Pixels { x: 0., y: -160. },
    })]);
    let (expected, point) = (0..35)
        .find_map(|i| {
            let area = ui.find(format!("row{i:02}")).ok()?.visible_bounds()?;
            (area.y > bounds.y + 25. && area.y < bounds.y + 120.)
                .then(|| (format!("id{i:02}"), area.center()))
        })
        .unwrap();
    assert_ne!(expected, "id00");
    ui.point_at(point);
    ui.simulate([iced::Event::Mouse(mouse::Event::ButtonPressed(
        mouse::Button::Right,
    ))]);
    assert!(
        ui.into_messages()
            .any(|event| matches!(event,Event::Menu(id,anchor) if id==expected && anchor==point))
    );
}

#[test]
fn ssh_dialog_shows_two_labeled_fields_without_a_nested_connection_section() {
    use crate::i18n::{I18n, Language};
    for language in [Language::Russian, Language::Spanish, Language::English] {
        let i18n = I18n::new(language);
        let mut state = State {
            open: true,
            ..Default::default()
        };
        let _ = state.update(Event::Act(Action::AddContext), Path::new("."), true);
        let mut ui = iced_test::Simulator::with_size(
            iced::Settings::default(),
            (620., 560.),
            overlay(
                &state,
                &i18n,
                true,
                iced::widget::space()
                    .width(iced::Length::Fill)
                    .height(iced::Length::Fill)
                    .into(),
                |e| e,
            ),
        );
        assert!(ui.find(i18n.text(Key::DockerConnection)).is_err());
        for key in [Key::DockerName, Key::DockerEndpoint, Key::Cancel] {
            assert!(ui.find(i18n.text(key)).unwrap().visible_bounds().is_some());
        }
    }
}
