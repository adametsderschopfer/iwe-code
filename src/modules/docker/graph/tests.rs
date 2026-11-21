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
    data.attachments("web\tsha\tstack\tname:development id:vanished \tv ")
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
        modules::docker::{Event as Docker, State},
    };
    use iced::{Point, mouse, widget::Id as WidgetId};
    let mut state = State {
        open: true,
        context: Some("local".into()),
        ..Default::default()
    };
    let root = Path::new(".");
    let _ = state.update(Docker::Graph, root, true);
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
    let _ = state.update(Docker::Graph, root, true);
    assert_eq!(state.request, req);
    let i18n = I18n::new(Language::Russian);
    let mut ui = iced_test::Simulator::with_size(
        iced::Settings::default(),
        (920., 430.),
        super::super::panel(&state, &i18n, true),
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
    assert_eq!(state.diagram_view.selected, Some(id.clone()));
    let mut ui = iced_test::Simulator::with_size(
        iced::Settings::default(),
        (920., 430.),
        super::super::panel(&state, &i18n, true),
    );
    ui.click(i18n.text(Key::DockerOpenResource)).unwrap();
    assert!(
        ui.into_messages()
            .any(|e| matches!(e,Docker::OpenNode(key) if key==id))
    );
    let _ = state.update(Docker::GraphLoaded(req - 1, Ok(Arc::default())), root, true);
    assert!(!state.diagram.nodes.is_empty());
    let _ = state.update(Docker::GraphLoaded(req, Ok(Arc::default())), root, true);
    assert!(state.diagram_view.selected.is_none());
}
