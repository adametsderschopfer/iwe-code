use super::*;

#[test]
fn preserves_positions_and_excludes_surrounding_prose() {
    let text = "# Example\n```python\nname = '🦀'\nname.\n```\nprose";
    let block = at(
        text,
        Position {
            line: 3,
            character: 5,
        },
    )
    .unwrap();
    assert_eq!(block.text, "\n\nname = '🦀'\nname.\n\n");
    assert_eq!((block.start, block.end), (2, 4));
    assert_eq!(block.language, Language::Python);
    assert_eq!(block.extension, "py");
    assert!(
        at(
            text,
            Position {
                line: 5,
                character: 0
            }
        )
        .is_none()
    );
}

#[test]
fn respects_fence_length_and_unfinished_blocks() {
    let text = "~~~~ts\nconst x = 1;\n~~~\nx.";
    let block = at(
        text,
        Position {
            line: 3,
            character: 2,
        },
    )
    .unwrap();
    assert_eq!(block.language, Language::TypeScript);
    assert!(block.text.contains("~~~"));
}

#[test]
fn does_not_treat_nested_fences_in_unknown_blocks_as_code() {
    let text = "````unknown\n```python\nx.\n```\n````";
    assert!(
        at(
            text,
            Position {
                line: 2,
                character: 2
            }
        )
        .is_none()
    );
}
