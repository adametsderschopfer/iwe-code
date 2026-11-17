use super::*;

#[test]
fn recognizes_language_variants_and_configuration_files() {
    for (path, language) in [
        ("src/main.py", Language::Python),
        ("index.d.ts", Language::TypeScript),
        ("index.mjs", Language::JavaScript),
        ("view.tsx", Language::TypeScriptReact),
        ("README.MD", Language::Markdown),
        (".env.production", Language::Env),
        ("compose.yml", Language::Yaml),
        ("Cargo.toml", Language::Toml),
        (".eslintrc", Language::JsonComments),
    ] {
        assert_eq!(Language::for_path(Path::new(path)), Some(language));
        assert!(!language.id().is_empty());
    }
    assert_eq!(Language::for_path(Path::new("picture.png")), None);
}

#[test]
fn resolves_utf16_positions_with_crlf_and_trailing_empty_line() {
    let text = "a🦀б\r\nx\n";
    for (line, character, offset) in [
        (0, 0, 0),
        (0, 1, 1),
        (0, 3, 5),
        (0, 4, 7),
        (1, 0, 9),
        (2, 0, 11),
    ] {
        assert_eq!(
            byte_offset(text, Position { line, character }),
            Some(offset)
        );
    }
    assert_eq!(
        byte_offset(
            text,
            Position {
                line: 0,
                character: 2
            }
        ),
        None
    );
    assert_eq!(
        byte_offset(
            text,
            Position {
                line: 3,
                character: 0
            }
        ),
        None
    );
}

fn edit(start: u32, end: u32, value: &str) -> TextEdit {
    TextEdit {
        range: Range {
            start: Position {
                line: 0,
                character: start,
            },
            end: Position {
                line: 0,
                character: end,
            },
        },
        new_text: value.to_owned(),
    }
}

#[test]
fn applies_completion_and_import_edits_against_the_same_snapshot() {
    assert_eq!(
        apply_edits("foo bar", &[edit(4, 7, "baz"), edit(0, 0, "import x; ")]).as_deref(),
        Some("import x; foo baz")
    );
}

#[test]
fn rejects_overlapping_reversed_and_invalid_edits_atomically() {
    for edits in [
        vec![edit(0, 3, "x"), edit(2, 4, "y")],
        vec![edit(3, 1, "x")],
        vec![edit(0, 99, "x")],
    ] {
        assert_eq!(apply_edits("hello", &edits), None);
    }
}
