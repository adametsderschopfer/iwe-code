use super::*;
#[test]
fn graph_joins_direct_resources_and_retains_slots_after_inventory_changes() {
    let mut data = Resources::default();
    for (kind, id) in [
        (Kind::Images, "sha"),
        (Kind::Containers, "web"),
        (Kind::Volumes, "v"),
        (Kind::Networks, "net"),
        (Kind::Networks, "unused"),
    ] {
        data.names.insert(Id(kind, id.into()), id.into());
    }
    data.network_ids.insert("development".into(), "net".into());
    data.attachments(
        "web\tsha\tstack\tname:development id:vanished \tv ",
        "stack",
    )
    .unwrap();
    let old = data.layout(&Data::default());
    assert_eq!(old.nodes.len(), 6);
    assert_eq!(old.edges.len(), 4);
    let slot = |name: &str| {
        old.nodes
            .iter()
            .find(|node| node.name == name)
            .unwrap()
            .slot
    };
    assert!(slot("net") < slot("unused") && slot("v") < slot("unused"));
    let mut data = Resources::default();
    for node in &old.nodes {
        data.names.insert(node.id.clone(), node.name.clone());
    }
    data.names
        .insert(Id(Kind::Networks, "aaa".into()), "added".into());
    let next = data.layout(&old);
    for node in old.nodes {
        assert_eq!(
            next.nodes.iter().find(|n| n.id == node.id).unwrap().slot,
            node.slot
        );
    }
}

#[test]
fn graph_selection_drag_zoom_and_repeated_open_keep_the_active_view() {
    use crate::{
        i18n::{I18n, Key, Language},
        modules::docker::{Event as Docker, State, adapter, overlay, panel},
    };
    use iced::{Point, mouse, widget::Id as WidgetId};
    let mut state = State {
        open: true,
        context: Some("local".into()),
        kind: Kind::Compose,
        items: Arc::new(adapter::parse(Kind::Compose, r#"[{"Name":"stack"}]"#).unwrap()),
        ..Default::default()
    };
    let root = Path::new(".");
    let _ = state.update(Docker::ProjectGraph("stack".into()), root, true);
    let id = Id(Kind::Images, "sha".into());
    let data = Arc::new(Data {
        nodes: vec![Node {
            id: id.clone(),
            name: "app".into(),
            slot: 0,
        }],
        edges: vec![],
    });
    let _ = state.update(Docker::GraphLoaded(state.request, Ok(data)), root, true);
    let req = state.request;
    let _ = state.update(Docker::ProjectGraph("stack".into()), root, true);
    assert_eq!(state.request, req);
    let i18n = I18n::new(Language::Russian);
    let mut ui = iced_test::Simulator::with_size(
        iced::Settings::default(),
        (920., 430.),
        overlay(&state, &i18n, true, panel(&state, &i18n, true), |e| e),
    );
    let box_ = ui
        .find(WidgetId::new("docker-graph-canvas"))
        .unwrap()
        .visible_bounds()
        .unwrap();
    let start = Point::new(box_.x + 70., box_.y + 40.);
    let moved = Point::new(start.x + 40., start.y + 20.);
    ui.point_at(start);
    ui.simulate([iced::Event::Mouse(mouse::Event::ButtonPressed(
        mouse::Button::Left,
    ))]);
    ui.point_at(moved);
    ui.simulate([
        iced::Event::Mouse(mouse::Event::CursorMoved { position: moved }),
        iced::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)),
        iced::Event::Mouse(mouse::Event::WheelScrolled {
            delta: mouse::ScrollDelta::Lines { x: 0., y: 1. },
        }),
    ]);
    let events = ui.into_messages().collect::<Vec<_>>();
    assert!(
        events
            .iter()
            .any(|e| matches!(e,Docker::Diagram(Interaction::Move(key,_)) if key==&id))
    );
    assert!(
        events
            .iter()
            .any(|e| matches!(e,Docker::Diagram(Interaction::Zoom(factor,_)) if *factor>1.))
    );
    for e in events {
        let _ = state.update(e, root, true);
    }
    assert_eq!(
        state.project_graph.as_ref().unwrap().viewport.selected,
        Some(id.clone())
    );
    let mut ui = iced_test::Simulator::with_size(
        iced::Settings::default(),
        (920., 430.),
        overlay(&state, &i18n, true, panel(&state, &i18n, true), |e| e),
    );
    ui.click(i18n.text(Key::DockerOpenResource)).unwrap();
    assert!(
        ui.into_messages()
            .any(|e| matches!(e,Docker::OpenNode(key) if key==id))
    );
    let _ = state.update(Docker::GraphLoaded(req - 1, Ok(Arc::default())), root, true);
    assert!(!state.project_graph.as_ref().unwrap().data.nodes.is_empty());
    let _ = state.update(Docker::GraphLoaded(req, Ok(Arc::default())), root, true);
    assert!(
        state
            .project_graph
            .as_ref()
            .unwrap()
            .viewport
            .selected
            .is_none()
    );
}

#[test]
fn project_graph_uses_exact_membership_and_deduplicates_shared_attachments() {
    use super::super::adapter;
    let containers = adapter::parse(
        Kind::Containers,
        r#"[
        {"ID":"a","Names":"app-web","Image":"web:latest"},
        {"ID":"b","Names":"app-worker","Image":"web:latest"},
        {"ID":"c","Names":"app-old-web","Image":"web:latest"},
        {"ID":"x","Names":"disappeared","Image":"gone"}]"#,
    )
    .unwrap();
    let networks = adapter::parse(
        Kind::Networks,
        r#"[
        {"ID":"n","Name":"common"},{"ID":"o","Name":"outside"}]"#,
    )
    .unwrap();
    let records =
        "a\tsha\tapp\tid:n\tv\nb\tsha\tapp\tname:common\tv\nc\tsha\tapp-old\tid:n id:o\tv other";
    for (project, expected) in [
        (
            "app",
            vec!["app-web", "app-worker", "web:latest", "common", "v", "app"],
        ),
        ("gone", vec![]),
    ] {
        let mut resources = Resources::default();
        resources.inventory(Kind::Containers, &containers);
        resources.inventory(Kind::Networks, &networks);
        resources.attachments(records, project).unwrap();
        resources.limit_to_attachments(&containers);
        let graph = resources.layout(&Data::default());
        assert_eq!(
            graph
                .nodes
                .iter()
                .map(|n| n.name.as_str())
                .collect::<Vec<_>>(),
            expected
        );
        assert_eq!(graph.edges.len(), if project == "app" { 8 } else { 0 });
    }
}

#[test]
fn project_context_window_preserves_inventory_and_ignores_late_results() {
    use super::super::{Event, State, adapter, overlay, panel};
    use crate::i18n::{I18n, Key, Language};
    use iced::{Point, widget::Id as WidgetId};
    for language in [Language::English, Language::Russian, Language::Spanish] {
        let i18n = I18n::new(language);
        let projects =
            adapter::parse(Kind::Compose, r#"[{"Name":"demo"},{"Name":"demo-test"}]"#).unwrap();
        let mut state = State {
            open: true,
            context: Some("local".into()),
            kind: Kind::Compose,
            items: Arc::new(projects),
            selected: Some("demo".into()),
            query: "demo".into(),
            ..Default::default()
        };
        state.filter();
        let root = Path::new(".");
        let _ = state.update(
            Event::Menu("demo-test".into(), Point::new(40., 60.)),
            root,
            true,
        );
        let mut menu = iced_test::Simulator::with_size(
            iced::Settings::default(),
            (800., 480.),
            overlay(&state, &i18n, true, panel(&state, &i18n, true), |e| e),
        );
        menu.click(WidgetId::new("docker-menu-project-graph"))
            .unwrap();
        for e in menu.into_messages() {
            let _ = state.update(e, root, true);
        }
        assert!(state.has_dialog());
        assert_eq!(state.project_graph.as_ref().unwrap().name, "demo-test");
        let request = state.request;
        let _ = state.update(
            Event::GraphLoaded(request, Err(Error::Unavailable)),
            root,
            true,
        );
        assert_eq!(
            state.project_graph.as_ref().unwrap().error,
            Some(Error::Unavailable)
        );
        assert!(state.connection_error.is_none());
        let _ = state.update(Event::Refresh, root, true);
        let retry = state.request;
        assert!(retry > request);
        let _ = state.update(Event::GraphLoaded(retry, Ok(Arc::default())), root, true);
        let mut window = iced_test::Simulator::with_size(
            iced::Settings::default(),
            (520., 370.),
            overlay(&state, &i18n, true, panel(&state, &i18n, true), |e| e),
        );
        assert!(
            window
                .find(i18n.text(Key::DockerNoProjectContainers))
                .unwrap()
                .visible_bounds()
                .is_some()
        );
        window
            .click(WidgetId::new("docker-close-project-graph"))
            .unwrap();
        let closing = window.into_messages().collect::<Vec<_>>();
        let _ = state.update(Event::Refresh, root, true);
        let stale = state.request;
        let _ = state.update(Event::Search("background edit".into()), root, true);
        for e in closing {
            let _ = state.update(e, root, true);
        }
        assert!(!state.has_dialog());
        assert_eq!(state.query, "demo");
        assert_eq!(state.selected.as_deref(), Some("demo"));
        assert!(state.busy.is_none());
        let mut properties = iced_test::Simulator::with_size(
            iced::Settings::default(),
            (800., 480.),
            panel(&state, &i18n, true),
        );
        properties.click(i18n.text(Key::DockerGraph)).unwrap();
        for event in properties.into_messages() {
            let _ = state.update(event, root, true);
        }
        assert_eq!(state.project_graph.as_ref().unwrap().name, "demo");
        let _ = state.update(
            Event::GraphLoaded(stale, Err(Error::Unavailable)),
            root,
            true,
        );
        assert!(state.busy.is_some());
        assert!(state.project_graph.as_ref().unwrap().error.is_none());
        assert!(state.project_graph.as_ref().unwrap().data.nodes.is_empty());
    }
}

#[test]
fn docker_navigation_contains_only_resource_sections() {
    use super::super::{Event, State, panel};
    use crate::i18n::{I18n, Key, Language};
    use iced::widget::Id;
    let i18n = I18n::new(Language::English);
    for kind in Kind::ALL {
        let mut state = State {
            open: true,
            kind,
            ..Default::default()
        };
        let mut ui = iced_test::Simulator::with_size(
            iced::Settings::default(),
            (800., 480.),
            panel(&state, &i18n, true),
        );
        for id in [
            "docker-kind-containers",
            "docker-kind-images",
            "docker-kind-networks",
            "docker-kind-volumes",
            "docker-kind-compose",
            "docker-registries",
        ] {
            assert!(ui.find(Id::new(id)).is_ok());
        }
        assert!(ui.find(Id::new("docker-graph-tab")).is_err());
        assert!(ui.find(i18n.text(Key::DockerGraph)).is_err());
        drop(ui);
        let _ = state.update(Event::ProjectGraph("missing".into()), Path::new("."), true);
        assert!(!state.has_dialog());
    }
}
