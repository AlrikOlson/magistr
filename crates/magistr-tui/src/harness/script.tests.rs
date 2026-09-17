use std::time::Duration;

use super::*;

const MS: fn(u64) -> Duration = Duration::from_millis;

#[test]
fn steps_come_out_in_time_order_with_ties_in_insertion_order() {
    let script = Script::new()
        .text(MS(20), "b")
        .text(MS(0), "a")
        .close(MS(20))
        .resize(MS(10), Size::new(10, 5));
    assert_eq!(script.len(), 4);
    let steps = script.into_steps();
    assert_eq!(
        steps,
        vec![
            (MS(0), Step::Bytes(b"a".to_vec())),
            (MS(10), Step::Resize(Size::new(10, 5))),
            (MS(20), Step::Bytes(b"b".to_vec())),
            (MS(20), Step::Close),
        ]
    );
}

#[test]
fn a_click_is_an_sgr_press_and_release_one_based() {
    let steps = Script::new().click(MS(0), 4, 1).into_steps();
    assert_eq!(
        steps[0].1,
        Step::Bytes(b"\x1b[<0;5;2M\x1b[<0;5;2m".to_vec())
    );
}

#[test]
fn the_startup_replies_end_the_probe_and_answer_every_query() {
    use crate::graphics::{Passthrough, Probe};
    use crate::input::{Decoder, Event, Reply};
    use magistr_cockpit::render::Color;

    let steps = Script::new().startup_replies(MS(0)).into_steps();
    let Step::Bytes(bytes) = &steps[0].1 else {
        panic!("bytes");
    };
    let mut probe = Probe::new(Passthrough::Direct);
    assert!(probe.feed(bytes));
    assert!(probe.reply().kitty);
    let mut events = Vec::new();
    Decoder::new().feed(bytes, &mut events);
    assert_eq!(
        events,
        vec![
            Event::Reply(Reply::Background(Color::Rgb(30, 30, 46))),
            Event::Reply(Reply::KeyboardFlags(3)),
            Event::Reply(Reply::CellPixels {
                width: 10,
                height: 20
            }),
        ]
    );

    let steps = Script::new().plain_replies(MS(0)).into_steps();
    let Step::Bytes(bytes) = &steps[0].1 else {
        panic!("bytes");
    };
    let mut probe = Probe::new(Passthrough::Direct);
    assert!(probe.feed(bytes));
    assert!(!probe.reply().kitty);
}

#[test]
fn an_empty_script_is_empty() {
    assert!(Script::new().is_empty());
    assert!(!Script::new().close(MS(0)).is_empty());
}

#[test]
fn a_mouse_move_is_sgr_motion_with_no_button() {
    let steps = Script::new().mouse_move(MS(0), 4, 1).into_steps();
    assert_eq!(steps[0].1, Step::Bytes(b"\x1b[<35;5;2M".to_vec()));
}
