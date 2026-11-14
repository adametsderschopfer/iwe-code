use super::{Link, Request};
use std::path::{Path, PathBuf};

/// A path prefix that stays absolute on every platform.
#[cfg(windows)]
const ROOT: &str = "C:/notes-work";
#[cfg(not(windows))]
const ROOT: &str = "/notes-work";

fn open_link(path: &str) -> String {
    format!("iwe://open?path={path}")
}

fn project_link(path: &str) -> String {
    format!("iwe://project?path={path}")
}

#[test]
fn parse_open_link() {
    let path = format!("{ROOT}/ideas.md");
    let link = Link::parse(&open_link(&path)).expect("valid link");
    assert_eq!(link, Link::Open(PathBuf::from(path)));
}

#[test]
fn parse_project_link() {
    let path = format!("{ROOT}/work/site");
    let link = Link::parse(&project_link(&path)).expect("valid link");
    assert_eq!(link, Link::Project(PathBuf::from(path)));
}

#[test]
fn parse_link_percent_decodes_the_path() {
    let link = Link::parse(&open_link(&format!("{ROOT}/my%20file.txt"))).expect("valid link");
    assert_eq!(
        link,
        Link::Open(PathBuf::from(format!("{ROOT}/my file.txt")))
    );
}

#[test]
fn parse_link_accepts_action_case_insensitively() {
    let path = format!("{ROOT}/a.txt");
    let link = Link::parse(&format!("iwe://OPEN?path={path}")).expect("valid link");
    assert_eq!(link, Link::Open(PathBuf::from(path)));
}

#[test]
fn reject_other_schemes() {
    assert_eq!(Link::parse("https://open?path=/notes/a.txt"), None);
    assert_eq!(Link::parse("iwas://open?path=/notes/a.txt"), None);
}

#[test]
fn reject_unknown_actions() {
    assert_eq!(Link::parse("iwe://close?path=/notes/a.txt"), None);
    assert_eq!(Link::parse("iwe:///notes/a.txt"), None);
}

#[test]
fn reject_missing_or_relative_paths() {
    assert_eq!(Link::parse("iwe://open"), None);
    assert_eq!(Link::parse("iwe://open?target=/notes/a.txt"), None);
    assert_eq!(Link::parse(&open_link("notes/a.txt")), None);
    assert_eq!(Link::parse(&project_link("")), None);
}

#[test]
fn parse_broken_links() {
    assert_eq!(Link::parse(""), None);
    assert_eq!(Link::parse("iwe://open?path="), None);
    assert_eq!(Link::parse("not a link"), None);
}

#[test]
fn round_trip_request_through_json() {
    let file = Request::File {
        path: PathBuf::from("/notes/ideas.md"),
    };
    let project = Request::Project {
        directory: PathBuf::from("/work/site"),
    };
    let focus = Request::Focus;
    for request in [file, project, focus] {
        let text = serde_json::to_string(&request).expect("serialize");
        let parsed: Request = serde_json::from_str(&text).expect("deserialize");
        assert_eq!(parsed, request);
    }
}

#[test]
fn focus_request_uses_its_own_tag() {
    let text = serde_json::to_string(&Request::Focus).expect("serialize");
    assert_eq!(text, r#"{"type":"focus"}"#);
}

#[test]
fn open_text_produces_a_parsed_link() {
    let path = format!("{ROOT}/ideas.md");
    let text = Link::open_text(Path::new(&path));
    assert_eq!(Link::parse(&text), Some(Link::Open(PathBuf::from(path))));
}
