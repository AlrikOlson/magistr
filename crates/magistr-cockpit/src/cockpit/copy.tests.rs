use super::*;

/// Money: the ISO code after the amount in a label, the bare pair in a
/// meter, and cents that round-trip to two places.
#[test]
fn money_is_spelled_the_one_way() {
    assert_eq!(Copy::amount(184), "1.84");
    assert_eq!(Copy::amount(5), "0.05");
    assert_eq!(Copy::amount(1200), "12.00");
    assert_eq!(Copy::usd(207), "2.07 USD");
    assert_eq!(Copy::usd(0), "0.00 USD");
    assert_eq!(Copy::pair(184, 1200), "1.84 / 12.00");
    assert!(!Copy::usd(207).contains('$'));
}

/// A clock is minutes and seconds, two places each, the minutes never
/// rolling into an hour.
#[test]
fn a_clock_is_minutes_and_seconds() {
    assert_eq!(Copy::clock(0), "00:00");
    assert_eq!(Copy::clock(4 * 60 + 12), "04:12");
    assert_eq!(Copy::clock(64 * 60 + 5), "64:05");
}

/// A duration is its coarsest single unit, and never two.
#[test]
fn a_duration_is_one_unit() {
    assert_eq!(Copy::duration(45), "45 s");
    assert_eq!(Copy::duration(60), "1 min");
    assert_eq!(Copy::duration(12 * 60 + 59), "12 min");
    assert_eq!(Copy::duration(4 * 3_600 + 30 * 60), "4 h");
    assert_eq!(Copy::duration(2 * 86_400 + 5 * 3_600), "2 d");
}

/// The metaphor words are found as whole words in either case, and a
/// word that merely contains one is not a metaphor.
#[test]
fn a_metaphor_is_a_whole_word_in_either_case() {
    assert_eq!(
        Copy::metaphor_in("Open a shipped star on the Sky"),
        Some("star")
    );
    assert_eq!(Copy::metaphor_in("The Sky"), Some("sky"));
    assert_eq!(Copy::metaphor_in("Roadmap · 211 / 306"), None);
    assert_eq!(Copy::metaphor_in("skyline starboard"), None);
    assert_eq!(Copy::metaphor_in(""), None);
}

/// A token count in the foot's `left` slot is its nearest thousand
/// with a `k` — `162k` — and under a thousand the count itself.
#[test]
fn a_kilo_count_is_the_nearest_thousand() {
    assert_eq!(Copy::kilo(161_796), "162k");
    assert_eq!(Copy::kilo(200_000), "200k");
    assert_eq!(Copy::kilo(1_499), "1k");
    assert_eq!(Copy::kilo(1_500), "2k");
    assert_eq!(Copy::kilo(999), "999");
    assert_eq!(Copy::kilo(0), "0");
}
