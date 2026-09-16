use super::*;
use magistr_cockpit::cockpit::{Size, Widget};
use magistr_cockpit::layout::{Length, TextRole};
use magistr_cockpit::render::{Color, Style};
use std::time::Duration;

#[test]
fn custom_compositions_keep_fractional_pixels_and_use_the_real_graphics_wire() {
    let model = Model::new(Size::new(80, 24));
    let clock = FrameClock::new(0, Duration::ZERO);
    let view = |_: &Model, _: &FrameClock| {
        Node::row()
            .child(Node::leaf().width(Length::Px(13.5)).shrink(0.0))
            .child(
                Node::text("WWW iii")
                    .id(Widget::Prompt.id())
                    .text_role(TextRole::Prose)
                    .width(Length::Px(137.25))
                    .style(Style::new().fg(Color::Rgb(255, 255, 255))),
            )
    };
    let resolved = Preview::layout(&model, &clock, view).expect("pixel layout");
    let rect = resolved
        .placed()
        .iter()
        .find(|p| p.id == Some(Widget::Prompt.id()))
        .expect("text rectangle")
        .px;
    assert_eq!(rect.x, 13.5, "layout must not snap to a cell boundary");
    assert_eq!(rect.width, 137.25);
    let (cells, bytes) = Preview::wire(&model, &clock, Protocol::Kitty(Passthrough::Direct), view)
        .expect("native wire");
    assert!(
        cells.cells().iter().all(|c| c.symbol == " "),
        "graphics output must contain no terminal-painted glyphs"
    );
    let placements = kitty::decode(&bytes);
    assert_eq!(
        placements.len(),
        6,
        "six opaque four-row strips cover 24 rows"
    );
    assert!(
        placements
            .iter()
            .all(|p| p.rgba.chunks_exact(4).all(|px| px[3] == 255))
    );
    let capture = Preview::frame(&model, &clock, view).expect("capture");
    let unique: std::collections::HashSet<_> = capture.data().chunks_exact(4).collect();
    assert!(
        unique.len() > 2,
        "the proportional text is antialiased into the actual pixels"
    );
}
