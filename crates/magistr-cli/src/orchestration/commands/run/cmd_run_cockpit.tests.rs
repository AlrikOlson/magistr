//! The entry point's order is the acceptance: engines, then the agent,
//! both failures on stdout, then the terminal, the session, the run, the
//! summary after.

const SOURCE: &str = include_str!("cmd_run_cockpit.rs");

fn position(needle: &str) -> usize {
    SOURCE
        .find(needle)
        .unwrap_or_else(|| panic!("cmd_run_cockpit.rs mentions {needle:?}"))
}

#[test]
fn the_engines_and_the_agent_come_before_the_terminal_and_their_failures_are_printed() {
    let engines = position("EngineClient::open(");
    let agent = position("CockpitSession::start(");
    let terminal = position("RealTerminal::stdio()");
    assert!(
        engines < agent,
        "the store is checked before an agent is spawned"
    );
    assert!(agent < terminal, "the agent is up before raw mode");
    let between = &SOURCE[engines..terminal];
    assert_eq!(
        between.matches("println!(").count(),
        2,
        "both failures are printed on a normal stdout"
    );
}

#[test]
fn the_session_is_attached_before_the_run_and_ended_after_it() {
    let commands = position(".commands()");
    let session = position("spawn_session(");
    let run = position(".run(");
    let ended = position("timeout(SESSION_GRACE, &mut session)");
    let summary = position("Summary::of(");
    assert!(commands < session, "the commands are taken for the session");
    assert!(session < run);
    assert!(
        run < ended,
        "the session is waited for once the terminal is back"
    );
    assert!(ended < summary);
}

#[test]
fn the_dispatch_builds_the_cockpit_only_with_no_environment_hatch() {
    let dispatch = include_str!("cmd_run_tui.rs");
    dispatch
        .find("cmd_run_cockpit(")
        .expect("the cockpit is the only path");
    assert!(
        !dispatch.contains("MAGISTR_TUI"),
        "the MAGISTR_TUI=legacy hatch is gone"
    );
    assert!(
        !dispatch.contains("TuiShell::run("),
        "the legacy shell is unreachable from magistr run"
    );
}
