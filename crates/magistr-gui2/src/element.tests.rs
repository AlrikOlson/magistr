use accesskit::Role;
use qs_gpu::color::Srgba;

use super::*;
use crate::element_id::ElementId;

fn white() -> Srgba {
    Srgba::new(1.0, 1.0, 1.0, 1.0)
}

fn black() -> Srgba {
    Srgba::new(0.0, 0.0, 0.0, 1.0)
}

fn voice() -> Voice {
    Voice {
        family: "sans",
        px: 13.0,
        weight: 400,
        tracking_em: 0.0,
        line_height: 0.0,
        upper: false,
    }
}

#[test]
fn builders_set_what_they_name() {
    let ink = Srgba::new(0.1, 0.2, 0.3, 1.0);
    let element = Element::row()
        .pad(8.0)
        .gap(4.0)
        .width(120.0)
        .height(36.0)
        .grow(1.0)
        .center()
        .space_between()
        .fill(ink)
        .hover_fill(ink)
        .press_fill(ink)
        .stroke(ink, 1.0)
        .dashed()
        .glow(ink, 6.0)
        .radius(11.0)
        .elevation(Elevation::row())
        .id(ElementId(7))
        .focusable()
        .focus_ring(ink)
        .role(Role::Button)
        .label("ship");
    assert_eq!(element.axis, Axis::Row);
    assert_eq!(element.pad, [8.0; 4]);
    assert_eq!(element.gap, 4.0);
    assert_eq!((element.width, element.height), (Some(120.0), Some(36.0)));
    assert_eq!(element.grow, 1.0);
    assert_eq!(element.align, Align::Center);
    assert_eq!(element.justify, Justify::SpaceBetween);
    assert!(element.dashed);
    assert_eq!(element.id, Some(ElementId(7)));
    assert!(element.focusable && !element.disabled);
    assert_eq!(element.role, Role::Button);
    assert_eq!(element.label.as_deref(), Some("ship"));
}

#[test]
fn the_remaining_arrangements_construct() {
    assert_eq!(Element::surface().axis, Axis::Column);
    assert_eq!(Element::surface().align_start().align, Align::Start);
    assert_eq!(Element::surface().align_end().align, Align::End);
    assert_eq!(Element::surface().justify_center().justify, Justify::Center);
    assert_eq!(Element::surface().justify_end().justify, Justify::End);
    assert_eq!(
        Element::surface().gradient(white(), black()).gradient,
        Some([white(), black()])
    );
    let side = Element::surface().pad_x(3.0).pad_y(5.0);
    assert_eq!(side.pad, [3.0, 5.0, 3.0, 5.0]);
    assert!(Element::surface().disabled().disabled);
}

#[test]
fn a_text_run_keeps_its_voice_and_wears_its_ink_in_the_fill_slot() {
    let ink = Srgba::new(0.9, 0.9, 0.9, 1.0);
    let run = Element::text(voice(), ink, "alpha");
    assert_eq!(run.fill, Some(ink));
    assert_eq!(run.role, Role::Label);
    match &run.kind {
        Kind::Text { voice, content } => {
            assert_eq!(voice.px, 13.0);
            assert_eq!(content, "alpha");
        }
        Kind::Surface | Kind::Mark { .. } | Kind::Picture { .. } | Kind::Grid { .. } => {
            panic!("a text run is neither surface, mark, picture nor cells")
        }
    }
}

#[test]
fn a_picture_takes_its_own_aspect_at_the_asked_height_and_is_never_scaled_up() {
    use magistr_cockpit::pixel::CellSize;
    use magistr_cockpit::turn::StreamImage;
    let png = |w: u32, h: u32| {
        let mut pixmap = tiny_skia::Pixmap::new(w, h).expect("a pixmap allocates");
        pixmap.fill(tiny_skia::Color::from_rgba8(40, 120, 200, 255));
        pixmap.encode_png().expect("png")
    };
    let wide = StreamImage::decode(&png(120, 40), "wide", CellSize::DEFAULT).expect("decodes");
    let thumb = Element::picture(wide, 20.0);
    assert_eq!((thumb.width, thumb.height), (Some(60.0), Some(20.0)));
    assert_eq!(thumb.role, Role::Image);
    assert_eq!(thumb.label.as_deref(), Some("wide"));
    assert!(matches!(thumb.kind, Kind::Picture { .. }));
    // Smaller than the asked height: its own pixels, not a blow-up.
    let small = StreamImage::decode(&png(10, 8), "small", CellSize::DEFAULT).expect("decodes");
    let own = Element::picture(small, 20.0);
    assert_eq!((own.width, own.height), (Some(10.0), Some(8.0)));
}

#[test]
fn each_numbers_the_tree_depth_first() {
    let tree = Element::surface()
        .child(
            Element::row()
                .child(Element::text(voice(), white(), "a"))
                .child(Element::text(voice(), white(), "b")),
        )
        .child(Element::surface());
    let mut seen = Vec::new();
    tree.each(&mut |index, element| {
        seen.push((index, matches!(element.kind, Kind::Text { .. })));
    });
    assert_eq!(
        seen,
        vec![(0, false), (1, false), (2, true), (3, true), (4, false)]
    );
    assert_eq!(tree.count(), 5);
}

/// A Cells element is the terminal's: its role, its rows read back as
/// its label, no id and no focus — and its size scales with the tree.
#[test]
fn cells_are_a_read_only_terminal_whose_rows_are_the_label() {
    use crate::pane_tile::PaneTile;
    let cell = |t: &str| PaneTile {
        text: t.to_owned(),
        ink: Srgba::new(1.0, 1.0, 1.0, 1.0),
        ground: None,
        bold: false,
        italic: false,
        underline: false,
    };
    let rows = vec![
        vec![cell("h"), cell("i"), cell(" "), cell(" ")],
        vec![cell("y"), cell("o")],
    ];
    let cells = Element::grid(rows, 11.0);
    assert_eq!(cells.role, Role::Terminal);
    assert_eq!(cells.label.as_deref(), Some("hi\nyo"));
    assert!(cells.id.is_none() && !cells.focusable);
    assert!(cells.runs().is_empty(), "cells are not text runs");
    let doubled = scaled(cells, 2.0);
    match &doubled.kind {
        Kind::Grid { px, rows } => {
            assert_eq!(*px, 22.0);
            assert_eq!(rows.len(), 2);
        }
        _ => panic!("still cells"),
    }
}
