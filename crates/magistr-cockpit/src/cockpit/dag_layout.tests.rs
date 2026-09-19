use super::DagLayout;

/// The unit-grid scale the roadmap paints at: six cells per rank, two
/// per row — what makes a distance in `check_clear` comparable to a dot.
const COL: f32 = 6.0;
const ROW: f32 = 2.0;

fn layout(ranks: &[usize], edges: &[(usize, usize)]) -> DagLayout {
    let order: Vec<usize> = (0..ranks.len()).collect();
    DagLayout::new(ranks, edges, &order)
}

/// Longest-path ranks over `edges` for `count` vertices — what the
/// roadmap derives from dependency depth.
fn depths(count: usize, edges: &[(usize, usize)]) -> Vec<usize> {
    let mut ranks = vec![0; count];
    let mut moved = true;
    let mut guard = 0;
    while moved && guard < count + 1 {
        moved = false;
        guard += 1;
        for &(from, to) in edges {
            if ranks[to] < ranks[from] + 1 {
                ranks[to] = ranks[from] + 1;
                moved = true;
            }
        }
    }
    ranks
}

/// The cubic the painter draws between two consecutive route points,
/// sampled at `t`, in scaled coordinates.
fn bezier(a: (f32, f32), b: (f32, f32), t: f32) -> (f32, f32) {
    let (x0, y0) = (a.0 * COL, a.1 * ROW);
    let (x3, y3) = (b.0 * COL, b.1 * ROW);
    let mid = f32::midpoint(x0, x3);
    let u = 1.0 - t;
    let x = u * u * u * x0 + 3.0 * u * u * t * mid + 3.0 * u * t * t * mid + t * t * t * x3;
    let y = u * u * u * y0 + 3.0 * u * u * t * y0 + 3.0 * u * t * t * y3 + t * t * t * y3;
    (x, y)
}

/// Every invariant the module documents, checked on one layout: ranks
/// honoured, paths monotone one rank at a time, no two vertices on a
/// rank closer than the gap, every routing point clear of every real
/// vertex on its rank, nothing above the top, and — sampling the very
/// curve the painter strokes — no edge passing through a node body it
/// does not end on.
fn check_invariants(layout: &DagLayout, ranks: &[usize]) {
    assert_eq!(layout.real, ranks.len());
    for (at, &rank) in ranks.iter().enumerate() {
        assert_eq!(layout.rank(at), rank, "vertex {at} keeps its rank");
    }
    assert!(
        layout.min_gap() >= DagLayout::MIN_GAP - 1e-4,
        "vertices on one rank stay {} apart: {}",
        DagLayout::MIN_GAP,
        layout.min_gap()
    );
    for &(_, y) in &layout.points {
        assert!(y >= -1e-4, "nothing sits above the top: {y}");
    }
    for path in &layout.paths {
        assert_eq!(path.points.first(), Some(&layout.points[path.from]));
        assert_eq!(path.points.last(), Some(&layout.points[path.to]));
        for pair in path.points.windows(2) {
            if ranks[path.from] < ranks[path.to] {
                assert!(
                    (pair[1].0 - pair[0].0 - 1.0).abs() < 1e-4,
                    "a routed path steps one rank at a time: {pair:?}"
                );
            }
            for sample in 1..16 {
                let (sx, sy) = bezier(pair[0], pair[1], sample as f32 / 16.0);
                for (at, &(x, y)) in layout.points[..layout.real].iter().enumerate() {
                    if at == path.from || at == path.to {
                        continue;
                    }
                    let (dx, dy) = (sx - x * COL, sy - y * ROW);
                    assert!(
                        dx.mul_add(dx, dy * dy).sqrt() >= 0.9,
                        "edge {}->{} passes through vertex {at} at ({sx:.2},{sy:.2})",
                        path.from,
                        path.to
                    );
                }
            }
        }
        for &(x, y) in &path.points[1..path.points.len().saturating_sub(1)] {
            for (at, &(rx, ry)) in layout.points[..layout.real].iter().enumerate() {
                if (rx - x).abs() < 0.5 {
                    assert!(
                        (ry - y).abs() >= DagLayout::MIN_GAP - 1e-4,
                        "routing point ({x},{y}) sits on vertex {at}"
                    );
                }
            }
        }
    }
}

#[test]
fn an_empty_graph_lays_out_nothing() {
    let layout = layout(&[], &[]);
    assert!(layout.points.is_empty());
    assert!(layout.paths.is_empty());
    assert_eq!(layout.real, 0);
    assert_eq!(layout.min_gap(), f32::INFINITY);
    assert_eq!(layout.crossings(), 0);
}

#[test]
fn a_linear_chain_is_one_straight_row() {
    let ranks = [0, 1, 2, 3];
    let edges = [(0, 1), (1, 2), (2, 3)];
    let layout = layout(&ranks, &edges);
    check_invariants(&layout, &ranks);
    let ys: Vec<f32> = layout.points.iter().map(|p| p.1).collect();
    assert!(
        ys.iter().all(|&y| (y - ys[0]).abs() < 1e-4),
        "one row: {ys:?}"
    );
    assert_eq!(layout.crossings(), 0);
    assert!(layout.paths.iter().all(|p| p.points.len() == 2));
}

#[test]
fn one_branch_splits_into_two_rows_a_gap_apart() {
    let ranks = [0, 1, 1];
    let edges = [(0, 1), (0, 2)];
    let layout = layout(&ranks, &edges);
    check_invariants(&layout, &ranks);
    assert!((layout.points[1].1 - layout.points[2].1).abs() >= DagLayout::MIN_GAP);
    let parent = layout.points[0].1;
    assert!(
        (parent - f32::midpoint(layout.points[1].1, layout.points[2].1)).abs() < 1e-3,
        "the fork sits between its branches"
    );
    assert_eq!(layout.crossings(), 0);
}

#[test]
fn multiple_branches_keep_their_lanes_without_crossing() {
    let edges = [(0, 1), (0, 2), (0, 3), (1, 4), (2, 5), (3, 6)];
    let ranks = depths(7, &edges);
    let layout = layout(&ranks, &edges);
    check_invariants(&layout, &ranks);
    assert_eq!(layout.crossings(), 0);
    // Each branch continues on its own row.
    for (parent, child) in [(1, 4), (2, 5), (3, 6)] {
        assert!((layout.points[parent].1 - layout.points[child].1).abs() < 1e-3);
    }
}

#[test]
fn a_merge_joins_between_its_sources() {
    let edges = [(0, 2), (1, 2), (2, 3)];
    let ranks = depths(4, &edges);
    let layout = layout(&ranks, &edges);
    check_invariants(&layout, &ranks);
    assert_eq!(layout.crossings(), 0);
    let (a, b, join) = (layout.points[0].1, layout.points[1].1, layout.points[2].1);
    assert!(
        join >= a.min(b) && join <= a.max(b),
        "the join sits between its sources"
    );
}

#[test]
fn a_long_edge_is_routed_through_free_rows_on_every_rank_it_crosses() {
    // 4 is pinned three ranks out, so 0->4 crosses ranks 1 and 2, where
    // 1 and 2 already sit.
    let ranks = [0, 1, 2, 3, 3];
    let edges = [(0, 1), (1, 2), (2, 3), (0, 4)];
    let layout = layout(&ranks, &edges);
    check_invariants(&layout, &ranks);
    let long = layout
        .paths
        .iter()
        .find(|p| p.from == 0 && p.to == 4)
        .expect("routed");
    assert_eq!(long.points.len(), 4, "one routing point per crossed rank");
    assert_eq!(layout.points.len(), 7, "two routing vertices were added");
}

#[test]
fn an_implied_edge_gets_no_path_of_its_own() {
    let edges = [(0, 1), (1, 2), (0, 2)];
    let layout = layout(&[0, 1, 2], &edges);
    assert_eq!(layout.paths.len(), 2, "0->2 is the chain 0->1->2");
    assert!(layout.paths.iter().all(|p| !(p.from == 0 && p.to == 2)));
}

#[test]
fn the_sweeps_untangle_a_crossed_pair() {
    // In the caller's order 0 and 1 stack, then 2 and 3 — which crosses
    // 0->3 with 1->2 until the barycentre sweep swaps one layer.
    let layout = layout(&[0, 0, 1, 1], &[(0, 3), (1, 2)]);
    assert_eq!(layout.crossings(), 0);
    check_invariants(&layout, &[0, 0, 1, 1]);
}

#[test]
fn a_complete_bipartite_pair_keeps_its_one_unavoidable_crossing() {
    let layout = layout(&[0, 0, 1, 1], &[(0, 2), (0, 3), (1, 2), (1, 3)]);
    assert_eq!(layout.crossings(), 1);
    check_invariants(&layout, &[0, 0, 1, 1]);
}

#[test]
fn the_same_input_lays_out_identically_and_edge_order_does_not_move_vertices() {
    let edges = [(0, 1), (0, 2), (1, 3), (2, 3), (3, 4), (0, 4), (2, 5)];
    let ranks = depths(6, &edges);
    let first = layout(&ranks, &edges);
    let again = layout(&ranks, &edges);
    assert_eq!(first, again, "a repeated layout is byte-identical");
    let mut reversed = edges;
    reversed.reverse();
    let shuffled = layout(&ranks, &reversed);
    assert_eq!(
        first.points, shuffled.points,
        "edge order never moves a vertex"
    );
}

#[test]
fn the_caller_order_breaks_ties_between_disconnected_vertices() {
    let ranks = [0, 0, 0];
    let ordered = DagLayout::new(&ranks, &[], &[2, 0, 1]);
    assert!(ordered.points[2].1 < ordered.points[0].1);
    assert!(ordered.points[0].1 < ordered.points[1].1);
    check_invariants(&ordered, &ranks);
}

#[test]
fn compaction_pulls_a_vertex_toward_its_earliest_dependent_and_leaves_sinks_and_cycles() {
    let edges = [(0, 3), (1, 2), (2, 3)];
    let ranks = depths(4, &edges);
    assert_eq!(ranks, vec![0, 0, 1, 2]);
    assert_eq!(DagLayout::compact_ranks(&ranks, &edges), vec![1, 0, 1, 2]);
    assert_eq!(
        DagLayout::compact_ranks(&[0, 0], &[(0, 1), (1, 0)]),
        vec![0, 0],
        "a cycle is left alone"
    );
    assert_eq!(
        DagLayout::compact_ranks(&[0, 0, 0], &[(0, 1)]),
        vec![0, 0, 0],
        "an adjacent edge and an isolated vertex keep their ranks"
    );
    assert_eq!(DagLayout::compact_ranks(&[], &[]), Vec::<usize>::new());
}

#[test]
fn the_dense_harbor_fixture_keeps_every_invariant() {
    let (ranks, edges) = harbor();
    assert!(
        ranks.len() >= 50 && edges.len() >= 100,
        "the fixture is dense"
    );
    let layout = layout(&ranks, &edges);
    check_invariants(&layout, &ranks);
    assert_eq!(layout, layout_again(&ranks, &edges));
    let reduced = DagLayout::reduce_edges(edges.clone(), ranks.len());
    assert!(
        reduced.len() < edges.len(),
        "the overview drops implied edges"
    );
    // The crossings the sweeps settle on for this fixture, pinned as a
    // ceiling: a change to the ordering that tangles the dense graph
    // further fails here before it reaches a capture.
    assert!(
        layout.crossings() <= HARBOR_CROSSINGS_CEILING,
        "the dense fixture crossed {} times (ceiling {HARBOR_CROSSINGS_CEILING})",
        layout.crossings()
    );
}

/// Where the sweeps and the sifting leave the Harbor fixture today
/// (96), with a little room for a legitimately different fixture;
/// lower it when the layout improves. The barycentre sweeps alone
/// settled at 105 under the old ceiling of 110.
const HARBOR_CROSSINGS_CEILING: usize = 100;

fn layout_again(ranks: &[usize], edges: &[(usize, usize)]) -> DagLayout {
    layout(ranks, edges)
}

/// The rows of every vertex on `rank`, real and routing, top to bottom.
fn rows_on(layout: &DagLayout, rank: usize) -> Vec<f32> {
    let mut rows: Vec<f32> = layout
        .points
        .iter()
        .filter(|(x, _)| (*x - rank as f32).abs() < 0.5)
        .map(|&(_, y)| y)
        .collect();
    rows.sort_by(f32::total_cmp);
    rows
}

#[test]
fn a_layer_with_room_spreads_past_the_minimum_gap_without_growing_the_drawing() {
    // One root feeding a pair on the next rank and four vertices pinned
    // a rank further: every vertex wants the root's row, so without the
    // balancing the pair would sit at the minimum gap. The last rank is
    // the tallest at four vertices; the pair has three rows to share
    // and spreads to the cap, the tallest rank stays at the minimum,
    // and the drawing is no taller than that rank.
    let ranks = [0, 1, 1, 2, 2, 2, 2];
    let edges = [(0, 1), (0, 2), (1, 3), (1, 4), (1, 5), (1, 6)];
    let layout = layout(&ranks, &edges);
    check_invariants(&layout, &ranks);
    let middle = rows_on(&layout, 1);
    assert!(
        (middle[1] - middle[0] - DagLayout::MAX_GAP).abs() < 1e-4,
        "the pair with room sits a full cap apart: {middle:?}"
    );
    let last = rows_on(&layout, 2);
    for pair in last.windows(2) {
        assert!(
            (pair[1] - pair[0] - DagLayout::MIN_GAP).abs() < 1e-4,
            "the tallest rank stays at the minimum: {last:?}"
        );
    }
    let bottom = layout.points.iter().map(|p| p.1).fold(0.0, f32::max);
    assert!(
        (bottom - 3.0 * DagLayout::MIN_GAP).abs() < 1e-4,
        "the drawing is exactly the tallest rank tall: {bottom}"
    );
    assert_eq!(layout.crossings(), 0);
}

#[test]
fn the_dense_harbor_fixture_spreads_every_rank_that_has_room() {
    let (ranks, edges) = harbor();
    let layout = layout(&ranks, &edges);
    let tallest = (0..=ranks.iter().copied().max().unwrap_or(0))
        .map(|rank| rows_on(&layout, rank).len())
        .max()
        .unwrap_or(0);
    let height = (tallest - 1) as f32 * DagLayout::MIN_GAP;
    let mut spread = 0;
    for rank in 0..=ranks.iter().copied().max().unwrap_or(0) {
        let rows = rows_on(&layout, rank);
        if rows.len() < 2 {
            continue;
        }
        let want = (height / (rows.len() - 1) as f32).min(DagLayout::MAX_GAP);
        for pair in rows.windows(2) {
            assert!(
                pair[1] - pair[0] >= want - 1e-3,
                "rank {rank} ({} vertices) packs a pair at {} under its share {want}",
                rows.len(),
                pair[1] - pair[0]
            );
        }
        if want > DagLayout::MIN_GAP + 1e-3 {
            spread += 1;
        }
    }
    assert!(
        spread >= 10,
        "most of the fixture's ranks have room: {spread}"
    );
    // The balancing does not buy its room with height: the drawing is
    // no taller than the aligned layout was before it (27.5 rows).
    let bottom = layout.points.iter().map(|p| p.1).fold(0.0, f32::max);
    assert!(
        bottom <= 28.0,
        "the drawing stays within its height: {bottom}"
    );
}

/// The Harbor fixture's compacted ranks and dependency edges.
fn harbor() -> (Vec<usize>, Vec<(usize, usize)>) {
    let json = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../apps/website/fixtures/harbor.json"
    ))
    .expect("the website's synthetic fixture");
    let data: serde_json::Value = serde_json::from_str(&json).expect("valid json");
    let chunks = data["chunks"].as_array().expect("chunks");
    let ids: Vec<&str> = chunks
        .iter()
        .map(|c| c["id"].as_str().expect("id"))
        .collect();
    let mut edges = Vec::new();
    for (to, chunk) in chunks.iter().enumerate() {
        for dep in chunk["deps"].as_array().expect("deps") {
            let from = ids
                .iter()
                .position(|id| Some(*id) == dep.as_str())
                .expect("a dependency inside the fixture");
            edges.push((from, to));
        }
    }
    let ranks = DagLayout::compact_ranks(&depths(ids.len(), &edges), &edges);
    (ranks, edges)
}
