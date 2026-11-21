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
    assert!(!Action::actions(Kind::Containers, &items[0], true).contains(&Action::Remove));
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
    for kind in [Kind::Networks, Kind::Volumes] {
        invoke(root, &context, kind, Action::Create, None, &name, "").await;
        let item = resource(root, &context, kind, &name).await;
        assert!(
            invoke(root, &context, kind, Action::Inspect, Some(&item), "", "")
                .await
                .contains(&name)
        );
        if kind == Kind::Networks {
            for action in [Action::Connect, Action::Disconnect] {
                invoke(root, &context, kind, action, Some(&item), &container.id, "").await;
            }
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
        format!("name: {name}\nservices:\n  probe:\n    image: {tag}\n"),
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
        assert!(nav.x + nav.width < list.x && list.x + list.width < details.x);
        assert_eq!(nav.y, details.y);
        assert_eq!(nav.width, 42.);
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
fn section_and_filter_borders_reach_all_four_corners_in_both_themes() {
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
        for id in ["docker-navigation", "docker-inventory", "docker-filter"] {
            let area = ui.find(Id::new(id)).unwrap().visible_bounds().unwrap();
            for y in [area.y + 0.5, area.y + area.height - 0.5] {
                let edge = pixel(area.center_x(), y);
                assert_ne!(edge, pixel(area.x - 2., y), "{id} has a visible border");
                assert_eq!(pixel(area.x + 0.5, y), edge, "{id} left corner");
                assert_eq!(
                    pixel(area.x + area.width - 0.5, y),
                    edge,
                    "{id} right corner"
                );
            }
        }
    }
}
