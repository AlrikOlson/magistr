use super::*;

#[test]
fn paths_resolve_to_the_workspace_relative_dialect_and_escapes_are_denied() {
    let root = Path::new("/w/root");
    assert_eq!(relative_to(root, "/w/root/src/a.rs").unwrap(), "src/a.rs");
    assert!(matches!(
        relative_to(root, "/w/other/a.rs"),
        Err(EffectError::Denied(_))
    ));
    assert!(matches!(
        relative_to(root, "/w/root"),
        Err(EffectError::InvalidRequest(_))
    ));
}
