use std::fmt::Write as _;

use super::*;
use crate::layout::{Align, Inset, Justify, Length};
use crate::widget_id::WidgetId;

fn id(raw: u32) -> WidgetId {
    WidgetId::new(raw)
}

/// Draw every id-carrying rect as its letter (`A` for id 0, `B` for 1, …)
/// in tree order so children paint over parents, then list the rects.
fn map(resolved: &Resolved, width: u16, height: u16) -> String {
    let mut grid = vec![vec!['.'; usize::from(width)]; usize::from(height)];
    let mut listing = String::new();
    for placed in resolved.placed() {
        let Some(widget) = placed.id else {
            continue;
        };
        let letter = char::from(b'A' + u8::try_from(widget.raw() % 26).expect("< 26"));
        let rect = placed.rect;
        for row in rect.y..rect.bottom() {
            for col in rect.x..rect.right() {
                grid[usize::from(row)][usize::from(col)] = letter;
            }
        }
        // The layer is printed only when there is one, so a tree with
        // no overlay reads exactly as it always did.
        let layer = match placed.layer {
            0 => String::new(),
            n => format!(" layer={n}"),
        };
        let _ = writeln!(
            listing,
            "{letter} depth={}{layer} x={} y={} w={} h={}",
            placed.depth, rect.x, rect.y, rect.width, rect.height
        );
    }
    let mut out = format!("{width}x{height}\n");
    for row in grid {
        out.push_str(&row.into_iter().collect::<String>());
        out.push('\n');
    }
    out.push_str(&listing);
    out
}

fn layout(root: &Node, width: u16, height: u16) -> Resolved {
    LayoutTree::new()
        .compute(root, width, height)
        .expect("layout succeeds")
}

fn cockpit() -> Node {
    Node::column()
        .child(Node::leaf().id(id(0)).height(Length::Cells(1)))
        .child(
            Node::row()
                .grow(1.0)
                .child(
                    Node::column()
                        .id(id(1))
                        .width(Length::Cells(24))
                        .shrink(0.0)
                        .padding(1)
                        .child(Node::text("roadmap\nnext: tui2-0a2").id(id(5))),
                )
                .child(
                    Node::column()
                        .id(id(2))
                        .grow(1.0)
                        .padding(1)
                        .child(
                            Node::text(
                                "The stage shows the agent's stream: plan, execute, verify, and the tool calls in between.",
                            )
                            .id(id(6)),
                        ),
                )
                // The stage's auto basis is its text width; without
                // `shrink(0.0)` the dock would give way to it (CSS semantics).
                .child(
                    Node::column()
                        .id(id(3))
                        .width(Length::Percent(0.3))
                        .shrink(0.0),
                ),
        )
        .child(Node::leaf().id(id(4)).height(Length::Cells(1)))
}

// --- unit ---------------------------------------------------------------

#[test]
fn the_root_is_forced_to_the_screen() {
    let root = Node::leaf()
        .id(id(0))
        .width(Length::Cells(5))
        .height(Length::Cells(5));
    let resolved = layout(&root, 30, 12);
    assert_eq!(resolved.rect(id(0)), Some(Rect::new(0, 0, 30, 12)));
    assert_eq!(resolved.len(), 1);
    assert_eq!(resolved.placed()[0].depth, 0);
}

#[test]
fn nodes_come_back_in_tree_order_with_depths() {
    let root = Node::column()
        .id(id(0))
        .child(Node::row().id(id(1)).child(Node::leaf().id(id(2))))
        .child(Node::leaf().id(id(3)));
    let resolved = layout(&root, 10, 4);
    let order: Vec<(Option<u32>, u16)> = resolved
        .placed()
        .iter()
        .map(|p| (p.id.map(WidgetId::raw), p.depth))
        .collect();
    assert_eq!(
        order,
        vec![(Some(0), 0), (Some(1), 1), (Some(2), 2), (Some(3), 1)]
    );
}

#[test]
fn every_rect_lies_within_the_screen() {
    let root = Node::row()
        .child(Node::leaf().id(id(0)).width(Length::Cells(50)).shrink(0.0))
        .child(Node::leaf().id(id(1)).width(Length::Cells(50)).shrink(0.0));
    let resolved = layout(&root, 60, 5);
    let screen = Rect::new(0, 0, 60, 5);
    for placed in resolved.placed() {
        assert_eq!(placed.rect.intersection(screen), placed.rect);
    }
    assert_eq!(resolved.rect(id(1)), Some(Rect::new(50, 0, 10, 5)));
}

#[test]
fn one_engine_serves_many_frames() {
    let mut tree = LayoutTree::default();
    let root = Node::row().child(Node::leaf().id(id(0)).grow(1.0));
    let first = tree.compute(&root, 80, 24).expect("first frame");
    let second = tree.compute(&root, 180, 50).expect("second frame");
    let third = tree.compute(&root, 80, 24).expect("third frame");
    assert_eq!(first.rect(id(0)), Some(Rect::new(0, 0, 80, 24)));
    assert_eq!(second.rect(id(0)), Some(Rect::new(0, 0, 180, 50)));
    assert_eq!(first, third);
}

#[test]
fn a_text_leaf_measures_through_the_grapheme_width() {
    let root = Node::row()
        .child(Node::text("日本語").id(id(0)))
        .child(Node::leaf().grow(1.0));
    let resolved = layout(&root, 20, 3);
    assert_eq!(resolved.rect(id(0)), Some(Rect::new(0, 0, 6, 3)));
}

#[test]
fn a_zero_sized_screen_resolves_to_empty_rects() {
    let root = Node::row().child(Node::text("x").id(id(0)));
    let resolved = layout(&root, 0, 0);
    assert_eq!(resolved.rect(id(0)), Some(Rect::default()));
}

#[test]
fn to_cells_rounds_and_drops_the_offscreen_part() {
    assert_eq!(
        to_cells(0.4, 0.6, 9.6, 2.4, Basis::CELLS),
        Rect::new(0, 1, 10, 2)
    );
    assert_eq!(
        to_cells(-3.0, 0.0, 8.0, 1.0, Basis::CELLS),
        Rect::new(0, 0, 5, 1)
    );
    assert_eq!(
        to_cells(2.0, 2.0, -1.0, -1.0, Basis::CELLS),
        Rect::new(2, 2, 0, 0)
    );
    assert_eq!(cell(f32::NAN), 0);
    assert_eq!(cell(1e9), u16::MAX);
}

// --- snapshots ----------------------------------------------------------

#[test]
fn snap_row_two_fixed_widths() {
    let root = Node::row()
        .child(Node::leaf().id(id(0)).width(Length::Cells(10)))
        .child(Node::leaf().id(id(1)).width(Length::Cells(20)));
    insta::assert_snapshot!(map(&layout(&root, 40, 4), 40, 4));
}

#[test]
fn snap_row_fixed_then_grow() {
    let root = Node::row()
        .child(Node::leaf().id(id(0)).width(Length::Cells(10)))
        .child(Node::leaf().id(id(1)).grow(1.0));
    insta::assert_snapshot!(map(&layout(&root, 40, 4), 40, 4));
}

#[test]
fn snap_row_grow_ratio_one_to_three() {
    let root = Node::row()
        .child(Node::leaf().id(id(0)).grow(1.0))
        .child(Node::leaf().id(id(1)).grow(3.0));
    insta::assert_snapshot!(map(&layout(&root, 40, 3), 40, 3));
}

#[test]
fn snap_column_header_body_footer() {
    let root = Node::column()
        .child(Node::leaf().id(id(0)).height(Length::Cells(1)))
        .child(Node::leaf().id(id(1)).grow(1.0))
        .child(Node::leaf().id(id(2)).height(Length::Cells(1)));
    insta::assert_snapshot!(map(&layout(&root, 20, 8), 20, 8));
}

#[test]
fn snap_percent_widths() {
    let root = Node::row()
        .child(Node::leaf().id(id(0)).width(Length::Percent(0.25)))
        .child(Node::leaf().id(id(1)).width(Length::Percent(0.75)));
    insta::assert_snapshot!(map(&layout(&root, 40, 3), 40, 3));
}

#[test]
fn snap_percent_heights_in_a_column() {
    let root = Node::column()
        .child(Node::leaf().id(id(0)).height(Length::Percent(0.5)))
        .child(Node::leaf().id(id(1)).height(Length::Percent(0.3)))
        .child(Node::leaf().id(id(2)).grow(1.0));
    insta::assert_snapshot!(map(&layout(&root, 12, 10), 12, 10));
}

#[test]
fn snap_min_width_resists_shrinking() {
    let root = Node::row()
        .child(Node::leaf().id(id(0)).width(Length::Cells(20)))
        .child(
            Node::leaf()
                .id(id(1))
                .width(Length::Cells(20))
                .min_width(Length::Cells(18)),
        );
    insta::assert_snapshot!(map(&layout(&root, 30, 3), 30, 3));
}

#[test]
fn snap_max_width_caps_growth() {
    let root = Node::row()
        .child(
            Node::leaf()
                .id(id(0))
                .grow(1.0)
                .max_width(Length::Cells(10)),
        )
        .child(Node::leaf().id(id(1)).grow(1.0));
    insta::assert_snapshot!(map(&layout(&root, 40, 3), 40, 3));
}

#[test]
fn snap_no_shrink_overflow_is_clipped() {
    let root = Node::row()
        .child(Node::leaf().id(id(0)).width(Length::Cells(30)).shrink(0.0))
        .child(Node::leaf().id(id(1)).width(Length::Cells(30)).shrink(0.0));
    insta::assert_snapshot!(map(&layout(&root, 40, 3), 40, 3));
}

#[test]
fn snap_wrapping_row_of_tiles() {
    let tiles = (0..5).map(|n| {
        Node::leaf()
            .id(id(n))
            .width(Length::Cells(10))
            .height(Length::Cells(2))
    });
    let root = Node::row().wrap().children(tiles);
    insta::assert_snapshot!(map(&layout(&root, 25, 8), 25, 8));
}

#[test]
fn snap_gap_between_children() {
    let root = Node::row()
        .gap(2)
        .child(Node::leaf().id(id(0)).grow(1.0))
        .child(Node::leaf().id(id(1)).grow(1.0))
        .child(Node::leaf().id(id(2)).grow(1.0));
    insta::assert_snapshot!(map(&layout(&root, 40, 2), 40, 2));
}

#[test]
fn snap_padding_insets_children() {
    let root = Node::column()
        .id(id(0))
        .padding(1)
        .child(Node::leaf().id(id(1)).grow(1.0));
    insta::assert_snapshot!(map(&layout(&root, 20, 6), 20, 6));
}

#[test]
fn snap_text_takes_its_own_width_in_a_row() {
    let root = Node::row()
        .child(Node::text("hello world").id(id(0)))
        .child(Node::leaf().id(id(1)).grow(1.0));
    insta::assert_snapshot!(map(&layout(&root, 30, 2), 30, 2));
}

#[test]
fn snap_text_wraps_to_the_column_width() {
    let root = Node::column()
        .child(Node::text("the quick brown fox jumps").id(id(0)))
        .child(Node::leaf().id(id(1)).grow(1.0));
    insta::assert_snapshot!(map(&layout(&root, 10, 6), 10, 6));
}

#[test]
fn snap_text_in_an_overfull_row_shrinks_and_wraps() {
    let root = Node::row()
        .child(Node::text("the quick brown fox jumps over").id(id(0)))
        .child(Node::leaf().id(id(1)).width(Length::Cells(8)).shrink(0.0));
    insta::assert_snapshot!(map(&layout(&root, 20, 5), 20, 5));
}

#[test]
fn snap_cjk_text_wraps_by_columns() {
    let root = Node::column()
        .child(Node::text("日本語 テキスト").id(id(0)))
        .child(Node::leaf().id(id(1)).grow(1.0));
    insta::assert_snapshot!(map(&layout(&root, 8, 4), 8, 4));
}

#[test]
fn snap_cockpit_80x24() {
    insta::assert_snapshot!(map(&layout(&cockpit(), 80, 24), 80, 24));
}

#[test]
fn snap_cockpit_180x50() {
    insta::assert_snapshot!(map(&layout(&cockpit(), 180, 50), 180, 50));
}

#[test]
fn snap_cockpit_resized_from_80x24_to_180x50_and_back() {
    let mut tree = LayoutTree::new();
    let root = cockpit();
    tree.compute(&root, 80, 24).expect("80x24");
    tree.compute(&root, 180, 50).expect("180x50");
    let again = tree.compute(&root, 80, 24).expect("80x24 again");
    insta::assert_snapshot!(map(&again, 80, 24));
}

#[test]
fn snap_deep_nesting_reports_depth() {
    let root = Node::column().id(id(0)).child(
        Node::row().id(id(1)).grow(1.0).child(
            Node::column()
                .id(id(2))
                .grow(1.0)
                .padding(1)
                .child(Node::leaf().id(id(3)).grow(1.0)),
        ),
    );
    insta::assert_snapshot!(map(&layout(&root, 12, 5), 12, 5));
}

/// The bug shape: a 60-row stream in a body on a 40-row screen.
/// The viewport lets the body shrink to the 38 rows between header and
/// footer instead of growing to its content and pushing the footer off.
fn stream_screen() -> Node {
    Node::column()
        .child(Node::leaf().id(id(0)).height(Length::Cells(1)))
        .child(
            Node::column()
                .id(id(1))
                .grow(1.0)
                .viewport()
                .children((0..60).map(|_| Node::leaf().height(Length::Cells(1)).shrink(0.0))),
        )
        .child(Node::leaf().id(id(2)).height(Length::Cells(1)))
}

#[test]
fn snap_a_viewport_keeps_the_footer_on_screen() {
    insta::assert_snapshot!(map(&layout(&stream_screen(), 120, 40), 120, 40));
}

#[test]
fn a_viewport_reports_its_content_extent() {
    let resolved = layout(&stream_screen(), 120, 40);
    assert_eq!(resolved.rect(id(1)), Some(Rect::new(0, 1, 120, 38)));
    assert_eq!(
        resolved.content_extent(id(1)),
        Some(Extent {
            width: 120,
            height: 60
        })
    );
    // A node whose content fits reports its own size as the extent.
    assert_eq!(
        resolved.content_extent(id(2)),
        Some(Extent {
            width: 120,
            height: 1
        })
    );
    assert_eq!(resolved.content_extent(id(9)), None);
}

#[test]
fn snap_root_without_ids_paints_nothing() {
    let root = Node::row().child(Node::leaf().grow(1.0));
    insta::assert_snapshot!(map(&layout(&root, 5, 3), 5, 3));
}

#[test]
fn snap_column_with_fixed_and_percent_mix() {
    let root = Node::column()
        .gap(1)
        .child(Node::leaf().id(id(0)).height(Length::Cells(2)))
        .child(Node::leaf().id(id(1)).height(Length::Percent(0.25)))
        .child(
            Node::leaf()
                .id(id(2))
                .grow(1.0)
                .min_height(Length::Cells(1)),
        )
        .child(
            Node::leaf()
                .id(id(3))
                .height(Length::Cells(3))
                .max_height(Length::Cells(2)),
        );
    insta::assert_snapshot!(map(&layout(&root, 6, 16), 6, 16));
}

#[test]
fn a_frame_insets_children_by_one_cell_on_every_edge() {
    use crate::layout::Frame;
    use crate::render::Style;

    let root = Node::column()
        .id(id(0))
        .frame(Frame::new(Style::new()))
        .child(Node::leaf().id(id(1)).grow(1.0));
    let resolved = layout(&root, 10, 6);
    assert_eq!(resolved.rect(id(0)), Some(Rect::new(0, 0, 10, 6)));
    assert_eq!(resolved.rect(id(1)), Some(Rect::new(1, 1, 8, 4)));
}

#[test]
fn a_frame_composes_with_padding_inside_the_ring() {
    use crate::layout::Frame;
    use crate::render::Style;

    let root = Node::column()
        .id(id(0))
        .frame(Frame::new(Style::new()))
        .padding(1)
        .child(Node::leaf().id(id(1)).grow(1.0));
    let resolved = layout(&root, 10, 6);
    assert_eq!(resolved.rect(id(1)), Some(Rect::new(2, 2, 6, 2)));
}

#[test]
fn snap_an_overlay_is_inset_from_the_edges_it_names() {
    // A toast in the top-right: two edges named, two left `Auto`, and a
    // size of its own. It costs the body nothing.
    let root = Node::column()
        .child(Node::leaf().id(id(0)).grow(1.0))
        .child(
            Node::leaf()
                .id(id(1))
                .overlay(Inset::none().top(Length::Cells(1)).right(Length::Cells(2)))
                .width(Length::Cells(12))
                .height(Length::Cells(3)),
        );
    insta::assert_snapshot!(map(&layout(&root, 40, 10), 40, 10));
}

#[test]
fn snap_an_overlay_stretches_between_two_opposite_edges() {
    // Both edges of an axis named and no width: CSS's `left: 0; right: 0`.
    let root = Node::column()
        .child(Node::leaf().id(id(0)).grow(1.0))
        .child(
            Node::leaf()
                .id(id(1))
                .overlay(Inset::all(3))
                .height(Length::Cells(4)),
        );
    insta::assert_snapshot!(map(&layout(&root, 30, 10), 30, 10));
}

#[test]
fn snap_a_centered_modal_over_the_cockpit() {
    let modal = Node::leaf()
        .id(id(8))
        .width(Length::Cells(30))
        .height(Length::Cells(7));
    let root = cockpit().child(Node::centered_overlay(modal).id(id(7)));
    insta::assert_snapshot!(map(&layout(&root, 80, 24), 80, 24));
}

#[test]
fn snap_a_footer_right_aligns_its_items() {
    let root = Node::column()
        .child(Node::leaf().id(id(0)).grow(1.0))
        .child(
            Node::row()
                .id(id(1))
                .height(Length::Cells(1))
                .gap(1)
                .justify_content(Justify::End)
                .child(Node::leaf().id(id(2)).width(Length::Cells(8)))
                .child(Node::leaf().id(id(3)).width(Length::Cells(6))),
        );
    insta::assert_snapshot!(map(&layout(&root, 40, 6), 40, 6));
}

#[test]
fn snap_align_items_centres_children_on_the_cross_axis() {
    // The default is CSS's `stretch`: the sized child keeps its height
    // and the unsized one fills the row. `Center` lets both keep their
    // own and sits them in the middle.
    let row = |align| {
        Node::row()
            .align_items(align)
            .gap(2)
            .child(
                Node::leaf()
                    .id(id(0))
                    .width(Length::Cells(8))
                    .height(Length::Cells(2)),
            )
            .child(
                Node::leaf()
                    .id(id(1))
                    .width(Length::Cells(8))
                    .height(Length::Cells(4)),
            )
            .child(Node::leaf().id(id(2)).grow(1.0))
    };
    let shown = format!(
        "stretch (the default)\n{}\ncentre\n{}",
        map(&layout(&row(Align::Stretch), 30, 6), 30, 6),
        map(&layout(&row(Align::Center), 30, 6), 30, 6)
    );
    insta::assert_snapshot!(shown);
}

#[test]
fn snap_a_zero_basis_stops_wide_text_pushing_its_siblings() {
    // `grow(1.0)` alone keeps CSS's auto basis, so the stage asks for its
    // text's width first and the dock gives way. `basis(Cells(0))` is
    // `flex: 1 1 0`: ask for nothing, take a share of what is spare.
    let text = "The stage shows the agent's stream: plan, execute, verify.";
    let stage = |basis| {
        Node::column()
            .id(id(0))
            .grow(1.0)
            .basis(basis)
            .child(Node::text(text))
    };
    let dock = || Node::column().id(id(1)).width(Length::Percent(0.3));
    let auto = Node::row().child(stage(Length::Auto)).child(dock());
    let zero = Node::row().child(stage(Length::Cells(0))).child(dock());
    let shown = format!(
        "auto basis\n{}\nzero basis\n{}",
        map(&layout(&auto, 60, 4), 60, 4),
        map(&layout(&zero, 60, 4), 60, 4)
    );
    insta::assert_snapshot!(shown);
}

#[test]
fn snap_an_overlay_escapes_its_parents_clip() {
    // A tooltip anchored in a one-row footer. Clipped to its parent it
    // would be one row tall; an overlay answers to the screen instead.
    let root = Node::column()
        .child(Node::leaf().id(id(0)).grow(1.0))
        .child(
            Node::row().id(id(1)).height(Length::Cells(1)).child(
                Node::leaf()
                    .id(id(2))
                    .overlay(
                        Inset::none()
                            .left(Length::Cells(2))
                            .bottom(Length::Cells(0)),
                    )
                    .width(Length::Cells(14))
                    .height(Length::Cells(5)),
            ),
        );
    insta::assert_snapshot!(map(&layout(&root, 30, 8), 30, 8));
}

#[test]
fn an_overlay_takes_no_space_from_its_siblings() {
    let sibling = Node::leaf().id(id(1)).grow(1.0);
    let plain = Node::row()
        .child(sibling.clone())
        .child(Node::leaf().id(id(2)).width(Length::Cells(20)));
    let floated = Node::row().child(sibling).child(
        Node::leaf()
            .id(id(2))
            .overlay(Inset::none())
            .width(Length::Cells(20)),
    );
    assert_eq!(
        layout(&plain, 40, 4).rect(id(1)),
        Some(Rect::new(0, 0, 20, 4)),
        "in flow the sibling gives up half the row"
    );
    assert_eq!(
        layout(&floated, 40, 4).rect(id(1)),
        Some(Rect::new(0, 0, 40, 4)),
        "out of flow it costs the sibling nothing"
    );
}

#[test]
fn a_nested_overlay_reports_the_layer_it_sits_on() {
    let root = Node::column().child(Node::leaf().id(id(0))).child(
        Node::centered_overlay(
            Node::column().id(id(1)).child(
                Node::leaf()
                    .id(id(2))
                    .overlay(Inset::none())
                    .width(Length::Cells(4))
                    .height(Length::Cells(1)),
            ),
        )
        .id(id(3)),
    );
    let resolved = layout(&root, 20, 6);
    let layers: Vec<(u32, u16)> = resolved
        .placed()
        .iter()
        .filter_map(|placed| placed.id.map(|w| (w.raw(), placed.layer)))
        .collect();
    assert_eq!(
        layers,
        vec![(0, 0), (3, 1), (1, 1), (2, 2)],
        "the modal's contents ride its layer; a toast over it is a layer up"
    );
}

// --- the unit basis -----------------------------------------------------

/// A tree with all four things a basis has to resolve per axis: sizes,
/// padding, a gap, an overlay inset and a framed ring.
fn measured_tree() -> Node {
    Node::column()
        .padding(1)
        .gap(2)
        .child(
            Node::row()
                .id(id(0))
                .height(Length::Cells(3))
                .frame(crate::layout::Frame::new(crate::render::Style::default()))
                .child(Node::text("alpha beta gamma").id(id(1)).grow(1.0)),
        )
        .child(
            Node::row()
                .id(id(2))
                .grow(1.0)
                .child(Node::leaf().id(id(3)).width(Length::Cells(10))),
        )
        .child(
            Node::leaf()
                .id(id(4))
                .overlay(Inset::none().top(Length::Cells(1)).right(Length::Cells(2)))
                .width(Length::Cells(4))
                .height(Length::Cells(2)),
        )
}

#[test]
fn the_cell_basis_and_the_cell_measurer_are_what_compute_already_did() {
    let root = measured_tree();
    let mut tree = LayoutTree::new();
    let cells = tree.compute(&root, 40, 12).expect("layout");
    let on = tree
        .compute_on(&root, 40.0, 12.0, Basis::CELLS, &mut CellMeasure)
        .expect("layout");
    assert_eq!(cells, on);
}

#[test]
fn a_placed_nodes_px_is_its_rect_on_the_cell_basis() {
    let root = measured_tree();
    let mut tree = LayoutTree::new();
    let resolved = tree.compute(&root, 40, 12).expect("layout");
    for placed in resolved.placed() {
        assert_eq!(
            placed.px.to_cells(Basis::CELLS),
            placed.rect,
            "px and rect disagree at depth {}",
            placed.depth
        );
    }
}

#[test]
fn a_window_basis_scales_every_axis_by_its_own_cell() {
    // The same tree on a 9x20 grid: every rect is the cell layout
    // multiplied out, because CellMeasure still makes a row one cell
    // tall and the shaper is what changes that, not the basis.
    let root = measured_tree();
    let basis = Basis::new(9.0, 20.0);
    let mut tree = LayoutTree::new();
    let cells = tree.compute(&root, 40, 12).expect("layout");
    let mut scaled = ScaledMeasure(basis);
    let px = tree
        .compute_on(&root, 40.0 * 9.0, 12.0 * 20.0, basis, &mut scaled)
        .expect("layout");
    assert_eq!(px.len(), cells.len());
    for (a, b) in px.placed().iter().zip(cells.placed()) {
        assert_eq!(a.rect, b.rect, "cell reading at depth {}", a.depth);
        assert_eq!(a.px.x, f32::from(b.rect.x) * 9.0);
        assert_eq!(a.px.y, f32::from(b.rect.y) * 20.0);
        assert_eq!(a.px.width, f32::from(b.rect.width) * 9.0);
        assert_eq!(a.px.height, f32::from(b.rect.height) * 20.0);
    }
}

/// [`CellMeasure`] with its answers multiplied out onto a basis — a
/// monospaced measurer for a pixel grid, which is what isolates the
/// basis from the shaper in the test above.
struct ScaledMeasure(Basis);

impl Measure for ScaledMeasure {
    fn intrinsic(&mut self, text: &TextMetrics) -> (f32, f32) {
        let (max, min) = CellMeasure.intrinsic(text);
        (max * self.0.x(), min * self.0.x())
    }

    fn wrap<'a>(&mut self, text: &'a TextMetrics, width: f32) -> Vec<&'a str> {
        CellMeasure.wrap(text, width / self.0.x())
    }

    fn line_height(&mut self) -> f32 {
        self.0.y()
    }
}

#[test]
fn a_taller_row_makes_a_taller_leaf_and_the_terminal_never_sees_it() {
    // The same text leaf is one cell tall in cells and as tall as its
    // measurer's own line box in pixels.
    let root = Node::column().child(Node::text("one two").id(id(0)));
    let mut tree = LayoutTree::new();
    let cells = tree.compute(&root, 20, 6).expect("layout");
    assert_eq!(cells.rect(id(0)).map(|r| r.height), Some(1));

    let basis = Basis::new(9.0, 20.0);
    let px = tree
        .compute_on(&root, 180.0, 120.0, basis, &mut TallRows)
        .expect("layout");
    assert_eq!(px.px(id(0)).map(|r| r.height), Some(18.0));
}

/// A measurer with a row height of its own, which is what a shaper's
/// is.
struct TallRows;

impl Measure for TallRows {
    fn intrinsic(&mut self, text: &TextMetrics) -> (f32, f32) {
        (f32::from(text.max_content_width()) * 9.0, 9.0)
    }

    fn wrap<'a>(&mut self, text: &'a TextMetrics, width: f32) -> Vec<&'a str> {
        text.wrap((width / 9.0).floor().max(0.0) as u16)
    }

    fn line_height(&mut self) -> f32 {
        18.0
    }
}
