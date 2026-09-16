use super::*;
use crate::cockpit::Widget;

/// A hand over what can be clicked, the terminal's own shape over a row,
/// a panel or nothing at all.
#[test]
fn the_hand_appears_over_buttons_chips_and_links_only() {
    for id in [
        Widget::Accept.id(),
        Widget::TabSession.id(),
        Widget::footer_chip(0),
        Widget::signal_snooze(1),
        Widget::trace_link(4),
        Widget::trace_chunk(4),
    ] {
        assert_eq!(PointerShape::over(Some(id)), PointerShape::Hand, "{id:?}");
    }
    for id in [
        Widget::Stage.id(),
        Widget::Header.id(),
        Widget::stage_row(0),
        Widget::launch_session(0),
        Widget::RailDivider.id(),
        Widget::RoadmapGraph.id(),
    ] {
        assert_eq!(
            PointerShape::over(Some(id)),
            PointerShape::Default,
            "{id:?}"
        );
    }
    assert_eq!(PointerShape::over(None), PointerShape::Default);
    assert_eq!(PointerShape::default(), PointerShape::Default);
}

/// The wire form is kitty's OSC 22: a W3C cursor name for the hand, an
/// empty name to hand the shape back.
#[test]
fn the_sequences_are_the_osc_22_strings() {
    assert_eq!(PointerShape::Hand.sequence(), b"\x1b]22;pointer\x1b\\");
    assert_eq!(PointerShape::Default.sequence(), b"\x1b]22;\x1b\\");
}
