use super::*;

#[test]
fn the_bundled_table_answers_first_and_a_retained_claim_answers_after_a_miss() {
    let languages = SourceLanguages::bundled();
    assert_eq!(languages.language("src/main.rs").as_deref(), Some("rust"));
    assert_eq!(languages.language("src/main.gleam"), None);
    assert_eq!(languages.language("Makefile"), None);

    languages.retain(vec![("gleam".into(), vec!["gleam".into()])]);
    assert_eq!(
        languages.language("src/MAIN.GLEAM").as_deref(),
        Some("gleam"),
        "extensions compare case-insensitively like the bundled table"
    );
    assert_eq!(languages.language("src/main.rs").as_deref(), Some("rust"));
    assert_eq!(
        languages.claims(),
        vec![("gleam".to_owned(), vec!["gleam".to_owned()])]
    );
}

#[test]
fn a_claim_never_shadows_a_bundled_extension_and_clones_share_one_table() {
    let languages = SourceLanguages::bundled();
    let consumer = languages.clone();
    languages.retain(vec![("gloom".into(), vec!["rs".into(), "gloom".into()])]);
    assert_eq!(
        consumer.language("lib.rs").as_deref(),
        Some("rust"),
        "the bundled table answers before any claim"
    );
    assert_eq!(consumer.language("lib.gloom").as_deref(), Some("gloom"));
    assert_eq!(
        SourceLanguages::pattern_language("lib.gloom"),
        None,
        "a pack grammar has no pattern language"
    );
    assert_eq!(
        SourceLanguages::pattern_language("lib.rs").as_deref(),
        Some("rust")
    );
    languages.retain(Vec::new());
    assert_eq!(
        consumer.language("lib.gloom"),
        None,
        "a lost registration is unselected through every clone"
    );
}
