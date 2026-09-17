use super::*;

#[test]
fn pixels_scale_a_rect_and_saturate_at_the_wire_limit() {
    assert_eq!(CellSize::new(16, 33).pixels(40, 12), (640, 396));
    assert_eq!(CellSize::new(16, 33).pixels(u16::MAX, 1), (u16::MAX, 33));
}
