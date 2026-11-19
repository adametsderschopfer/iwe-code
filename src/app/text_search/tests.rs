use super::*;
use crate::i18n::{I18n, Language};
use std::rc::Rc;
#[test]
fn obsolete_completion_cannot_replace_new_query() {
    let mut app = App::new(Rc::new(I18n::new(Language::English)));
    app.text_search.serial = 2;
    app.text_search.project = Some(Search::new(0, PathBuf::from("/project")));
    app.text_search.project.as_mut().unwrap().busy = true;
    let _ = app.update_text_search(Event::Loaded(true, 1, Results::default()));
    assert!(app.text_search.project.as_ref().unwrap().busy);
    app.text_search.cancel();
    let _ = app.update_text_search(Event::Loaded(true, 2, Results::default()));
    assert!(app.text_search.project.is_none());
}
#[test]
fn replace_all_uses_one_undo_step() {
    let mut document = crate::domain::editor::Document::new("кот кот".into());
    let pattern = text_search::compile("кот", &Options::default()).unwrap();
    assert!(document.edit(pattern.replace_all(&document.text, "пёс").into_owned()));
    assert_eq!(document.text.as_str(), "пёс пёс");
    document.undo(false);
    assert_eq!(document.text.as_str(), "кот кот");
}

fn editor_app(language: Language) -> App {
    let mut app = App::new(Rc::new(I18n::new(language)));
    app.workspace
        .create_space("Search".into(), vec![PathBuf::from("project")]);
    app.loaded = true;
    let _ = app.open_file(PathBuf::from("project/file.txt"));
    let id = app.preview().id;
    let _ = app.finish_preview(
        id,
        Ok(crate::ui::code::Document::new("кот кот\n".into(), "txt")),
    );
    app
}
#[tokio::test]
async fn replacement_updates_editor_and_undo_restores_original() {
    let mut app = editor_app(Language::English);
    let _ = app.update_text_search(Event::Open(false));
    let _ = app.update_text_search(Event::Query(false, "кот".into()));
    let _ = app.update_text_search(Event::Replacement("пёс".into()));
    let apply = app.update_text_search(Event::Replace(true));
    let mut actions = iced_test::runtime::task::into_stream(apply).unwrap();
    use iced::futures::StreamExt;
    while let Some(iced_test::runtime::Action::Output(message)) = actions.next().await {
        let _ = app.update(message);
    }
    assert_eq!(app.editor_buffer().unwrap().content.text(), "пёс пёс\n");
    assert!(app.editor_buffer().unwrap().document.dirty());
    let id = app.preview().id;
    let _ = app.update_editor(editor::Event::Undo(id, false));
    assert_eq!(app.editor_buffer().unwrap().content.text(), "кот кот\n");
}
#[test]
fn unicode_match_navigation_selects_exact_text() {
    let mut app = editor_app(Language::English);
    let _ = app.update_text_search(Event::Open(false));
    let found = text_search::matches(
        "кот кот\n",
        &text_search::compile("кот", &Options::default()).unwrap(),
        10,
    );
    app.text_search.file.as_mut().unwrap().results.hits = found
        .into_iter()
        .map(|found| crate::infrastructure::text_search::Hit {
            path: PathBuf::new(),
            found,
        })
        .collect();
    let _ = app.update_text_search(Event::Move(false, true));
    assert_eq!(
        app.editor_buffer().unwrap().content.selection().as_deref(),
        Some("кот")
    );
    assert_eq!(
        app.editor_buffer()
            .unwrap()
            .content
            .cursor()
            .position
            .column,
        13
    );
}
#[test]
fn translated_search_panels_fit_the_workspace() {
    for language in [Language::English, Language::Russian, Language::Spanish] {
        let mut app = editor_app(language);
        let _ = app.update_text_search(Event::Open(false));
        let mut ui =
            iced_test::Simulator::with_size(iced::Settings::default(), (1440., 920.), app.view());
        assert!(ui.find(iced::widget::Id::from("file-search")).is_ok());
        drop(ui);
        let _ = app.update_text_search(Event::Open(true));
        let mut ui =
            iced_test::Simulator::with_size(iced::Settings::default(), (1440., 920.), app.view());
        assert!(ui.find(iced::widget::Id::from("project-search")).is_ok());
        assert!(ui.find(app.i18n.text(Key::FindEmpty)).is_ok());
        assert!(ui.find(app.i18n.text(Key::FindNoResults)).is_err());
    }
}

#[test]
fn opening_file_search_again_preserves_query_and_pending_search() {
    let mut app = editor_app(Language::English);
    let _ = app.update_text_search(Event::Open(false));
    let _ = app.update_text_search(Event::Query(false, "кот".into()));
    let serial = app.text_search.serial;
    let _ = app.update_text_search(Event::Open(false));
    assert_eq!(app.text_search.serial, serial);
    assert_eq!(app.file_search().unwrap().query, "кот");
}

#[tokio::test]
async fn replacing_an_obsolete_snapshot_preserves_newer_typing() {
    use iced::futures::StreamExt;
    let mut app = editor_app(Language::English);
    let _ = app.update_text_search(Event::Open(false));
    let _ = app.update_text_search(Event::Query(false, "кот".into()));
    let pending = app.update_text_search(Event::Replace(true));
    let tab = app.preview().id;
    let _ = app.update_editor(editor::Event::Replace(tab, "new draft".into()));
    let mut actions = iced_test::runtime::task::into_stream(pending).unwrap();
    while let Some(iced_test::runtime::Action::Output(message)) = actions.next().await {
        let _ = app.update(message);
    }
    assert_eq!(
        app.editor_buffer().unwrap().document.text.as_str(),
        "new draft"
    );
}
