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

#[test]
fn audit_dense_source_line() {
    if std::env::var_os("IWE_PROFILE_WORKSPACE").is_none() {
        return;
    }
    let text = "я".repeat(240_000) + &"éx".repeat(10_000);
    let pattern = compile("x", &Options::default()).unwrap();
    for _ in 0..3 {
        let start = std::time::Instant::now();
        let hits = matches(&text, &pattern, 10_000);
        eprintln!("dense text search {:?}", start.elapsed());
        assert_eq!(hits.len(), 10_000);
        assert_eq!(hits.last().unwrap().end_column, 260_000);
    }
}

#[test]
fn snippets_share_storage_and_keep_unicode_columns_across_multiline_matches() {
    let text = "αxxx\r\nββ\nx";
    let pattern = Regex::new("x|ββ\\nx").unwrap();
    let hits = matches(text, &pattern, 10);
    assert_eq!(hits.len(), 4);
    assert!(Arc::ptr_eq(&hits[0].text, &hits[2].text));
    assert_eq!(hits[2].byte_column, 4);
    assert_eq!(hits[2].column, 3);
    assert_eq!(
        (hits[3].line, hits[3].end_column, hits[3].end_byte_column),
        (2, 2, 4)
    );
    let zero = matches("α\n", &Regex::new("(?m)$").unwrap(), 2);
    assert_eq!((zero[1].line, zero[1].column), (2, 0));
}

#[test]
fn replacements_bound_expanded_captures_and_preserve_regex_context() {
    let pattern = Regex::new(r"\b(?P<word>é)\b").unwrap();
    let text = "é é";
    let template = "$$${word}$0$missing";
    let expected = pattern.replace_all(text, template).into_owned();
    assert_eq!(
        replace(text, &pattern, template, true, None, expected.len()),
        Ok(Some(expected.clone()))
    );
    assert_eq!(
        replace(text, &pattern, template, true, None, expected.len() - 1),
        Err(super::super::Failure::TooLarge)
    );
    assert_eq!(
        replace(text, &pattern, "$0", false, Some(1), 8),
        Ok(Some("é $0".into()))
    );
    let source = "x".repeat(1024);
    assert_eq!(
        replace(
            &source,
            &Regex::new("(x+)").unwrap(),
            &"$1".repeat(1000),
            true,
            None,
            2048
        ),
        Err(super::super::Failure::TooLarge)
    );
}
