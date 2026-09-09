use magistr_cockpit::theme::Severity;

use super::*;
use crate::medium::Medium;

/// Every signal band's ground is the volume's own 9% oklab mix over
/// the mantle — closer to the mantle than to the signal, tinted toward
/// it — and the cap is the state code at full strength.
#[test]
fn a_signal_band_tints_the_mantle_and_caps_at_full_strength() {
    for medium in Medium::ALL {
        let lex = Lexicon::of(medium);
        for (severity, signal) in [
            (Severity::Alert, lex.red),
            (Severity::Attention, lex.peach),
            (Severity::Good, lex.green),
        ] {
            let ink = SeverityInk::of(severity, &lex);
            assert_eq!(ink.cap, signal, "{severity:?} caps in its own hue");
            let dist = |a: qs_gpu::color::Srgba, b: qs_gpu::color::Srgba| {
                (a.r - b.r).abs() + (a.g - b.g).abs() + (a.b - b.b).abs()
            };
            assert!(
                dist(ink.ground, lex.mantle) < dist(ink.ground, signal),
                "{severity:?}'s ground stays a ground, not a signal"
            );
            assert_ne!(ink.ground, lex.mantle, "{severity:?}'s ground is tinted");
        }
    }
}

/// Quiet is a measurement, not a signal: untinted mantle, faint ink.
#[test]
fn quiet_is_untinted() {
    let lex = Lexicon::of(Medium::Night);
    let ink = SeverityInk::of(Severity::Quiet, &lex);
    assert_eq!(ink.ground, lex.mantle);
    assert_eq!(ink.cap, lex.ov0);
}

/// The pair is the whole vocabulary: the type has no third field a
/// left-border strip could ride in on, and the ground of every band is
/// a full-card color, never a stripe width. (The structural probe over
/// the DrawList lives in the parity suite; this pins the type.)
#[test]
fn the_pair_is_ground_and_cap_and_nothing_else() {
    let lex = Lexicon::of(Medium::Day);
    let SeverityInk { ground, cap } = SeverityInk::of(Severity::Alert, &lex);
    assert_ne!(ground, cap, "low saturation and full strength differ");
}
