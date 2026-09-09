use super::*;

// ── format_cost ────────────────────────────────────────────

#[test]
fn format_cost_zero() {
    assert_eq!(format_cost(0.0), "$0.0000");
}

#[test]
fn format_cost_tiny() {
    assert_eq!(format_cost(0.0042), "$0.0042");
    assert_eq!(format_cost(0.005), "$0.0050");
}

#[test]
fn format_cost_normal() {
    assert_eq!(format_cost(1.234), "$1.23");
    assert_eq!(format_cost(99.99), "$99.99");
}

#[test]
fn format_cost_boundary() {
    assert_eq!(format_cost(0.01), "$0.01");
    assert_eq!(format_cost(0.009), "$0.0090");
}
