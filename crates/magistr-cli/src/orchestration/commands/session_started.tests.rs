use super::*;

/// The label is a clock — two digits, a colon, two digits — and nothing
/// else: the crumb's copy, not a timestamp.
#[test]
fn the_started_label_is_a_wall_clock() {
    let label = session_started_label();
    let (h, m) = label.split_once(':').expect("HH:MM");
    assert_eq!((h.len(), m.len()), (2, 2), "{label}");
    let hour: u32 = h.parse().expect("hours");
    let minute: u32 = m.parse().expect("minutes");
    assert!(hour < 24 && minute < 60, "{label}");
}
