use super::*;

const THEME: &[u8] = br##"{
    "version": 1,
    "id": "ocean",
    "name": "Ocean",
    "dark": { "colors": { "background": "#141820" } },
    "light": { "colors": { "background": "#f4f7fb" } }
}"##;

#[test]
fn installed_theme_is_a_validated_copy_that_loads_after_source_removal() {
    let folder = tempfile::tempdir().unwrap();
    let source = folder.path().join("source.json");
    let managed = folder.path().join("themes");
    fs::write(&source, THEME).unwrap();

    assert_eq!(install(&source, &managed).unwrap().id, "ocean");
    fs::remove_file(&source).unwrap();
    assert_eq!(fs::read(managed.join("ocean.json")).unwrap(), THEME);
    let definitions = load_from(&managed);
    assert_eq!(definitions.len(), 1);
    assert_eq!(definitions[0].name, "Ocean");
}

#[test]
fn bad_replacements_keep_the_previously_installed_theme() {
    let folder = tempfile::tempdir().unwrap();
    let source = folder.path().join("source.json");
    let managed = folder.path().join("themes");
    fs::write(&source, THEME).unwrap();
    install(&source, &managed).unwrap();

    for change in [
        (r#""version": 1"#, r#""version": 2"#),
        (r#""id": "ocean""#, r#""id": "../ocean""#),
        (r#""id": "ocean""#, r#""id": "iwe""#),
        (r#""id": "ocean""#, r#""id": "iwe-islands""#),
        ("#141820", "#14182088"),
        (r#""background""#, r#""background-typo""#),
        (
            r#""colors": { "background": "#,
            r#""icons": { "folder": { "path": "#,
        ),
    ] {
        let invalid = std::str::from_utf8(THEME)
            .unwrap()
            .replacen(change.0, change.1, 1);
        fs::write(&source, invalid).unwrap();
        assert_eq!(install(&source, &managed).unwrap_err(), Error::Invalid);
        assert_eq!(fs::read(managed.join("ocean.json")).unwrap(), THEME);
    }

    fs::write(&source, vec![b' '; MAX_BYTES as usize + 1]).unwrap();
    assert_eq!(install(&source, &managed).unwrap_err(), Error::Invalid);
}

#[test]
fn a_valid_replacement_updates_only_the_matching_theme() {
    let folder = tempfile::tempdir().unwrap();
    let source = folder.path().join("source.json");
    let managed = folder.path().join("themes");
    fs::write(&source, THEME).unwrap();
    install(&source, &managed).unwrap();
    let text = std::str::from_utf8(THEME).unwrap();
    fs::write(
        &source,
        text.replace("ocean", "cloud").replace("Ocean", "Cloud"),
    )
    .unwrap();
    install(&source, &managed).unwrap();

    fs::write(&source, text.replace("Ocean", "Ocean Updated")).unwrap();
    install(&source, &managed).unwrap();
    let names: std::collections::BTreeSet<_> = load_from(&managed)
        .into_iter()
        .map(|theme| theme.name)
        .collect();
    assert_eq!(
        names,
        ["Cloud".to_owned(), "Ocean Updated".to_owned()].into()
    );
}
