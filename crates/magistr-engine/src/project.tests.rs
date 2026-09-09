use super::*;

#[test]
fn engine_export_is_the_domain_context() {
    let domain_context = magistr_domain::ProjectContext::new(
        vec!["abc1234 feat: add widget".into()],
        20,
        12,
        vec!["src/lib.rs".into()],
        None,
        None,
    );
    let engine_context: crate::ProjectContext = domain_context.clone();
    let module_context: ProjectContext = engine_context;
    assert_eq!(module_context, domain_context);
}
