use super::*;

#[test]
fn filter_builder() {
    let f = SessionFilter::new()
        .with_project("magistr")
        .with_limit(50)
        .with_tag("auth");
    assert_eq!(f.project.as_deref(), Some("magistr"));
    assert_eq!(f.limit, 50);
    assert_eq!(f.tag.as_deref(), Some("auth"));
    assert!(f.since.is_none());
}

#[test]
fn filter_default_limit() {
    let f = SessionFilter::new();
    assert_eq!(f.limit, 20);
}
