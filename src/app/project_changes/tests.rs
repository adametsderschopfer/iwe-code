use super::*;
use crate::{
    domain::changes::{FileChange, FileStatus},
    i18n::{I18n, Language},
    ui::theme,
};
use std::rc::Rc;

fn app(language: Language) -> App {
    let mut app = App::new(Rc::new(I18n::new(language)));
    app.loaded = true;
    app.workspace.create_space(
        "Work".into(),
        vec![PathBuf::from("first"), PathBuf::from("active")],
    );
    app.workspace.select_project(0, 1);
    app
}

#[test]
fn inactive_project_indicators_follow_status_and_keep_last_known_results_on_errors() {
    let mut app = app(Language::English);
    let root = PathBuf::from("first");
    let generation = app.generation;
    for (result, expected) in [
        (Ok(true), true),
        (Err(Failure::Read), true),
        (Ok(false), false),
        (Ok(true), true),
        (Err(Failure::NotRepository), false),
    ] {
        let _ = app.refresh_project_changes();
        let request = app.project_changes.request;
        let _ = app.update_project_changes(Event::Loaded(
            request,
            generation,
            vec![(root.clone(), result)],
        ));
        assert!(!app.project_changes.pending);
        assert_eq!(app.project_has_changes(&root), expected);
    }
}

#[test]
fn obsolete_results_cannot_overwrite_active_or_removed_projects() {
    let mut app = app(Language::English);
    let _ = app.refresh_project_changes();
    let request = app.project_changes.request;
    let generation = app.generation;
    let _ = app.update_project_changes(Event::Loaded(
        request,
        generation,
        vec![(PathBuf::from("active"), Ok(true))],
    ));
    assert!(!app.project_has_changes(Path::new("active")));
    let _ = app.refresh_project_changes();
    let request = app.project_changes.request;
    app.generation += 1;
    let _ = app.update_project_changes(Event::Loaded(
        request,
        generation,
        vec![(PathBuf::from("first"), Ok(true))],
    ));
    assert!(!app.project_changes.pending);
    assert!(!app.project_has_changes(Path::new("first")));
    let _ = app.refresh_project_changes();
    let request = app.project_changes.request;
    app.workspace.spaces[0].projects.remove(0);
    let _ = app.update_project_changes(Event::Loaded(
        request,
        app.generation,
        vec![(PathBuf::from("first"), Ok(true))],
    ));
    assert!(!app.project_has_changes(Path::new("first")));
}

#[tokio::test]
async fn background_scan_delivers_all_inactive_projects_in_one_message() {
    use iced_test::runtime::{Action, task};

    let fixture = tempfile::tempdir().unwrap();
    let roots = ["selected", "first", "second"].map(|name| {
        let path = fixture.path().join(name);
        std::fs::create_dir(&path).unwrap();
        path
    });
    let mut app = App::new(Rc::new(I18n::new(Language::English)));
    app.loaded = true;
    app.workspace.create_space("Work".into(), roots.to_vec());
    assert_eq!(app.workspace.directory(), Some(&roots[0]));

    let task = app.refresh_project_changes();
    let mut output = task::into_stream(task)
        .unwrap()
        .filter_map(async |action| match action {
            Action::Output(message) => Some(message),
            _ => None,
        })
        .boxed();
    let Some(Message::ProjectChanges(Event::Loaded(_, _, results))) = output.next().await else {
        panic!("expected one project scan result");
    };
    let returned: BTreeSet<_> = results.into_iter().map(|(root, _)| root).collect();
    assert_eq!(
        returned,
        BTreeSet::from([roots[1].clone(), roots[2].clone()])
    );
    assert!(output.next().await.is_none());
}

#[test]
fn partial_clean_results_preserve_known_changes_and_active_clean_status_clears_them() {
    let mut app = app(Language::English);
    let root = PathBuf::from("active");
    let dirty = Worktree::new(
        &root,
        BTreeMap::from([(
            root.join("new.rs"),
            FileChange {
                status: FileStatus::Untracked,
                original: None,
            },
        )]),
    );
    app.remember_project_changes(root.clone(), &dirty);
    let mut partial = Worktree::default();
    partial.failures.push((root.clone(), Failure::Read));
    app.remember_project_changes(root.clone(), &partial);
    assert!(app.project_has_changes(&root));
    app.changes.loaded = true;
    assert!(!app.project_has_changes(&root));
    app.remember_project_changes(root.clone(), &Worktree::default());
    app.changes.loaded = false;
    assert!(!app.project_has_changes(&root));
}

#[test]
fn dirty_indicators_appear_for_pinned_and_active_projects_in_every_language() {
    use iced::widget::Id;
    for language in [Language::English, Language::Russian, Language::Spanish] {
        let mut app = app(language);
        app.workspace.spaces[0].projects[0].pinned = true;
        app.project_changes
            .known
            .insert(PathBuf::from("first"), true);
        app.changes.loaded = true;
        app.changes.status.files.insert(
            PathBuf::from("active/file.rs"),
            FileChange {
                status: FileStatus::Modified,
                original: None,
            },
        );
        {
            let mut ui = iced_test::Simulator::with_size(
                iced::Settings {
                    default_font: theme::font(),
                    ..iced::Settings::default()
                },
                (1280., 900.),
                app.view(),
            );
            for project in 0..2 {
                let badge = ui
                    .find(Id::from(format!("project-changes-0-{project}")))
                    .unwrap()
                    .visible_bounds()
                    .unwrap();
                let row = ui
                    .find(Id::from(format!("space-project-0-{project}")))
                    .unwrap()
                    .visible_bounds()
                    .unwrap();
                assert!(badge.center_x() > row.center_x());
                assert!(badge.x + badge.width <= row.x + row.width);
            }
        }
        app.project_changes.known.clear();
        app.changes.status.files.clear();
        let mut ui = iced_test::Simulator::with_size(
            iced::Settings {
                default_font: theme::font(),
                ..iced::Settings::default()
            },
            (1280., 900.),
            app.view(),
        );
        assert!(ui.find(Id::new("project-changes-0-0")).is_err());
        assert!(ui.find(Id::new("project-changes-0-1")).is_err());
    }
}
