use super::*;

#[test]
fn default_has_everything_off() {
    let env = MagistrEnv::default();
    assert!(!env.reduce_motion);
    assert!(!env.no_animations);
    assert!(!env.no_mouse);
    assert!(!env.accessible);
    assert!(!env.high_contrast);
    assert!(!env.continue_session);
    assert!(!env.no_autoformat);
    assert!(!env.export_summary);
    assert!(env.model.is_none());
    assert!(env.theme.is_none());
    assert!(env.budget.is_none());
}

#[test]
fn instant_scroll_is_case_insensitive() {
    let mut env = MagistrEnv::default();
    assert!(!env.instant_scroll());

    env.scroll = Some("instant".into());
    assert!(env.instant_scroll());

    env.scroll = Some("INSTANT".into());
    assert!(env.instant_scroll());

    env.scroll = Some("smooth".into());
    assert!(!env.instant_scroll());
}
