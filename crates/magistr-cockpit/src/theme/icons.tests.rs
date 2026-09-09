use super::*;
use crate::render::width;

#[test]
fn every_glyph_in_both_sets_is_one_cell() {
    for set in [Icons::UNICODE, Icons::NERD] {
        for (name, glyph) in set.entries() {
            assert_eq!(width(glyph), 1, "{name}: {glyph:?}");
        }
    }
}

#[test]
fn no_glyph_is_empty() {
    for set in [Icons::UNICODE, Icons::NERD] {
        for (name, glyph) in set.entries() {
            assert!(!glyph.is_empty(), "{name} has no glyph");
        }
    }
}

#[test]
fn the_nerd_set_covers_the_same_vocabulary() {
    let unicode = Icons::UNICODE.entries().map(|(name, _)| name);
    let nerd = Icons::NERD.entries().map(|(name, _)| name);
    assert_eq!(unicode, nerd);
}

#[test]
fn the_nerd_set_differs_wherever_it_is_iconography() {
    for ((name, plain), (_, nerd)) in Icons::UNICODE
        .entries()
        .into_iter()
        .zip(Icons::NERD.entries())
    {
        if Icons::SHARED.contains(&plain) {
            assert_eq!(plain, nerd, "{name} is typography, shared by both sets");
        } else {
            assert_ne!(plain, nerd, "{name} is iconography, so the sets differ");
        }
    }
}

#[test]
fn the_nerd_glyphs_live_in_the_private_use_area() {
    for (name, glyph) in Icons::NERD.entries() {
        if Icons::SHARED.contains(&glyph) {
            continue;
        }
        let ch = glyph.chars().next().expect("a glyph");
        assert!(
            ('\u{e000}'..='\u{f8ff}').contains(&ch),
            "{name}: {ch:?} is not a Nerd Font codepoint"
        );
    }
}

#[test]
fn the_unicode_set_stays_out_of_the_private_use_area() {
    for (name, glyph) in Icons::UNICODE.entries() {
        for ch in glyph.chars() {
            assert!(
                !('\u{e000}'..='\u{f8ff}').contains(&ch),
                "{name}: {ch:?} needs a patched font"
            );
        }
    }
}

#[test]
fn check_fold_and_gate_read_from_the_set_in_force() {
    let nerd = Icons::NERD;
    assert_eq!(nerd.check(true), nerd.done);
    assert_eq!(nerd.check(false), nerd.failed);
    assert_eq!(nerd.fold(true), nerd.expanded);
    assert_eq!(nerd.fold(false), nerd.collapsed);
    assert_eq!(nerd.gate(None), nerd.waiting);
    assert_eq!(nerd.gate(Some(true)), nerd.done);
    assert_eq!(nerd.gate(Some(false)), nerd.failed);
}

#[test]
fn the_default_set_is_the_one_every_font_draws() {
    assert_eq!(Icons::default(), Icons::UNICODE);
}

#[test]
fn the_spinner_turns_a_quarter_at_a_time_and_wraps() {
    let icons = Icons::UNICODE;
    let frames: Vec<&str> = (0..4).map(|step| icons.spin(step)).collect();
    assert_eq!(frames, Icons::SPINNER);
    assert_eq!(icons.spin(4), icons.spin(0), "four steps is one turn");
    assert_eq!(icons.spin(u64::MAX), icons.spin(3));
    // Four distinct marks, or the spinner would stall for a frame.
    let mut distinct = Icons::SPINNER.to_vec();
    distinct.sort_unstable();
    distinct.dedup();
    assert_eq!(distinct.len(), 4);
    // Both sets turn: the rotation has no Nerd Font equivalent.
    assert_eq!(Icons::NERD.spinner, Icons::UNICODE.spinner);
}

/// the cell-drawn sparkline's ramp — what the header shows
/// when the terminal has no pixel layer.
#[test]
fn the_spark_ramp_climbs_from_the_lowest_block_to_the_full_one() {
    let icons = Icons::UNICODE;
    assert_eq!(icons.spark(0.0), Icons::SPARK[0]);
    assert_eq!(icons.spark(1.0), Icons::SPARK[7]);
    assert_eq!(icons.spark(0.5), Icons::SPARK[4], "the middle rounds up");
    // Clamped at both ends, and a non-finite sample reads as nothing.
    assert_eq!(icons.spark(-3.0), Icons::SPARK[0]);
    assert_eq!(icons.spark(9.0), Icons::SPARK[7]);
    assert_eq!(icons.spark(f32::NAN), Icons::SPARK[0]);
    // Monotonic, and both sets draw the same blocks.
    let steps: Vec<&str> = (0..=8).map(|n| icons.spark(n as f32 / 8.0)).collect();
    for pair in steps.windows(2) {
        let [lower, upper] = [pair[0], pair[1]];
        let index = |g: &str| Icons::SPARK.iter().position(|s| *s == g).expect("in ramp");
        assert!(index(lower) <= index(upper), "{lower:?} then {upper:?}");
    }
    assert_eq!(Icons::NERD.spark, Icons::UNICODE.spark);
}
