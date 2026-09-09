use super::*;
use crate::layout::Rect;
use crate::pixel::{CellSize, PixelSurface};

/// The RGBA buffer as rows of glyphs by luminance — the same coarse
/// scale the sparkline snapshots use.
fn ascii(surface: &PixelSurface) -> String {
    let width = usize::from(surface.width());
    surface
        .pixels()
        .chunks(4)
        .map(|px| {
            let lum =
                (u32::from(px[0]) * 299 + u32::from(px[1]) * 587 + u32::from(px[2]) * 114) / 1000;
            match lum {
                0..=40 => ' ',
                41..=110 => '.',
                111..=180 => '+',
                _ => '#',
            }
        })
        .collect::<Vec<char>>()
        .chunks(width)
        .map(|row| row.iter().collect::<String>())
        .collect::<Vec<String>>()
        .join("\n")
}

fn render(graph: &BranchGraph, cols: u16, rows: u16) -> PixelSurface {
    let mut surface = PixelSurface::new(Rect::new(0, 0, cols, rows), CellSize::new(4, 8));
    surface.paint(graph);
    surface
}

#[test]
fn snap_a_fork_draws_two_lanes_and_the_link() {
    // Steps 1 2 on main, 3 4 on a branch forked from index 1, 5 on main.
    let graph = BranchGraph::new(vec![0, 0, 1, 1, 0], vec![(1, 2)]);
    insta::assert_snapshot!(ascii(&render(&graph, 16, 3)));
}

#[test]
fn a_fork_lights_both_lanes() {
    let graph = BranchGraph::new(vec![0, 0, 1, 1, 0], vec![(1, 2)]);
    let surface = render(&graph, 16, 3);
    let art = ascii(&surface);
    let rows: Vec<&str> = art.lines().collect();
    let lit = |row: &str| row.trim().len();
    assert!(
        lit(rows[rows.len() / 3]) > 0,
        "the main lane is drawn: {art}"
    );
    assert!(
        lit(rows[rows.len() * 2 / 3]) > 0,
        "the branch lane is drawn: {art}"
    );
}

#[test]
fn a_single_step_is_one_dot_and_no_steps_is_background_only() {
    let one = render(&BranchGraph::new(vec![0], Vec::new()), 4, 2);
    let art = ascii(&one);
    assert!(art.contains('#') || art.contains('+'), "a dot shows: {art}");
    let none = render(&BranchGraph::new(Vec::<u8>::new(), Vec::new()), 4, 2);
    assert!(
        none.pixels().chunks(4).all(|px| px == [26, 27, 38, 255]),
        "background only"
    );
}

#[test]
fn out_of_range_links_are_ignored() {
    let graph = BranchGraph::new(vec![0, 1], vec![(0, 9)]);
    let art = ascii(&render(&graph, 8, 2));
    assert!(!art.trim().is_empty(), "the dots still draw: {art}");
}

/// the sweep reveals the graph from the
/// left, whole steps at a time, and at `1` (or `None`) the frame is the
/// graph the tick never touched — which is what keeps a settled Trace
/// tab byte-identical to the frame before the sweep existed.
#[test]
fn a_sweep_reveals_the_graph_from_the_left_and_ends_whole() {
    let graph = || BranchGraph::new(vec![0, 0, 1, 1, 0], vec![(1, 2)]);
    let lit = |surface: &PixelSurface| {
        ascii(surface)
            .chars()
            .filter(|c| !c.is_whitespace())
            .count()
    };
    let whole = render(&graph(), 16, 3);
    let half = render(&graph().sweep(Some(0.5)), 16, 3);
    assert!(lit(&half) < lit(&whole), "{} < {}", lit(&half), lit(&whole));
    assert!(lit(&half) > 0, "the revealed part is drawn");

    // Nothing is drawn past the edge, give or take the dot's radius.
    let width = usize::from(half.width());
    for row in ascii(&half).lines() {
        let tail: String = row.chars().skip(width * 7 / 10).collect();
        assert!(tail.trim().is_empty(), "drawn past the edge: {row:?}");
    }

    let done = render(&graph().sweep(Some(1.0)), 16, 3);
    assert_eq!(done.pixels(), whole.pixels(), "1 is the whole graph");
    let never = render(&graph().sweep(None), 16, 3);
    assert_eq!(never.pixels(), whole.pixels(), "and so is no sweep at all");
    let none = render(&graph().sweep(Some(0.0)), 16, 3);
    assert_eq!(lit(&none), 0, "0 is the background alone");

    // Frames grow monotonically as the sweep advances.
    let mut last = 0;
    for step in 0..=10 {
        let count = lit(&render(&graph().sweep(Some(step as f32 / 10.0)), 16, 3));
        assert!(count >= last, "step {step}: {count} < {last}");
        last = count;
    }
}

/// A fork edge belongs to the step it lands on: while that step is still
/// behind the sweep's edge, the link is not drawn either.
#[test]
fn a_fork_edge_waits_for_the_step_it_lands_on() {
    let graph = BranchGraph::new(vec![0, 0, 1, 1, 0], vec![(1, 2)]);
    let early = render(&graph.clone().sweep(Some(0.3)), 16, 3);
    // Lane 1 sits on the lower half; nothing is on it before its first
    // step is revealed.
    let art = ascii(&early);
    let rows: Vec<&str> = art.lines().collect();
    let lower = rows[rows.len() / 2..].join("");
    assert!(lower.trim().is_empty(), "the branch drew too early: {art}");
}
