use super::*;
use crate::input::Button;

#[test]
fn a_terminal_report_carries_no_pixel() {
    let event = MouseEvent::new(MouseKind::Down(Button::Left), 3, 4);
    assert_eq!(event.px, None);
    assert_eq!((event.col, event.row), (3, 4));
    assert_eq!(event.mods, Modifiers::NONE);
}

#[test]
fn a_window_report_adds_the_pixel_to_the_cell_it_resolved() {
    let event = MouseEvent::new(MouseKind::Move, 3, 4)
        .with(Modifiers::SHIFT)
        .at_px(31, 85);
    assert_eq!(event.px, Some((31, 85)));
    assert_eq!((event.col, event.row), (3, 4), "the cell is untouched");
    assert_eq!(event.mods, Modifiers::SHIFT, "and so are the modifiers");
}
