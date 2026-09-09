use std::path::Path;

use super::*;

#[test]
fn the_title_carries_the_parts_that_are_known() {
    let (title, _) = chrome(Path::new("/home/dev/magistr"), None, None);
    assert_eq!(title, "magistr — magistr");
    let (title, _) = chrome(Path::new("/home/dev/proj"), Some("acp"), None);
    assert_eq!(title, "magistr — proj · acp");
    let (title, _) = chrome(Path::new("proj"), Some("acp"), Some("claude-sonnet-5"));
    assert_eq!(title, "magistr — proj · acp · claude-sonnet-5");
}

#[test]
fn a_rootless_path_falls_back_to_its_display() {
    let (title, _) = chrome(Path::new("/"), None, None);
    assert_eq!(title, "magistr — /");
}
