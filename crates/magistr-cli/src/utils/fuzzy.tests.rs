use super::*;

#[test]
fn basic_match() {
    let result = fuzzy_match("Toggle help overlay", "tho");
    assert!(result.is_some());
    let positions = result.unwrap();
    assert_eq!(positions.len(), 3);
    assert_eq!(positions[0], 0); // 't'
}

#[test]
fn no_match() {
    assert!(fuzzy_match("Toggle help", "xyz").is_none());
}

#[test]
fn case_insensitive() {
    assert!(fuzzy_match("Cargo.toml", "cargo").is_some());
    assert!(fuzzy_match("Toggle Help", "th").is_some());
}

#[test]
fn empty_query_matches() {
    let result = fuzzy_match("anything", "");
    assert!(result.is_some());
    assert!(result.unwrap().is_empty());
}

#[test]
fn exact_match() {
    let result = fuzzy_match("Quit", "quit");
    assert!(result.is_some());
    assert_eq!(result.unwrap(), vec![0, 1, 2, 3]);
}

#[test]
fn file_path_match() {
    let result = fuzzy_match("src/main.rs", "main");
    assert!(result.is_some());
}
