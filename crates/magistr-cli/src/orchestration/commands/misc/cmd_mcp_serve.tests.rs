/// The repair has to actually run, and it has to run *before* the server
/// takes over stdio — after that the process belongs to the client and
/// never returns. Pinned in the source because the seam is a process
/// lifetime: there is no way to observe it from a test that does not become
/// the server. What the repair is worth is measured on
/// `scaffold::repair::repair` itself.
const SOURCE: &str = include_str!("cmd_mcp_serve.rs");

#[test]
fn the_scaffold_is_repaired_before_the_server_takes_stdio() {
    let repair = SOURCE
        .find("scaffold::repair::repair(&project_root)")
        .expect("the scaffold is repaired on the way in");
    let serve = SOURCE
        .find("run_stdio_server(project_root)")
        .expect("and then the server runs");
    assert!(repair < serve, "the repair happens first or not at all");
}

/// It must not be able to stop the thing it runs before: no `?`, no unwrap,
/// no expect on the repair — it answers a list and the list is only logged.
#[test]
fn a_repair_can_never_fail_the_server() {
    let line = SOURCE
        .lines()
        .find(|l| l.contains("scaffold::repair::repair("))
        .expect("the call");
    for fatal in ["?;", "unwrap", "expect"] {
        assert!(!line.contains(fatal), "the repair is best-effort: {line}");
    }
}
