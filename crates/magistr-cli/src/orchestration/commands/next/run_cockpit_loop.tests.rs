//! The entry point's order is the acceptance: the engines (native only)
//! before the terminal with their failure printed, the driver built on
//! the runtime's commands and spawned before the run, the feed tapped for
//! it, the report after the summary; both `next` dispatches default to
//! the cockpit with the legacy shell behind the hatch.

const SOURCE: &str = include_str!("run_cockpit_loop.rs");

fn position(needle: &str) -> usize {
    SOURCE
        .find(needle)
        .unwrap_or_else(|| panic!("run_cockpit_loop.rs mentions {needle:?}"))
}

#[test]
fn the_loops_admitted_writer_is_attached_before_the_terminal_and_never_re_admitted() {
    let attach = position("EngineClient::attach");
    let terminal = position("RealTerminal::stdio()");
    assert!(
        attach < terminal,
        "the cockpit attaches to the loop's writer before raw mode"
    );
    assert!(
        !SOURCE.contains("EngineClient::open("),
        "a second host admission on the loop's project would refuse itself"
    );
    assert!(
        SOURCE.contains("iter_loop.native().cloned().map(EngineClient::attach)"),
        "the one writer is the loop's own admitted engines"
    );
}

#[test]
fn the_driver_is_spawned_before_the_run_on_a_tapped_feed_and_reported_after() {
    let commands = position(".commands()");
    let driver = position("LoopDriver::new(");
    let live = position("Connector::live(source)");
    let tap = position(".tap(driver.tap())");
    let spawn = position("driver.spawn()");
    let run = position(".run(");
    let grace = position("timeout(LOOP_GRACE, looping)");
    let summary = position("Summary::of(");
    let report = position("println!(\"{report}\")");
    assert!(commands < driver, "the commands are taken for the driver");
    assert!(
        driver < live && live < tap,
        "the live connector, the feed tapped for the driver"
    );
    assert!(
        tap < spawn && spawn < run,
        "the driver runs beside the cockpit"
    );
    assert!(
        run < grace,
        "the driver is waited for once the terminal is back"
    );
    assert!(grace < summary && summary < report);
}

#[test]
fn the_judge_holds_no_source_write_authority() {
    let judge = position("fn judge(");
    let refused = position("Connector::live(SourceRoute::Refused(SourceRefusal::NoAuthority))");
    assert!(
        judge < refused,
        "the judge's sessions are composed with the no-authority route, never the loop's writer"
    );
    assert_eq!(
        SOURCE.matches("Connector::live(").count(),
        2,
        "exactly two live connectors: the loop's routed one and the judge's refused one"
    );
}

#[test]
fn explicit_next_loop_entrypoints_start_a_new_request_after_settled_work() {
    for source in [
        include_str!("cmd_next_loop.rs"),
        include_str!("cmd_next_loop_cockpit.rs"),
        include_str!("cmd_next_loop_headless.rs"),
    ] {
        assert!(source.contains("IterationLoop::start_or_recover("));
        assert!(!source.contains("IterationLoop::open_or_recover("));
    }
}

#[test]
fn both_dispatches_build_the_cockpit_only_with_no_environment_hatch() {
    let module = include_str!("mod.rs");
    assert!(
        !module.contains("MAGISTR_TUI"),
        "the MAGISTR_TUI=legacy hatch is gone from the next dispatches"
    );
    for (name, dispatch, cockpit) in [
        (
            "cmd_next_tui.rs",
            include_str!("cmd_next_tui.rs"),
            "cmd_next_cockpit(",
        ),
        (
            "cmd_next_loop_tui.rs",
            include_str!("cmd_next_loop_tui.rs"),
            "cmd_next_loop_cockpit(",
        ),
    ] {
        dispatch
            .find(cockpit)
            .unwrap_or_else(|| panic!("{name} dispatches to the cockpit"));
        assert!(
            !dispatch.contains("TuiShell::run("),
            "{name}: the legacy shell is unreachable"
        );
        assert!(
            !dispatch.contains("legacy_tui_requested"),
            "{name}: no hatch read remains"
        );
    }
}
