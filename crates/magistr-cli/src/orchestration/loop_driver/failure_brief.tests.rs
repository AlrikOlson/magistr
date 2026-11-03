use super::*;

use std::time::Duration;

use magistr_core::{CheckFailureRow, CheckRowSeverity};

use crate::utils::check::CheckVerdict;

fn row(message: &str, file: Option<&str>, line: Option<u32>) -> CheckFailureRow {
    CheckFailureRow {
        severity: CheckRowSeverity::Error,
        code: None,
        message: message.to_owned(),
        file: file.map(str::to_owned),
        line,
        column: None,
    }
}

fn check(name: &str, exit_code: i32, rows: Vec<CheckFailureRow>, output: &str) -> CheckRun {
    CheckRun {
        name: name.to_owned(),
        command: "cargo clippy --workspace".to_owned(),
        required: true,
        verdict: CheckVerdict::Failed,
        exit_code,
        output: output.to_owned(),
        rows,
        queue_duration: std::time::Duration::ZERO,
        duration: Duration::from_secs(3),
    }
}

#[test]
fn the_brief_carries_the_rows_the_check_named_with_their_locations() {
    let brief = failure_brief(&[&check(
        "lint",
        101,
        vec![
            row("mismatched types", Some("src/a.rs"), Some(3)),
            row("unused variable `x`", Some("src/b.rs"), Some(9)),
        ],
        "",
    )]);
    assert!(brief.contains("`lint`"), "{brief}");
    assert!(brief.contains("exit 101"), "{brief}");
    assert!(brief.contains("cargo clippy --workspace"), "{brief}");
    assert!(brief.contains("`src/a.rs:3` — mismatched types"), "{brief}");
    assert!(
        brief.contains("`src/b.rs:9` — unused variable `x`"),
        "{brief}"
    );
}

#[test]
fn a_row_with_no_file_is_still_named() {
    let brief = failure_brief(&[&check(
        "test",
        100,
        vec![row("a::b panicked", None, None)],
        "",
    )]);
    assert!(brief.contains("- a::b panicked"), "{brief}");
}

#[test]
fn the_row_list_is_bounded_and_says_how_many_it_dropped() {
    let rows: Vec<CheckFailureRow> = (0..MAX_ROWS + 7)
        .map(|i| row(&format!("failure {i}"), Some("src/x.rs"), Some(1)))
        .collect();
    let brief = failure_brief(&[&check("check", 101, rows, "")]);
    assert!(brief.contains("failure 0"), "{brief}");
    assert!(
        brief.contains(&format!("failure {}", MAX_ROWS - 1)),
        "{brief}"
    );
    assert!(!brief.contains(&format!("failure {MAX_ROWS}")), "{brief}");
    assert!(brief.contains("and 7 more"), "{brief}");
}

#[test]
fn a_check_whose_output_parsed_no_rows_falls_back_to_its_tail_and_says_so() {
    let noise = "x".repeat(FALLBACK_TAIL_BYTES * 2);
    let output = format!("{noise}\nthe last thing it said");
    let brief = failure_brief(&[&check("structure", 1, Vec::new(), &output)]);
    assert!(brief.contains("named no failures"), "{brief}");
    assert!(brief.contains("the last thing it said"), "{brief}");
    assert!(
        brief.len() < FALLBACK_TAIL_BYTES + 800,
        "the whole output did not go in: {} bytes",
        brief.len()
    );
}

#[test]
fn several_red_checks_each_get_their_own_section() {
    let brief = failure_brief(&[
        &check(
            "check",
            101,
            vec![row("boom", Some("src/a.rs"), Some(1))],
            "",
        ),
        &check("lint", 3, vec![row("bang", Some("src/b.rs"), Some(2))], ""),
    ]);
    assert!(
        brief.contains("`check`") && brief.contains("`lint`"),
        "{brief}"
    );
    assert!(brief.contains("boom") && brief.contains("bang"), "{brief}");
}

#[test]
fn a_timed_out_check_says_it_timed_out_rather_than_failed() {
    let mut timed = check(
        "test",
        -1,
        Vec::new(),
        "still running when the clock ran out",
    );
    timed.verdict = CheckVerdict::TimedOut;
    let brief = failure_brief(&[&timed]);
    assert!(brief.contains("timed out"), "{brief}");
}
