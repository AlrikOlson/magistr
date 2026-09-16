use super::*;
use magistr_cockpit::render::Color;

#[test]
fn wrapped_unicode_editor_keeps_its_end_caret_visible_inside_the_viewport() {
    let mut painter = PixelView::default();
    let mut tree = LayoutTree::new();
    let draft = "small words ".repeat(40) + " café e\u{301} 漢字";
    let root = Node::editor(&draft, draft.len()).style(
        Style::new()
            .fg(Color::Rgb(255, 255, 255))
            .bg(Color::Rgb(0, 0, 0)),
    );
    let resolved = painter
        .layout(&mut tree, &root, 12, 2, CellSize::DEFAULT)
        .expect("layout");
    let (_, ink) = painter.layers(
        &root,
        &resolved,
        12,
        2,
        CellSize::DEFAULT,
        &magistr_cockpit::theme::Theme::default().palette(),
    );
    // A 2px × 16px caret must survive on the final visible row.
    let caret = (0..119).any(|x| {
        (22..38).all(|y| {
            let at = (y * 120 + x) * 4;
            ink.data()[at + 3] == 255 && ink.data()[at + 7] == 255
        })
    });
    assert!(
        caret,
        "the editor scrolled to its cursor, not its first lines"
    );
}

#[test]
fn labels_use_fractional_pixel_rects_instead_of_counted_columns() {
    let mut painter = PixelView::default();
    let root = Node::row()
        .child(Node::text("iii"))
        .child(Node::text("WWW"));
    let resolved = painter
        .layout(&mut LayoutTree::new(), &root, 40, 2, CellSize::DEFAULT)
        .expect("layout");
    let placed = resolved.placed();
    assert!(placed[1].px.width < placed[2].px.width);
    assert_ne!(placed[1].px.width % 10.0, 0.0);
}
