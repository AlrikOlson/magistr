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
