use super::*;

#[test]
fn marks_replacements_additions_and_eof_deletions() {
    let hunks = compare("a\nb\nc\nd", "x\nb\nc");
    assert_eq!(hunks[0].old, 0..1);
    assert_eq!(hunks[0].new, 0..1);
    assert_eq!(at(&hunks, 1), None);
    assert_eq!(at(&hunks, 2), Some(1));
    let added = compare("", "x\ny");
    assert!(added[0].old.is_empty());
    assert_eq!(added[0].rows, 0..2);
    let removed = compare("x\ny", "");
    assert_eq!(removed[0].rows, 0..1);
    assert!(removed[0].new.is_empty());
}
