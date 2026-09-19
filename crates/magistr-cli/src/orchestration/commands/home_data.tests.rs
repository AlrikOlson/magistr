use super::*;

#[test]
fn stack_summary_joins_known_parts_and_skips_unknowns() {
    assert_eq!(stack_summary("rust", "none", "cargo"), "rust / cargo");
    assert_eq!(stack_summary("unknown", "none", ""), "not detected");
    assert_eq!(
        stack_summary("typescript", "next.js", "pnpm"),
        "typescript / next.js / pnpm"
    );
}

#[test]
fn a_directory_outside_git_reads_as_such_and_collect_never_errors() {
    let tmp = tempfile::tempdir().expect("tempdir");
    assert_eq!(git_summary(tmp.path()), "not a git repository");
    // No home captured: the session database is unlocated and the card
    // simply lists no sessions.
    let inputs = ProjectInputs::from_lookup(tmp.path(), |_| None);
    let data = collect(tmp.path(), &inputs);
    assert!(data.sessions.is_empty());
    assert!(!data.has_config, "an empty directory has no config");
    assert_eq!(data.stack, "not detected");
    assert_eq!(
        data.project,
        tmp.path().file_name().unwrap().to_string_lossy()
    );
}
