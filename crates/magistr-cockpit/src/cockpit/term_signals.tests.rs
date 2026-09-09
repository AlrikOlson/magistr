use super::*;

/// A parser over the signals, fed `bytes`.
fn fed(bytes: &[u8]) -> vt100::Parser<TermSignals> {
    let mut parser = vt100::Parser::new_with_callbacks(24, 80, 0, TermSignals::default());
    parser.process(bytes);
    parser
}

#[test]
fn a_progress_report_is_read_in_each_state() {
    for (bytes, want) in [
        (&b"\x1b]9;4;1;40\x07"[..], Progress::Running(40)),
        (b"\x1b]9;4;2;70\x1b\\", Progress::Failed(70)),
        (b"\x1b]9;4;3;0\x07", Progress::Indeterminate),
        (b"\x1b]9;4;4;15\x07", Progress::Running(15)),
        (b"\x1b]9;4;1\x07", Progress::Running(0)),
        (b"\x1b]9;4;1;250\x07", Progress::Running(100)),
    ] {
        assert_eq!(fed(bytes).callbacks().progress(), want, "{bytes:?}");
    }
}

#[test]
fn a_clear_takes_the_bar_down_and_a_bad_report_leaves_it() {
    let parser = fed(b"\x1b]9;4;1;40\x07\x1b]9;4;0;0\x07");
    assert_eq!(parser.callbacks().progress(), Progress::Cleared);
    let parser = fed(b"\x1b]9;4;1;40\x07\x1b]9;4;9;1\x07\x1b]9;4;x\x07\x1b]9;9;1\x07");
    assert_eq!(
        parser.callbacks().progress(),
        Progress::Running(40),
        "an unknown state, a bad state and another OSC 9 all pass"
    );
}

#[test]
fn nothing_reported_is_cleared_and_the_screen_still_paints() {
    let parser = fed(b"hello\x1b]2;title\x07");
    assert_eq!(parser.callbacks().progress(), Progress::Cleared);
    assert_eq!(parser.screen().contents().trim_end(), "hello");
}
