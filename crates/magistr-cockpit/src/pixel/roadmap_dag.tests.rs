use super::*;
use crate::cockpit::{DagNode, StarState};
use crate::layout::Rect;
use crate::pixel::{CellSize, PixelSurface};

fn node(x: f32, y: f32, status: &str, region: usize) -> DagNode {
    DagNode {
        x,
        y,
        state: StarState::of(status, false),
        status: status.to_owned(),
        region,
        here: false,
        selected: false,
    }
}

/// A small three-chunk tree: done -> in-progress -> pending, the last
/// on a second workstream, the middle one selected.
fn scene() -> RoadmapDag {
    let mut here = node(7.5, 1.5, "in_progress", 0);
    here.here = true;
    here.selected = true;
    RoadmapDag::new(
        vec![
            node(1.5, 1.5, "done", 0),
            here,
            node(13.5, 4.5, "pending", 1),
            node(1.5, 4.5, "blocked", 1),
            node(7.5, 4.5, "backlog", 1),
        ],
        vec![(0, 1), (1, 2)],
        (10, 20),
    )
}

fn render(dag: &RoadmapDag, cols: u16, rows: u16) -> PixelSurface {
    let mut surface = PixelSurface::new(Rect::new(0, 0, cols, rows), CellSize::new(10, 20));
    surface.paint(dag);
    surface
}

fn pixel(surface: &PixelSurface, x: u32, y: u32) -> [u8; 4] {
    let width = u32::from(surface.width());
    let at = usize::try_from((y * width + x) * 4).expect("index");
    let px = &surface.pixels()[at..at + 4];
    [px[0], px[1], px[2], px[3]]
}

#[test]
fn full_plan_highlights_only_recorded_edges_touching_the_selected_work() {
    let mut selected = node(4.0, 2.0, "pending", 0);
    selected.selected = true;
    let graph = RoadmapDag::new(
        vec![
            node(1.0, 2.0, "done", 0),
            selected,
            node(7.0, 2.0, "pending", 0),
            node(10.0, 2.0, "pending", 0),
        ],
        vec![(0, 1), (1, 2), (2, 3)],
        (10, 20),
    )
    .highlight_incident(151, 191, 255);
    let accent = Color::from_rgba8(151, 191, 255, 255);
    assert_eq!(graph.link_ink(0, 1), accent);
    assert_eq!(graph.link_ink(1, 2), accent);
    assert_ne!(
        graph.link_ink(2, 3),
        accent,
        "connected work is not automatically a direct dependency"
    );
    let rendered = render(&graph, 12, 5);
    assert!(pixel(&rendered, 25, 40)[2] > pixel(&rendered, 85, 40)[2]);
}

#[test]
fn light_roadmap_marks_retain_contrast_without_a_white_halo() {
    use magistr_theme::{ThemeColor, math::contrast_ratio};
    let graph = scene().light(true).background(248, 249, 255).bands(false);
    let paper = ThemeColor::new(248, 249, 255);
    for status in ["in_progress", "done", "blocked", "backlog", "pending"] {
        let color = graph.status_ink(status).to_color_u8();
        assert!(
            contrast_ratio(
                ThemeColor::new(color.red(), color.green(), color.blue()),
                paper
            ) >= 3.0
        );
    }
    for walked in [false, true] {
        let color = graph.edge_ink(walked, true).to_color_u8();
        let alpha = f32::from(color.alpha()) / 255.0;
        let blend = |ink: u8, ground: u8| {
            (f32::from(ink) * alpha + f32::from(ground) * (1.0 - alpha)).round() as u8
        };
        assert!(
            contrast_ratio(
                ThemeColor::new(
                    blend(color.red(), 248),
                    blend(color.green(), 249),
                    blend(color.blue(), 255)
                ),
                paper
            ) >= 3.0
        );
    }
    let a = render(&graph.clone().glow(0.0), 16, 6);
    let b = render(&graph.glow(0.5), 16, 6);
    assert_eq!(
        a.pixels(),
        b.pixels(),
        "light mode uses ink instead of a light halo"
    );
}

#[test]
fn visual_reduction_preserves_reachability_and_removes_only_redundant_lines() {
    let edges = vec![(0, 1), (1, 2), (0, 2), (2, 3), (0, 3), (1, 3), (4, 3)];
    let reduced = transitive_reduction(edges.clone(), 5);
    assert_eq!(reduced, vec![(0, 1), (1, 2), (2, 3), (4, 3)]);
    let reachable = |edges: &[(usize, usize)], from: usize, to: usize| {
        let mut seen = [false; 5];
        let mut stack = vec![from];
        while let Some(at) = stack.pop() {
            if at == to {
                return true;
            }
            if !seen[at] {
                seen[at] = true;
                stack.extend(edges.iter().filter_map(|&(a, b)| (a == at).then_some(b)));
            }
        }
        false
    };
    for from in 0..5 {
        for to in 0..5 {
            assert_eq!(reachable(&edges, from, to), reachable(&reduced, from, to));
        }
    }
}

#[test]
fn visual_reduction_keeps_cyclic_input_and_is_order_independent() {
    let cycle = vec![(0, 1), (1, 2), (2, 0), (0, 2)];
    assert_eq!(
        transitive_reduction(cycle, 3),
        vec![(0, 1), (0, 2), (1, 2), (2, 0)]
    );
    assert_eq!(
        transitive_reduction(vec![(0, 2), (1, 2), (0, 1), (0, 1), (9, 0)], 3),
        vec![(0, 1), (1, 2)]
    );
}

#[test]
fn a_selection_lights_the_chain_to_its_relations_and_dims_the_rest() {
    let mut nodes = vec![
        node(1.5, 1.5, "done", 0),
        node(7.5, 1.5, "done", 0),
        node(13.5, 1.5, "pending", 0),
        node(7.5, 4.5, "pending", 1),
    ];
    let edges = vec![(0, 1), (1, 2), (0, 2)];
    let plain = RoadmapDag::new(nodes.clone(), edges.clone(), (10, 20));
    assert_eq!(plain.edges.len(), 2, "the overview keeps the reduction");
    assert_eq!(plain.focus, None);
    nodes[2].selected = true;
    let focused = RoadmapDag::new(nodes, edges, (10, 20));
    assert_eq!(
        focused.edges.len(),
        2,
        "a selection adds no line through the chain"
    );
    assert_eq!(
        focused.focus,
        Some(vec![true, true, true, false]),
        "the chain to the selection is the focus; the stray node is not"
    );
    let surface = render(&focused, 16, 6);
    let stray = pixel(&surface, 75, 90);
    let lit = pixel(&surface, 75, 30);
    let brightness = |[r, g, b, _]: [u8; 4]| u16::from(r) + u16::from(g) + u16::from(b);
    assert!(
        brightness(stray) < brightness(lit),
        "the unrelated dot fades back: {stray:?} vs {lit:?}"
    );
}

#[test]
fn a_waiting_chunk_is_hollow_and_a_ready_one_is_filled() {
    let mut ready = node(7.5, 1.5, "pending", 0);
    ready.state = StarState::of("pending", true);
    let waiting = node(1.5, 1.5, "pending", 0);
    let dag = RoadmapDag::new(vec![waiting, ready], Vec::new(), (10, 20)).bands(false);
    let surface = render(&dag, 12, 3);
    let [r, g, b, _] = pixel(&surface, 15, 30);
    assert_eq!(
        (r, g, b),
        (26, 27, 38),
        "the waiting dot's centre is ground"
    );
    let [r, g, b, _] = pixel(&surface, 75, 30);
    assert!(b > r && b > g, "the ready dot is filled blue: {r},{g},{b}");
}

#[test]
fn the_rail_can_paint_without_bands() {
    let surface = render(&scene().bands(false), 16, 6);
    assert_eq!(pixel(&surface, 155, 30), pixel(&surface, 155, 90));
    let [r, g, b, _] = pixel(&surface, 155, 30);
    assert_eq!((r, g, b), (26, 27, 38), "no band, only ground");
}

#[test]
fn dots_wear_their_status_colours() {
    let surface = render(&scene(), 16, 6);
    // Done dot at cell (1.5, 1.5) -> pixel (15, 30): green.
    let [r, g, b, _] = pixel(&surface, 15, 30);
    assert!(g > r && g > b, "done is green: {r},{g},{b}");
    // Pending dot at (13.5, 4.5) -> (135, 90): blue.
    let [r, g, b, _] = pixel(&surface, 135, 90);
    assert!(b > r && b > g, "pending is blue: {r},{g},{b}");
    // Blocked dot at (1.5, 4.5) -> (15, 90): red.
    let [r, g, b, _] = pixel(&surface, 15, 90);
    assert!(r > g && r > b, "blocked is red: {r},{g},{b}");
}

#[test]
fn the_selection_ring_and_the_edge_light_their_pixels() {
    let surface = render(&scene(), 16, 6);
    // The selected in-progress node at (7.5, 1.5) -> (75, 30). Its white
    // ring sits past the dot radius (7.6px) and the here-ring gap.
    let on_ring = (0..=14)
        .map(|offset| pixel(&surface, 75 + 10 + offset, 30))
        .any(|[r, g, b, _]| r > 220 && g > 220 && b > 220);
    assert!(on_ring, "the selection ring is white");
    // An edge crosses between the done and the in-progress dots.
    let lit = (25..=65).any(|x| {
        let [r, g, b, _] = pixel(&surface, x, 30);
        u16::from(r) + u16::from(g) + u16::from(b) > 150
    });
    assert!(lit, "the dependency edge is drawn");
}

#[test]
fn regions_tint_their_bands_differently() {
    let surface = render(&scene(), 16, 6);
    // Away from nodes and edges, the two bands differ by their tints.
    let band0 = pixel(&surface, 155, 30);
    let band1 = pixel(&surface, 155, 90);
    assert_ne!(band0, band1, "each workstream wears its own tint");
}

#[test]
fn an_empty_tree_is_background_only() {
    let dag = RoadmapDag::new(Vec::<DagNode>::new(), Vec::new(), (10, 20));
    let surface = render(&dag, 4, 2);
    let [r, g, b, _] = pixel(&surface, 10, 10);
    assert_eq!((r, g, b), (26, 27, 38));
}

/// The pixel-diff gate uses the shared golden review flow. A mismatch
/// writes a candidate; an intended change is blessed explicitly.
#[test]
fn the_scene_matches_the_golden_png() {
    let surface = render(&scene(), 16, 6);
    let pixmap = tiny_skia::Pixmap::from_vec(
        surface.pixels().to_vec(),
        tiny_skia::IntSize::from_wh(u32::from(surface.width()), u32::from(surface.height()))
            .expect("size"),
    )
    .expect("pixmap");
    crate::golden::Golden::new(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden"),
    )
    .assert_matches("roadmap_dag", &pixmap);
}

/// the chunk in progress carries a soft halo — bright near the
/// dot, gone by the glow's reach — and a tree with nothing in progress
/// has none.
#[test]
fn the_active_chunk_glows_and_nothing_else_does() {
    let surface = render(&scene(), 16, 6);
    // The here-node sits at cell (7.5, 1.5) -> pixel (75, 30); its dot
    // radius is capped at 7.6 px, so the glow reaches ~27 px.
    let sum = |surface: &PixelSurface, x: u32, y: u32| {
        let [r, g, b, _] = pixel(surface, x, y);
        u16::from(r) + u16::from(g) + u16::from(b)
    };
    // Straight up from the dot: inside the halo, then at its edge.
    let near = sum(&surface, 75, 12);
    let far = sum(&surface, 75, 3);
    assert!(near > far + 20, "the halo fades outward: {near} vs {far}");
    // The same pixel with nothing in progress is the halo's whole
    // contribution: background and band tint alone.
    let mut quiet = scene();
    for node in &mut quiet.nodes {
        node.here = false;
    }
    let unlit = sum(&render(&quiet, 16, 6), 75, 12);
    assert!(near > unlit + 20, "no `here`, no glow: {near} vs {unlit}");
}

// ---------------------------------------------------------------------
// the active chunk's glow breathes.
// ---------------------------------------------------------------------

/// The sum of the halo's channels at a pixel just outside the settled
/// glow's reach — where a swell shows up first and a dot cannot.
fn halo_edge(dag: &RoadmapDag) -> u16 {
    let surface = render(dag, 16, 6);
    let [r, g, b, _] = pixel(&surface, 75, 5);
    u16::from(r) + u16::from(g) + u16::from(b)
}

#[test]
fn the_foot_of_the_breath_is_exactly_the_still_the_glow_used_to_be() {
    let settled = render(&scene(), 16, 6);
    for phase in [0.0, 1.0] {
        let breathing = render(&scene().glow(phase), 16, 6);
        assert_eq!(
            breathing.pixels(),
            settled.pixels(),
            "phase {phase} must be the frame the still pinned"
        );
    }
}

#[test]
fn the_crest_of_the_breath_reaches_further_and_burns_brighter() {
    let foot = halo_edge(&scene());
    let crest = halo_edge(&scene().glow(0.5));
    assert!(
        crest > foot,
        "the halo must swell past where it settles: {foot} -> {crest}"
    );
    // And it comes back: a quarter and three quarters through are the
    // same point of the raised cosine.
    assert_eq!(
        halo_edge(&scene().glow(0.25)),
        halo_edge(&scene().glow(0.75))
    );
    assert!(halo_edge(&scene().glow(0.25)) < crest, "and peaks halfway");
}

#[test]
fn a_tree_with_nothing_in_progress_does_not_breathe() {
    let mut quiet = scene();
    for node in &mut quiet.nodes {
        node.here = false;
    }
    let foot = render(&quiet, 16, 6);
    let crest = render(&quiet.clone().glow(0.5), 16, 6);
    assert_eq!(
        foot.pixels(),
        crest.pixels(),
        "no `here`, no wave — which is why the driver leaves the phase out of its key"
    );
}
