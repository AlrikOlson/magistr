//! The entry point's order is the acceptance: engines before the terminal,
//! the failure on stdout in between, the summary after the run.

const SOURCE: &str = include_str!("cockpit.rs");

fn position(needle: &str) -> usize {
    SOURCE
        .find(needle)
        .unwrap_or_else(|| panic!("cockpit.rs mentions {needle:?}"))
}

#[test]
fn the_engines_are_observed_before_the_terminal_and_a_failure_is_printed_between() {
    // A viewer observes the store: it never admits itself as the host.
    assert!(!SOURCE.contains("EngineClient::open("));
    let engines = position("EngineClient::observe(");
    let report = position("println!(");
    let terminal = position("RealTerminal::stdio()");
    assert!(
        engines < report,
        "the store failure is reported after the open"
    );
    assert!(report < terminal, "…and before the terminal is touched");
}

#[test]
fn the_theme_is_read_from_the_environment_before_raw_mode_and_the_summary_printed_after() {
    let theme = position("Theme::from_os()");
    let terminal = position("RealTerminal::stdio()");
    let run = position(".run(");
    let summary = position("Summary::of(");
    assert!(theme < terminal);
    assert!(terminal < run);
    assert!(run < summary);
    let print = SOURCE[run..]
        .find("println!")
        .map(|at| run + at)
        .expect("the summary goes to the scrollback");
    assert!(
        print < summary && summary - print < 40,
        "the summary is what that println! prints"
    );
}
