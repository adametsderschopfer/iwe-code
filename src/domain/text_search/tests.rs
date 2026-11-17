use super::*;
#[test]
fn unicode_words_keep_byte_ranges_and_character_columns() {
    let text = "А кот\nКОТ коты";
    let pattern = compile(
        "кот",
        &Options {
            word: true,
            ..Options::default()
        },
    )
    .unwrap();
    let found = matches(text, &pattern, 100);
    assert_eq!(found.len(), 2);
    assert_eq!((found[0].line, found[0].column), (1, 2));
    assert_eq!((found[1].line, found[1].column), (2, 0));
    assert_eq!(&text[found[1].range.clone()], "КОТ");
}
#[test]
fn invalid_regex_and_zero_width_matches_are_bounded() {
    let options = Options {
        regex: true,
        ..Options::default()
    };
    assert!(compile("[", &options).is_err());
    assert_eq!(
        matches("abc", &compile("^", &options).unwrap(), 10).len(),
        1
    );
}
