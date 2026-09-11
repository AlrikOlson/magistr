use super::*;

fn lines(parts: &[&str]) -> Vec<String> {
    parts.iter().map(|part| (*part).to_owned()).collect()
}

#[test]
fn the_pre_1_73_panic_line_shape_is_still_read() {
    let details = parse_panic_block(&lines(&[
        "thread 'tests::old' panicked at 'boom', src/lib.rs:3:5",
        "note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace",
    ]));
    assert_eq!(details.assertion, "boom");
    assert_eq!(details.file.as_deref(), Some("src/lib.rs"));
    assert_eq!(details.trace, "src/lib.rs:3:5");
}

#[test]
fn a_block_without_a_panic_line_keeps_the_pre_013_shape() {
    let details = parse_panic_block(&lines(&["", "note: test did not panic as expected", ""]));
    assert_eq!(details.assertion, "");
    assert_eq!(details.file, None);
    assert_eq!(details.trace, "note: test did not panic as expected");
}

#[test]
fn the_modern_shape_reads_the_message_from_the_next_line_and_keeps_continuations() {
    let details = parse_panic_block(&lines(&[
        "thread 'tests::fails_eq' panicked at crates\\alpha\\src\\lib.rs:9:21:",
        "assertion `left == right` failed: math is hard",
        "  left: 3",
        " right: 4",
        "note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace",
    ]));
    assert_eq!(
        details.assertion,
        "assertion `left == right` failed: math is hard"
    );
    assert_eq!(details.file.as_deref(), Some("crates/alpha/src/lib.rs"));
    assert_eq!(
        details.trace,
        "crates/alpha/src/lib.rs:9:21\nleft: 3\nright: 4"
    );
}

#[test]
fn trim_backtrace_pairs_symbol_and_location_lines_and_keeps_only_workspace_paths() {
    let kept = trim_backtrace(&lines(&[
        "   0: std::panicking::panic_handler",
        "             at /rustc/abc/library\\std\\src\\panicking.rs:689",
        "   2: alpha::tests::fails_panic",
        "             at .\\src\\lib.rs:11",
        "   4: core::ops::function::FnOnce::call_once",
        "             at C:\\Users\\dev\\.rustup\\toolchains\\stable\\lib\\rustlib\\src\\rust\\library\\core\\src\\ops\\function.rs:250",
        "   5: mycrate::helper",
        "             at C:\\Users\\dev\\.cargo\\registry\\src\\index\\dep-1.0.0\\src\\lib.rs:9",
    ]));
    assert_eq!(
        kept,
        vec!["alpha::tests::fails_panic at ./src/lib.rs:11".to_owned()]
    );
}
