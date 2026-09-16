use super::*;

#[test]
fn translated_prose_keeps_its_baseline_under_a_viewport_clip() {
    let mut painter = PixelView::default();
    let palette = magistr_cockpit::theme::Theme::default().palette();
    let render = |painter: &mut PixelView, offset: f32| {
        let root = Node::column().viewport().child(
            Node::text("The saved result")
                .text_role(TextRole::Prose)
                .height(magistr_cockpit::layout::Length::Px(20.0))
                .shrink(0.0)
                .offset_y(offset)
                .style(Style::new().fg(Color::Rgb(255, 255, 255))),
        );
        let resolved = painter
            .layout(&mut LayoutTree::new(), &root, 20, 2, CellSize::DEFAULT)
            .unwrap();
        let placed = &resolved.placed()[1];
        assert_eq!(placed.bounds.y, offset);
        assert_eq!(placed.px.y, offset.max(0.0));
        painter
            .layers(&root, &resolved, 20, 2, CellSize::DEFAULT, &palette)
            .1
    };
    let full = render(&mut painter, 0.0);
    let clipped = render(&mut painter, -7.0);
    for y in 0..13usize {
        let stride = 200 * 4;
        assert_eq!(
            &clipped.data()[y * stride..(y + 1) * stride],
            &full.data()[(y + 7) * stride..(y + 8) * stride]
        );
    }
}

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
