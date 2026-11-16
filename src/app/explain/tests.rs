use super::*;
use crate::domain::repository::project::{BranchKey, CommitKey};

#[test]
fn explanations_use_the_clicked_repository_and_reference() {
    let app = App::new(std::rc::Rc::new(I18n::new(crate::i18n::Language::English)));
    let root = PathBuf::from("/other-repository");
    for (target, value) in [
        (
            Target::Commit(CommitKey {
                root: root.clone(),
                id: "abc123".into(),
            }),
            "abc123",
        ),
        (
            Target::Branch(BranchKey {
                root: root.clone(),
                reference: "refs/remotes/origin/feature".into(),
            }),
            "refs/remotes/origin/feature",
        ),
    ] {
        let request = app.explanation(target, Agent::Codex).unwrap();
        assert_eq!(request.directory.as_ref(), Some(&root));
        assert!(request.prompt.contains(value));
        assert!(request.prompt.contains(root.to_str().unwrap()));
    }
}

#[test]
fn includes_the_clicked_file_and_each_project_in_the_space() {
    let mut app = App::new(std::rc::Rc::new(I18n::new(crate::i18n::Language::English)));
    app.workspace.create_space(
        "Example Space".into(),
        vec!["/workspace/backend".into(), "/workspace/frontend".into()],
    );
    let space = app.explanation(Target::Space(0), Agent::ZCode).unwrap();
    for context in ["Example Space", "/workspace/backend", "/workspace/frontend"] {
        assert!(space.prompt.contains(context));
    }
    let file = app
        .explanation(Target::File("/notes/spec.md".into()), Agent::Claude)
        .unwrap();
    assert_eq!(file.directory, Some("/notes".into()));
    assert!(file.prompt.contains("/notes/spec.md"));
}

#[test]
fn ci_requests_identify_the_run_and_the_selected_job() {
    use crate::domain::{jobs::Job, repository::Pipeline};
    let app = App::new(std::rc::Rc::new(I18n::new(crate::i18n::Language::English)));
    let run = Pipeline {
        id: 42,
        number: Some(6),
        title: "Release build".into(),
        branch: "release".into(),
        commit: Some("abcd1234".into()),
        status: "failed".into(),
        date: String::new(),
        url: "https://example.com/runs/42".into(),
    };
    let request = app
        .explanation(Target::Pipeline(run), Agent::Claude)
        .unwrap();
    for context in ["Release build", "abcd1234", "failed", "/runs/42"] {
        assert!(request.prompt.contains(context));
    }

    let job = Job {
        id: 932,
        name: "Package installer".into(),
        status: "failed".into(),
        url: "https://example.com/jobs/932".into(),
        actions: vec![],
    };
    let request = app
        .explanation(Target::Job(42, job), Agent::DeepSeek)
        .unwrap();
    for context in ["42", "932", "Package installer", "failed", "/jobs/932"] {
        assert!(request.prompt.contains(context));
    }
}

#[test]
fn failed_launch_keeps_the_request_for_installation_or_retry() {
    let mut app = App::new(std::rc::Rc::new(I18n::new(crate::i18n::Language::English)));
    let _ = app.update_explain(Event::Choose(
        Target::File("/workspace/file.rs".into()),
        Agent::ZCode,
    ));
    let id = app.explain.request;
    let _ = app.update_explain(Event::Launched(id, None, false));
    let pending = app.explain.setup.as_ref().unwrap();
    assert_eq!(pending.agent, Agent::ZCode);
    assert!(!pending.busy);
    assert!(pending.prompt.contains("/workspace/file.rs"));

    let _ = app.update_explain(Event::Cancel);
    let _ = app.update_explain(Event::Launched(id, Some("/wrong".into()), true));
    assert!(app.explain.setup.is_none());
    assert!(app.workspace.preferences.assistant_paths.is_empty());
}
