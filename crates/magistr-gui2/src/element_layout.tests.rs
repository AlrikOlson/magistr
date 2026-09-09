use qs_gpu::color::Srgba;

use super::*;
use crate::element_id::ElementId;

fn white() -> Srgba {
    Srgba::new(1.0, 1.0, 1.0, 1.0)
}

fn prose() -> Voice {
    Voice {
        family: "sans",
        px: 13.0,
        weight: 400,
        tracking_em: 0.0,
        line_height: 1.65,
        upper: false,
    }
}

fn cap() -> Voice {
    Voice {
        family: "mono",
        px: 9.5,
        weight: 400,
        tracking_em: 0.12,
        line_height: 0.0,
        upper: true,
    }
}

#[test]
fn fixed_boxes_land_on_their_own_pixels_with_no_quantization() {
    let mut text = TextEngine::embedded();
    let tree = Element::row()
        .pad(7.0)
        .gap(5.0)
        .child(Element::surface().width(13.0).height(21.0))
        .child(Element::surface().width(30.5).height(10.0));
    let layout = ElementLayout::compute(&tree, 200.0, 100.0, &mut text).expect("layout");
    let [x0, y0, w0, _] = layout.rect(1);
    let [x1, ..] = layout.rect(2);
    assert_eq!((x0, y0), (7.0, 7.0), "the pad is the token's own f32");
    assert_eq!(w0, 13.0, "a 13 px box stays 13 px — no grid rounds it");
    // 7 + 13 + 5: the sibling starts after the gap, still in pixels.
    assert_eq!(x1, 25.0);
    assert_eq!(layout.len(), 3);
    assert!(!layout.is_empty());
}

#[test]
fn a_text_run_is_as_wide_as_the_shaper_says_and_one_line_box_tall() {
    let mut text = TextEngine::embedded();
    let tree = Element::row()
        .align_start()
        .child(Element::text(prose(), white(), "alpha beta"));
    let layout = ElementLayout::compute(&tree, 400.0, 100.0, &mut text).expect("layout");
    let [_, _, w, h] = layout.rect(1);
    let voice = prose();
    let shaped = shape_voice(&mut text, &voice, "alpha beta").width().ceil();
    let (line, baseline) = line_box(&mut text, &voice);
    assert_eq!(w, shaped, "width is the shaped ink, rounded up once");
    assert_eq!(h, line, "height is the voice's own line box");
    assert!(
        line >= (13.0f32 * 1.65).floor(),
        "the prose leading holds: {line} px line for 13 px type"
    );
    assert!(baseline > 0.0 && baseline <= line);
    assert_eq!(layout.text_rows(1).len(), 1);
}

#[test]
fn a_narrow_box_breaks_the_paragraph_where_shaped_words_stop_fitting() {
    let mut text = TextEngine::embedded();
    let voice = prose();
    let word = shape_voice(&mut text, &voice, "alpha").width();
    // Room for one word and change, never two words and the space.
    let narrow = word * 1.6;
    let tree = Element::surface().align_start().child(
        Element::surface()
            .width(narrow)
            .child(Element::text(voice, white(), "alpha beta gamma")),
    );
    let layout = ElementLayout::compute(&tree, 400.0, 200.0, &mut text).expect("layout");
    let rows = layout.text_rows(2);
    assert_eq!(rows.len(), 3, "one word per row: {rows:?}");
    let (line, _) = line_box(&mut text, &prose());
    let [_, _, _, h] = layout.rect(2);
    assert_eq!(h, (line * 3.0).ceil(), "three rows of height were paid for");
    // The rows partition the content — nothing is dropped or doubled.
    assert_eq!(rows[0], (0, 5));
    assert_eq!(rows[1], (6, 10));
    assert_eq!(rows[2], (11, 16));
}

#[test]
fn a_newline_always_breaks_and_an_empty_row_still_takes_its_line() {
    let mut text = TextEngine::embedded();
    let tree = Element::surface().child(Element::text(prose(), white(), "a\n\nb"));
    let layout = ElementLayout::compute(&tree, 400.0, 200.0, &mut text).expect("layout");
    let rows = layout.text_rows(1);
    assert_eq!(rows.len(), 3, "{rows:?}");
    assert_eq!(&rows[1], &(2, 2), "the blank row is a zero-width range");
}

#[test]
fn a_tracked_cap_is_measured_uppercased_with_its_tracking() {
    let mut text = TextEngine::embedded();
    let voice = cap();
    let run = shape_voice(&mut text, &voice, "verify");
    assert_eq!(
        run.text.as_ref(),
        "VERIFY",
        "the transform ran before shaping"
    );
    assert!(run.tracking > 0.0, "0.12 em at 9.5 px is real tracking");
    let plain = text
        .run("VERIFY", run.set.font, run.set.size, run.set.features)
        .width;
    assert!(run.width() > plain, "the tracked box is wider than its ink");
}

#[test]
fn grow_and_center_arrange_in_pixels() {
    let mut text = TextEngine::embedded();
    let tree = Element::row()
        .center()
        .child(Element::surface().grow(1.0).height(10.0))
        .child(Element::surface().width(40.0).height(20.0));
    let layout = ElementLayout::compute(&tree, 100.0, 50.0, &mut text).expect("layout");
    let [_, y0, w0, _] = layout.rect(1);
    let [x1, y1, ..] = layout.rect(2);
    assert_eq!(w0, 60.0, "grow takes what the fixed sibling leaves");
    assert_eq!(x1, 60.0);
    assert_eq!(y0, 20.0, "a 10 px child is centred in the 50 px row");
    assert_eq!(y1, 15.0);
}

#[test]
fn ids_survive_layout_untouched() {
    let mut text = TextEngine::embedded();
    let tree = Element::surface().child(Element::surface().id(ElementId(3)));
    let layout = ElementLayout::compute(&tree, 50.0, 50.0, &mut text).expect("layout");
    assert_eq!(
        layout.rect(1)[2],
        50.0,
        "a bare child stretches to its parent"
    );
}

/// A pinned child leaves flex flow and lands at its insets — left/top
/// against the parent's box, right/bottom measured back from its far
/// edges. The volume's switch thumbs, meter fills and card badges all
/// hang on this.
#[test]
fn a_pinned_child_is_absolute_at_its_insets() {
    let mut text = TextEngine::embedded();
    let tree = Element::row()
        .width(32.0)
        .height(19.0)
        .child(
            Element::surface()
                .width(14.0)
                .height(14.0)
                .pin_left(2.5)
                .pin_top(2.5),
        )
        .child(
            Element::surface()
                .width(14.0)
                .height(14.0)
                .pin_right(2.5)
                .pin_top(2.5),
        );
    let layout = ElementLayout::compute(&tree, 32.0, 19.0, &mut text).expect("layout");
    assert_eq!(layout.rect(1), [2.5, 2.5, 14.0, 14.0], "left-pinned");
    assert_eq!(
        layout.rect(2),
        [32.0 - 2.5 - 14.0, 2.5, 14.0, 14.0],
        "right-pinned lands measured back from the far edge"
    );
}

/// A pinned stripe with top and bottom insets spans between them — the
/// meter's fill (`top:0; bottom:0`) and its breaker tick (`top:-2px;
/// bottom:-2px`), which reaches OUTSIDE its parent's box.
#[test]
fn a_pinned_stripe_spans_its_vertical_insets_even_negative_ones() {
    let mut text = TextEngine::embedded();
    let tree = Element::row()
        .width(220.0)
        .height(4.0)
        .child(
            Element::surface()
                .width(33.0)
                .pin_left(0.0)
                .pin_top(0.0)
                .pin_bottom(0.0),
        )
        .child(
            Element::surface()
                .width(2.0)
                .pin_left(182.6)
                .pin_top(-2.0)
                .pin_bottom(-2.0),
        );
    let layout = ElementLayout::compute(&tree, 220.0, 4.0, &mut text).expect("layout");
    assert_eq!(
        layout.rect(1),
        [0.0, 0.0, 33.0, 4.0],
        "the fill spans the track"
    );
    assert_eq!(
        layout.rect(2),
        [182.6, -2.0, 2.0, 8.0],
        "the tick overshoots the track by 2px both ways"
    );
}

/// The transcript column's physics: flex min-size defaults to auto, so a feed holding
/// more pixel-height than its box refuses to shrink and pushes everything under it
/// below the fold. The cure is CSS's own scrollable-middle pattern, whole: basis zero
/// (`height(0.0)` + grow) so the siblings never enter shrink, and `min_height(0.0)` so
/// the automatic minimum cannot inflate the box — with the tail packed to the end, the
/// overflow moves to the TOP and the last row lands flush on the feed's bottom edge
/// while the composer keeps both its place and its own height.
#[test]
fn min_height_zero_lets_a_column_shrink_and_justify_end_anchors_the_tail() {
    let mut text = TextEngine::embedded();
    let feed = |floored: bool| {
        let mut feed = Element::surface().grow(1.0).justify_end();
        if !floored {
            feed = feed.height(0.0).min_height(0.0);
        }
        feed.child(Element::surface().height(40.0).no_shrink())
            .child(Element::surface().height(40.0).no_shrink())
            .child(Element::surface().height(40.0).no_shrink())
    };
    let tree = |floored| {
        Element::surface()
            .child(feed(floored))
            .child(Element::surface().height(20.0))
    };

    // Today's bug, kept as the negative: min-size auto holds the feed
    // at its 120 px content, and the composer is pushed below the fold.
    let inflated = ElementLayout::compute(&tree(true), 60.0, 100.0, &mut text).expect("layout");
    assert!(
        inflated.rect(5)[1] >= 100.0,
        "without min-height 0 the composer sits below the fold: {:?}",
        inflated.rect(5)
    );

    let anchored = ElementLayout::compute(&tree(false), 60.0, 100.0, &mut text).expect("layout");
    assert_eq!(
        anchored.rect(1),
        [0.0, 0.0, 60.0, 80.0],
        "the feed shrank to the room the composer leaves"
    );
    assert_eq!(
        anchored.rect(5),
        [0.0, 80.0, 60.0, 20.0],
        "the composer is on screen, flush under the feed"
    );
    assert_eq!(
        anchored.rect(4),
        [0.0, 40.0, 60.0, 40.0],
        "the tail row's bottom is the feed's bottom"
    );
    assert_eq!(
        anchored.rect(2)[1],
        -40.0,
        "the overflow stands above the top edge, not below the fold"
    );
}

/// A Cells leaf is its grid: as many advances wide as its widest row,
/// as many line heights tall as it has rows — the face's own whole
/// pixels, so a column is a column on every row.
#[test]
fn a_cells_leaf_measures_its_grid() {
    use crate::pane_tile::PaneTile;
    let mut text = TextEngine::embedded();
    let cell = |t: &str| PaneTile {
        text: t.to_owned(),
        ink: white(),
        ground: None,
        bold: false,
        italic: false,
        underline: false,
    };
    let rows = vec![
        (0..5).map(|_| cell("a")).collect::<Vec<_>>(),
        (0..3).map(|_| cell("b")).collect(),
    ];
    let tree = Element::surface()
        .align_start()
        .child(Element::grid(rows, 11.0));
    let layout = ElementLayout::compute(&tree, 400.0, 200.0, &mut text).expect("layout");
    let metrics = text.metrics(11.0).expect("the face has metrics");
    let [_, _, w, h] = layout.rect(1);
    assert_eq!(w, 5.0 * metrics.width());
    assert_eq!(h, 2.0 * metrics.height());
    assert_eq!(w.fract(), 0.0);
    assert_eq!(h.fract(), 0.0);
}

/// A translated range moves down by exactly its pixels — every element
/// in the range, none outside it, and only the y: the subtree's own
/// layout is what it was, carried to where the paint is between two
/// layouts (the feed between rows while a scroll glides).
#[test]
fn a_translated_range_carries_only_its_own_rects_down() {
    let mut text = TextEngine::embedded();
    let tree = Element::surface()
        .child(Element::surface().width(10.0).height(10.0))
        .child(
            Element::surface()
                .width(10.0)
                .height(20.0)
                .child(Element::surface().width(4.0).height(4.0)),
        )
        .child(Element::surface().width(10.0).height(10.0));
    let mut layout = ElementLayout::compute(&tree, 100.0, 100.0, &mut text).expect("layout");
    let before: Vec<[f32; 4]> = (0..layout.len()).map(|i| layout.rect(i)).collect();
    // The second child and its own child: indices 2 and 3.
    layout.translate(2..4, 7.5);
    for (index, was) in before.iter().enumerate() {
        let now = layout.rect(index);
        let dy = if (2..4).contains(&index) { 7.5 } else { 0.0 };
        assert_eq!(now[0], was[0], "x holds at {index}");
        assert!(
            (now[1] - (was[1] + dy)).abs() < 1e-5,
            "y at {index}: {now:?}"
        );
        assert_eq!((now[2], now[3]), (was[2], was[3]), "size holds at {index}");
    }
    // A negative move carries up; an empty range moves nothing.
    layout.translate(2..4, -7.5);
    layout.translate(0..0, 100.0);
    for (index, was) in before.iter().enumerate() {
        assert!(
            (layout.rect(index)[1] - was[1]).abs() < 1e-5,
            "back where it was at {index}"
        );
    }
}
