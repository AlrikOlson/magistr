use super::*;
use crate::render::{Attrs, Color};

fn style(fg: u8) -> Style {
    Style::new().fg(Color::Indexed(fg))
}

#[test]
fn a_plain_frame_has_no_title_no_meta_and_titles_fall_back_to_the_ring() {
    let frame = Frame::new(style(1));
    assert_eq!(frame.title_text(), None);
    assert_eq!(frame.meta_text(), None);
    assert_eq!(frame.border_style(), &style(1));
    assert_eq!(frame.heading_style(), &style(1));
}

#[test]
fn title_meta_and_title_style_are_carried() {
    let frame = Frame::new(style(1))
        .title("plan")
        .meta("3/7")
        .title_style(style(2).attrs(Attrs::BOLD));
    assert_eq!(frame.title_text(), Some("plan"));
    assert_eq!(frame.meta_text(), Some("3/7"));
    assert_eq!(frame.border_style(), &style(1));
    assert_eq!(frame.heading_style(), &style(2).attrs(Attrs::BOLD));
}

#[test]
fn a_frame_is_spelled_unless_asked_to_be_drawn() {
    assert!(!Frame::new(style(1)).is_drawn());
    assert!(Frame::new(style(1)).drawn(true).is_drawn());
    assert!(!Frame::new(style(1)).drawn(false).is_drawn());
}

#[test]
fn the_title_span_is_the_title_in_its_spaces_after_the_corner_cut_to_the_slot() {
    assert_eq!(Frame::new(style(1)).title_span(20), None);
    assert_eq!(Frame::new(style(1)).title("").title_span(20), None);
    assert_eq!(
        Frame::new(style(1)).title("plan").title_span(20),
        Some((2, 6))
    );
    // A ring 9 wide has a 5-cell slot: three characters of title.
    assert_eq!(
        Frame::new(style(1)).title("roadmap").title_span(9),
        Some((2, 5))
    );
    // Too narrow for even one character: no title at all.
    assert_eq!(Frame::new(style(1)).title("plan").title_span(6), None);
}
