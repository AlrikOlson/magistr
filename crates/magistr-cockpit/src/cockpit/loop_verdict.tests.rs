use super::*;

fn verdict(ok: bool, impossible: bool, reason: &str) -> LoopVerdict {
    LoopVerdict {
        ok,
        impossible,
        reason: reason.to_owned(),
    }
}

#[test]
fn the_three_shapes_have_a_word_and_a_glyph() {
    let icons = &Icons::UNICODE;
    let met = verdict(true, false, "all four criteria hold");
    assert_eq!(met.word(), "met");
    assert_eq!(met.glyph(icons), icons.gate(Some(true)));
    let not_yet = verdict(false, false, "no test for the timeout");
    assert_eq!(not_yet.word(), "not yet");
    assert_eq!(not_yet.glyph(icons), icons.gate(Some(false)));
    let stuck = verdict(false, true, "the acceptance contradicts itself");
    assert_eq!(stuck.word(), "impossible");
    assert_eq!(stuck.glyph(icons), icons.gate(Some(false)));
    // `met` and `impossible` together is read as not ok, as Verdict does.
    assert_eq!(verdict(true, true, "").word(), "impossible");
    assert_eq!(
        verdict(true, true, "").glyph(icons),
        icons.gate(Some(false))
    );
}

#[test]
fn the_text_is_the_word_and_the_reason_or_the_word_alone() {
    assert_eq!(
        verdict(false, false, "  no test for the timeout ").text(),
        "not yet — no test for the timeout"
    );
    assert_eq!(verdict(true, false, "   ").text(), "met");
}
