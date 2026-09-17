use super::*;
use magistr_cockpit::input::{Button, MouseKind};

use crate::element::Element;
use crate::icon::Icon;
use crate::mark::Mark;
use crate::voice::Voice;

/// A 2x frame carries the design's px doubled everywhere a pixel token
/// lives — the tree the physical surface is laid out from.
#[test]
fn scaled_doubles_every_pixel_token() {
    let lex = Lexicon::of(Medium::Night);
    let tree = Element::surface()
        .pad(10.0)
        .gap(6.0)
        .width(100.0)
        .height(40.0)
        .radius(13.0)
        .stroke(lex.mauve, 1.0)
        .dash(3.0, 3.0)
        .glow(lex.mauve, 8.0)
        .child(
            Element::text(Voice::SLOT, lex.tx, "a run")
                .pin_left(4.0)
                .pin_top(2.5),
        )
        .child(Element::mark(Mark::Icon(Icon::Done), 13, (255, 255, 255)));
    let scaled = scaled(tree, 2.0);
    assert_eq!(scaled.pad, [20.0; 4]);
    assert_eq!(scaled.gap, 12.0);
    assert_eq!(scaled.width, Some(200.0));
    assert_eq!(scaled.height, Some(80.0));
    assert_eq!(scaled.radius, 26.0);
    assert_eq!(scaled.stroke.map(|(_, w)| w), Some(2.0));
    assert_eq!(scaled.dash, (6.0, 6.0));
    assert_eq!(scaled.glow.map(|(_, f)| f), Some(16.0));
    let text = &scaled.children[0];
    assert_eq!(text.pin[0], Some(8.0));
    assert_eq!(text.pin[1], Some(5.0));
    let crate::element::Kind::Text { voice, .. } = &text.kind else {
        panic!("still a text run");
    };
    assert!((voice.px - Voice::SLOT.px * 2.0).abs() < f32::EPSILON);
    let crate::element::Kind::Mark { px, .. } = &scaled.children[1].kind else {
        panic!("still a mark");
    };
    assert_eq!(*px, 26);
    // The mark's box widths scale with the rest.
    assert_eq!(scaled.children[1].width, Some(26.0));
}

/// Scale 1 is an untouched tree — the 1x path pays nothing.
#[test]
fn scale_one_changes_nothing() {
    let tree = Element::surface().pad(10.0).width(100.0);
    let same = scaled(tree, 1.0);
    assert_eq!(same.pad, [10.0; 4]);
    assert_eq!(same.width, Some(100.0));
}

/// A 2x display's physical pixel lands on the logical layout's own
/// coordinates — the whole of the sink's HiDPI answer.
#[test]
fn descale_halves_a_retina_pixel() {
    let msg = Msg::Mouse {
        event: MouseEvent::new(MouseKind::Down(Button::Left), 3, 4).at_px(220, 640),
        target: None,
    };
    let Msg::Mouse { event, .. } = descale(msg, 2.0) else {
        panic!("still a mouse message");
    };
    assert_eq!(event.px, Some((110, 320)));
    assert!(matches!(event.kind, MouseKind::Down(Button::Left)));
}

/// Scale 1 changes nothing, and a message without a pixel passes
/// through untouched.
#[test]
fn descale_leaves_unscaled_and_pixel_free_messages_alone() {
    let msg = Msg::Mouse {
        event: MouseEvent::new(MouseKind::Move, 1, 2).at_px(64, 32),
        target: None,
    };
    let Msg::Mouse { event, .. } = descale(msg, 1.0) else {
        panic!("still a mouse message");
    };
    assert_eq!(event.px, Some((64, 32)));

    let bare = Msg::Mouse {
        event: MouseEvent::new(MouseKind::Move, 1, 2),
        target: None,
    };
    let Msg::Mouse { event, .. } = descale(bare, 2.0) else {
        panic!("still a mouse message");
    };
    assert_eq!(event.px, None);
}
