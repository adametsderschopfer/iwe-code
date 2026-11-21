use super::*;
use crate::{
    app::Navigation,
    domain::workspace::FileEntry,
    i18n::{I18n, Key, Language},
};
use std::rc::Rc;

fn app() -> App {
    let mut app = App::new(Rc::new(I18n::new(Language::English)));
    app.loaded = true;
    app.navigation = Navigation::Files;
    app.workspace
        .create_space("Work".to_owned(), vec![PathBuf::from("/project")]);
    let _ = app.load_file_tree();
    app
}

fn directory(app: &mut App, path: &str, entries: Vec<FileEntry>) {
    let _ = app.update_files(Event::Index(
        app.files.epoch,
        project_index::Event::Directory(path.into(), Ok(entries)),
    ));
}

fn entry(path: &str, directory: bool) -> FileEntry {
    let path = PathBuf::from(path);
    FileEntry {
        name: path.file_name().unwrap().to_string_lossy().into_owned(),
        path,
        directory,
    }
}

#[test]
fn project_contents_stay_visible_without_a_collapsible_root() {
    use iced::widget::Id;
    for (language, suffix) in [
        (Language::English, "en"),
        (Language::Russian, "ru"),
        (Language::Spanish, "es"),
    ] {
        let mut app = app();
        app.i18n = Rc::new(I18n::new(language));
        directory(
            &mut app,
            "/project",
            vec![
                entry("/project/src", true),
                entry("/project/README.md", false),
            ],
        );
        directory(
            &mut app,
            "/project/src",
            vec![entry("/project/src/main.rs", false)],
        );
        app.expanded.remove(&PathBuf::from("/project"));
        let settings = || iced::Settings {
            default_font: crate::ui::theme::font(),
            ..Default::default()
        };
        let mut ui = iced_test::Simulator::with_size(settings(), (1080., 720.), app.view());
        assert!(ui.find(Id::new("file-tree-/project")).is_err());
        assert!(ui.find(Id::new("space-project-0-0")).is_ok());
        assert!(ui.find("Work • project").is_ok());
        let source = ui.find("src").unwrap().visible_bounds().unwrap();
        let readme = ui.find("README.md").unwrap().visible_bounds().unwrap();
        assert_eq!(source.x, readme.x);
        assert!(ui.find("main.rs").is_err());
        ui.click(Id::new("file-tree-/project/src")).unwrap();
        for message in ui.into_messages() {
            let _ = app.update(message);
        }
        let mut ui = iced_test::Simulator::with_size(settings(), (1080., 720.), app.view());
        let child = ui.find("main.rs").unwrap().visible_bounds().unwrap();
        assert_eq!(child.x - source.x, 20.);
        if let Some(output) = std::env::var_os("IWE_RENDER_DIR").map(PathBuf::from) {
            ui.snapshot(&iced::Theme::Dark)
                .unwrap()
                .matches_image(output.join(format!("project-contents-{suffix}")))
                .unwrap();
        }
        ui.click(Id::new("file-tree-/project/src/main.rs")).unwrap();
        for message in ui.into_messages() {
            let _ = app.update(message);
        }
        assert_eq!(
            app.preview().path,
            Some(PathBuf::from("/project/src/main.rs"))
        );
        let mut ui = iced_test::Simulator::with_size(settings(), (1080., 720.), app.view());
        ui.click(Id::new("file-tree-/project/src")).unwrap();
        for message in ui.into_messages() {
            let _ = app.update(message);
        }
        let mut ui = iced_test::Simulator::with_size(settings(), (1080., 720.), app.view());
        assert!(ui.find(Id::new("file-tree-/project/src/main.rs")).is_err());
        assert!(ui.find("README.md").is_ok());
        assert_eq!(
            app.preview().path,
            Some(PathBuf::from("/project/src/main.rs"))
        );
    }
}

#[test]
fn cached_folders_stay_visible_across_refresh_and_project_switches() {
    let mut app = app();
    directory(&mut app, "/project", vec![entry("/project/src", true)]);
    directory(
        &mut app,
        "/project/src",
        vec![entry("/project/src/main.rs", false)],
    );
    let _ = app.toggle_folder("/project/src".into());
    assert!(!app.files.loading.contains(&PathBuf::from("/project/src")));
    let epoch = app.files.epoch;
    let _ = app.load_file_tree();
    assert_eq!(app.files.epoch, epoch);
    assert_eq!(app.tree[&PathBuf::from("/project/src")][0].name, "main.rs");
    app.workspace.add_project(0, "/other".into());
    let _ = app.load_file_tree();
    assert!(app.tree.is_empty());
    let _ = app.update_files(Event::Index(
        epoch,
        project_index::Event::Directory("/project".into(), Ok(vec![])),
    ));
    assert!(app.tree.is_empty());
    app.workspace.select_project(0, 0);
    let _ = app.load_file_tree();
    assert!(app.expanded.contains(&PathBuf::from("/project/src")));
    assert_eq!(app.tree[&PathBuf::from("/project/src")][0].name, "main.rs");
}

#[test]
fn loading_appears_only_for_slow_uncached_reads_and_errors_preserve_cache() {
    let mut app = app();
    let path = PathBuf::from("/project");
    let request = app.files.pending[&path];
    assert!(app.files.loading.is_empty());
    let _ = app.update_files(Event::Slow(app.files.epoch, path.clone(), request));
    assert!(app.files.loading.contains(&path));
    let cached = project_index::Tree::from([(path.clone(), vec![entry("/project/src", true)])]);
    let _ = app.update_files(Event::Index(
        app.files.epoch,
        project_index::Event::Cached(cached),
    ));
    assert!(app.files.loading.is_empty());
    let _ = app.update_files(Event::Slow(app.files.epoch, path.clone(), request));
    assert!(app.files.loading.is_empty());
    let _ = app.update_files(Event::Index(
        app.files.epoch,
        project_index::Event::Directory(path.clone(), Err(Failure::Read)),
    ));
    assert_eq!(app.files.errors[&path], Failure::Read);
    assert_eq!(app.tree[&path][0].name, "src");
    let mut ui =
        iced_test::Simulator::with_size(iced::Settings::default(), (1080., 720.), app.view());
    ui.click(app.i18n.text(Key::Retry)).unwrap();
    assert!(matches!(
        ui.into_messages().collect::<Vec<_>>().as_slice(),
        [Message::Files(Event::Retry(directory))] if directory == &path
    ));
    directory(&mut app, "/project", vec![]);
    assert!(app.files.errors.is_empty());
    assert!(app.tree[&path].is_empty());
}

#[test]
fn removed_directories_drop_descendants_and_cancel_loading_indicators() {
    let mut app = app();
    directory(&mut app, "/project", vec![entry("/project/src", true)]);
    let _ = app.toggle_folder("/project/src".into());
    let pending = app.files.pending[&PathBuf::from("/project/src")];
    directory(&mut app, "/project", vec![]);
    let _ = app.update_files(Event::Slow(app.files.epoch, "/project/src".into(), pending));
    assert!(!app.expanded.contains(&PathBuf::from("/project/src")));
    assert!(app.files.loading.is_empty());
}

#[test]
fn oldest_cached_project_is_restored_before_cache_eviction() {
    let mut app = app();
    directory(
        &mut app,
        "/project",
        vec![entry("/project/remember.rs", false)],
    );
    for index in 1..=4 {
        app.workspace
            .add_project(0, format!("/project-{index}").into());
        let _ = app.load_file_tree();
    }
    assert_eq!(app.files.cache.len(), 4);
    app.workspace.select_project(0, 0);
    let _ = app.load_file_tree();
    assert_eq!(app.tree[&PathBuf::from("/project")][0].name, "remember.rs");
    assert_eq!(app.files.cache.len(), 4);
}

#[test]
fn cached_tree_has_no_loading_label_in_any_language() {
    for language in [Language::English, Language::Russian, Language::Spanish] {
        let mut app = app();
        app.i18n = Rc::new(I18n::new(language));
        directory(&mut app, "/project", vec![entry("/project/src", true)]);
        directory(
            &mut app,
            "/project/src",
            vec![entry("/project/src/main.rs", false)],
        );
        let _ = app.toggle_folder("/project/src".into());
        let mut ui = iced_test::Simulator::with_size(
            iced::Settings {
                default_font: crate::ui::theme::font(),
                ..Default::default()
            },
            (1080., 720.),
            app.view(),
        );
        assert!(ui.find("main.rs").is_ok());
        assert!(ui.find(app.i18n.text(Key::Loading)).is_err());
        if let Some(output) = std::env::var_os("IWE_RENDER_DIR").map(PathBuf::from) {
            let suffix = match language {
                Language::English => "en",
                Language::Russian => "ru",
                Language::Spanish => "es",
            };
            ui.snapshot(&iced::Theme::Dark)
                .unwrap()
                .matches_image(output.join(format!("cached-tree-{suffix}")))
                .unwrap();
        }
    }
}

#[test]
fn file_tree_centers_names_inside_each_row() {
    let mut app = app();
    directory(
        &mut app,
        "/project",
        vec![
            entry("/project/public", true),
            entry("/project/reports", true),
            entry("/project/main.rs", false),
        ],
    );
    app.files.selected_path = Some(PathBuf::from("/project/reports"));
    app.changes.status = crate::domain::changes::Worktree::new(
        std::path::Path::new("/project"),
        std::collections::BTreeMap::from([(
            PathBuf::from("/project/reports/index.rs"),
            crate::domain::changes::FileChange {
                status: crate::domain::changes::FileStatus::Modified,
                original: None,
            },
        )]),
    )
    .into();
    let mut ui = iced_test::Simulator::with_size(
        iced::Settings {
            default_font: crate::ui::theme::font(),
            ..Default::default()
        },
        (1080., 720.),
        app.view(),
    );
    for name in ["public", "reports", "main.rs"] {
        let label = ui.find(name).unwrap().visible_bounds().unwrap();
        let row = ui
            .find(iced::widget::Id::from(format!("file-tree-/project/{name}")))
            .unwrap()
            .visible_bounds()
            .unwrap();
        assert!((row.height - 24.).abs() < 0.001);
        assert!((label.center_y() - row.center_y()).abs() < 0.1);
    }
    if let Some(output) = std::env::var_os("IWE_RENDER_DIR").map(PathBuf::from) {
        ui.snapshot(&iced::Theme::Dark)
            .unwrap()
            .matches_image(output.join("tree-centered"))
            .unwrap();
    }
}

#[test]
fn compact_tree_keeps_full_filenames_and_folder_selection() {
    let mut app = app();
    let long_name = "a_very_long_component_filename_that_must_remain_complete.tsx";
    directory(&mut app, "/project", vec![entry("/project/src", true)]);
    directory(
        &mut app,
        "/project/src",
        vec![
            entry(&format!("/project/src/{long_name}"), false),
            entry("/project/src/next.py", false),
        ],
    );
    let _ = app.toggle_folder("/project/src".into());
    let mut ui =
        iced_test::Simulator::with_size(iced::Settings::default(), (1080., 720.), app.view());
    let first = ui.find(long_name).unwrap().visible_bounds().unwrap();
    let second = ui.find("next.py").unwrap().visible_bounds().unwrap();
    assert!((second.y - first.y - 24.).abs() < 0.1);
    ui.click("next.py").unwrap();
    for message in ui.into_messages() {
        let _ = app.update(message);
    }
    assert_eq!(
        app.files.selected_path,
        Some(PathBuf::from("/project/src/next.py"))
    );
    let _ = app.toggle_folder("/project/src".into());
    assert_eq!(app.files.selected_path, Some(PathBuf::from("/project/src")));
    let mut ui =
        iced_test::Simulator::with_size(iced::Settings::default(), (1080., 720.), app.view());
    assert!(ui.find(long_name).is_err());
}

#[test]
fn collapsing_a_folder_keeps_descendants_collapsed_when_reopened() {
    let mut app = app();
    directory(
        &mut app,
        "/project",
        vec![
            entry("/project/src", true),
            entry("/project/src-other", true),
        ],
    );
    directory(
        &mut app,
        "/project/src",
        vec![entry("/project/src/nested", true)],
    );
    directory(
        &mut app,
        "/project/src/nested",
        vec![entry("/project/src/nested/deep", true)],
    );
    for path in [
        "/project/src",
        "/project/src/nested",
        "/project/src/nested/deep",
        "/project/src-other",
    ] {
        let _ = app.toggle_folder(path.into());
    }
    let deep = PathBuf::from("/project/src/nested/deep");
    let request = app.files.pending[&deep];
    let _ = app.update_files(Event::Slow(app.files.epoch, deep.clone(), request));
    assert!(app.files.loading.contains(&deep));

    let _ = app.toggle_folder("/project/src".into());
    assert!(
        !app.expanded
            .iter()
            .any(|path| path.starts_with("/project/src"))
    );
    assert!(app.expanded.contains(&PathBuf::from("/project/src-other")));
    assert!(app.files.loading.is_empty());
    assert!(app.tree.contains_key(&PathBuf::from("/project/src/nested")));

    let _ = app.update_files(Event::Slow(app.files.epoch, deep, request));
    assert!(app.files.loading.is_empty());
    let _ = app.toggle_folder("/project/src".into());
    assert!(app.expanded.contains(&PathBuf::from("/project/src")));
    assert!(!app.expanded.contains(&PathBuf::from("/project/src/nested")));
    assert!(
        !app.expanded
            .contains(&PathBuf::from("/project/src/nested/deep"))
    );
    assert!(app.expanded.contains(&PathBuf::from("/project/src-other")));
}

#[test]
fn switching_file_menu_rows_rejects_stale_submenu_events() {
    let mut app = app();
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().to_path_buf();
    app.workspace
        .create_space("Menus".into(), vec![path.clone()]);
    let point = iced::Point::new(80., 100.);
    let _ = app.update_files(Event::OpenMenu(path.clone(), point));
    for kind in [Submenu::New, Submenu::CopyPath] {
        let mut ui =
            iced_test::Simulator::with_size(iced::Settings::default(), (1080., 720.), app.view());
        let bounds = ui.find(kind.id()).unwrap().visible_bounds().unwrap();
        let position = bounds.center();
        ui.point_at(position);
        ui.simulate([iced::Event::Mouse(iced::mouse::Event::CursorMoved {
            position,
        })]);
        for message in ui.into_messages() {
            let _ = app.update(message);
        }
        let _ = app.update_files(Event::OptionsAnchor(
            kind,
            path.clone(),
            point,
            Some(bounds),
        ));
        assert_eq!(
            app.files.menu.as_ref().unwrap().submenu,
            Some((kind, Some(bounds)))
        );
    }
    let _ = app.update_files(Event::CloseOptions(Submenu::New));
    assert!(matches!(
        app.files.menu.as_ref().unwrap().submenu,
        Some((Submenu::CopyPath, _))
    ));
    let mut ui =
        iced_test::Simulator::with_size(iced::Settings::default(), (1080., 720.), app.view());
    let position = ui
        .find(app.i18n.text(Key::FileReveal).as_str())
        .unwrap()
        .visible_bounds()
        .unwrap()
        .center();
    ui.point_at(position);
    ui.simulate([iced::Event::Mouse(iced::mouse::Event::CursorMoved {
        position,
    })]);
    for message in ui.into_messages() {
        let _ = app.update(message);
    }
    assert!(app.files.menu.as_ref().unwrap().submenu.is_none());
}

#[test]
fn root_and_file_menus_expose_only_applicable_commands() {
    use iced::Event as UiEvent;
    use iced::mouse;
    let project = tempfile::tempdir().unwrap();
    let root = project.path();
    let file = root.join("example.rs");
    let mut app = App::new(Rc::new(I18n::new(Language::English)));
    app.loaded = true;
    app.navigation = Navigation::Files;
    app.workspace.create_space("Work".into(), vec![root.into()]);
    let _ = app.load_file_tree();
    directory(
        &mut app,
        root.to_str().unwrap(),
        vec![entry(file.to_str().unwrap(), false)],
    );
    for (id, expected) in [
        (format!("file-tree-{}", file.display()), file.as_path()),
        ("project-files-browser".into(), root),
    ] {
        let settings = iced::Settings::default();
        let mut ui = iced_test::Simulator::with_size(settings, (1080., 720.), app.view());
        let bounds = ui
            .find(iced::widget::Id::from(id))
            .unwrap()
            .visible_bounds()
            .unwrap();
        ui.point_at(bounds.center());
        ui.simulate([UiEvent::Mouse(mouse::Event::ButtonPressed(
            mouse::Button::Right,
        ))]);
        let messages = ui.into_messages().collect::<Vec<_>>();
        assert!(
            matches!(messages.as_slice(), [Message::Files(Event::OpenMenu(path, _))] if path == expected),
            "{messages:?}"
        );
    }
    for language in [Language::English, Language::Russian, Language::Spanish] {
        app.i18n = Rc::new(I18n::new(language));
        let _ = app.update_files(Event::OpenMenu(root.into(), iced::Point::new(300., 200.)));
        let mut ui =
            iced_test::Simulator::with_size(iced::Settings::default(), (1080., 720.), app.view());
        assert!(ui.find(app.i18n.text(Key::FileNew).as_str()).is_ok());
        assert!(ui.find(app.i18n.text(Key::CopyPathMenu).as_str()).is_ok());
        assert!(
            ui.find(app.i18n.text(Key::FileOpenTerminal).as_str())
                .is_ok()
        );
        assert!(ui.find(app.i18n.text(Key::FileTrash).as_str()).is_err());
    }
}

#[tokio::test]
async fn tree_terminals_start_shells_in_the_chosen_directory_and_plus_uses_the_root() {
    use crate::{app::modules, modules::terminal};
    use iced::futures::StreamExt;
    use iced_test::runtime::{Action, task};

    let fixture = tempfile::tempdir().unwrap();
    let root = fixture.path();
    let folder = root.join("папка с 'кавычками'");
    let file = folder.join("data.txt");
    std::fs::create_dir(&folder).unwrap();
    std::fs::write(&file, "").unwrap();
    let mut app = App::new(Rc::new(I18n::new(Language::English)));
    app.loaded = true;
    app.workspace.create_space("Work".into(), vec![root.into()]);
    app.workspace.preferences.show_git = false;
    app.workspace.preferences.show_pipelines = false;
    let _ = app.load_file_tree();
    directory(
        &mut app,
        root.to_str().unwrap(),
        vec![entry(folder.to_str().unwrap(), true)],
    );
    directory(
        &mut app,
        folder.to_str().unwrap(),
        vec![entry(file.to_str().unwrap(), false)],
    );

    for (index, selected) in [Some(&folder), Some(&file), None].into_iter().enumerate() {
        let start = match selected {
            Some(path) => {
                let _ = app.update_files(Event::OpenMenu(path.clone(), iced::Point::ORIGIN));
                app.update_files(Event::Operation(operations::Event::Command(
                    operations::Command::OpenTerminal,
                )))
            }
            None => app.update_module(modules::Event::Terminal(terminal::Event::Start)),
        };
        assert!(app.files.menu.is_none());
        assert!(app.modules.terminal.open);
        assert_eq!(app.modules.terminal.tabs.len(), index + 1);
        let mut actions = task::into_stream(start).unwrap();
        while let Some(action) = actions.next().await {
            if let Action::Output(
                message @ Message::Module(modules::Event::Terminal(terminal::Event::Started(..))),
            ) = action
            {
                let _ = app.update(message);
                break;
            }
        }
        let tab = app.modules.terminal.active_tab().unwrap();
        assert!(tab.running());
        let marker = format!("terminal-cwd-{index}");
        let input = format!("echo ready > {marker}\r");
        let _ = app.update_module(modules::Event::Terminal(terminal::Event::Input(
            tab.request,
            input.into_bytes(),
        )));
        let expected = if selected.is_some() { &folder } else { root };
        tokio::time::timeout(std::time::Duration::from_secs(10), async {
            while !expected.join(&marker).is_file() {
                tokio::time::sleep(std::time::Duration::from_millis(20)).await;
            }
        })
        .await
        .expect("the shell should create its marker in its startup directory");
    }
}

#[test]
fn audit_directory_updates_at_index_limit() {
    if std::env::var_os("IWE_PROFILE_WORKSPACE").is_none() {
        return;
    }
    for _ in 0..3 {
        let mut app = app();
        let start = std::time::Instant::now();
        for i in 0..4095 {
            directory(&mut app, &format!("/project/dir-{i}"), Vec::new());
        }
        eprintln!("directory updates {:?}", start.elapsed());
        assert_eq!(app.tree.len(), 4095);
    }
}

#[test]
fn refreshing_one_subtree_prunes_removed_children_and_preserves_similar_siblings() {
    let mut app = app();
    for folder in [
        "/project/sub/keep",
        "/project/sub/old/deep",
        "/project/sub-old",
    ] {
        directory(&mut app, folder, Vec::new());
        app.expanded.insert(folder.into());
    }
    app.files.selected_path = Some("/project/sub/old/deep/file".into());
    directory(
        &mut app,
        "/project/sub",
        vec![entry("/project/sub/keep", true)],
    );
    assert!(
        app.tree
            .contains_key(std::path::Path::new("/project/sub-old"))
    );
    assert!(
        app.tree
            .contains_key(std::path::Path::new("/project/sub/keep"))
    );
    assert!(
        !app.tree
            .contains_key(std::path::Path::new("/project/sub/old/deep"))
    );
    assert!(
        !app.expanded
            .contains(std::path::Path::new("/project/sub/old/deep"))
    );
    assert!(app.files.selected_path.is_none());
}

#[test]
fn revealing_an_open_file_expands_uncached_parents_and_selects_the_file() {
    let mut app = app();
    let path = PathBuf::from("/project/src/deep/file.rs");
    let _ = app.open_file(path.clone());
    let _ = app.update_files(Event::RevealOpened);
    assert!(app.expanded.contains(&PathBuf::from("/project/src/deep")));
    assert_eq!(app.files.selected_path, Some(path.clone()));
    directory(&mut app, "/project", vec![entry("/project/src", true)]);
    directory(
        &mut app,
        "/project/src",
        vec![entry("/project/src/deep", true)],
    );
    assert!(app.files.reveal.is_some());
    directory(
        &mut app,
        "/project/src/deep",
        vec![entry("/project/src/deep/file.rs", false)],
    );
    assert!(app.files.reveal.is_none());
}
