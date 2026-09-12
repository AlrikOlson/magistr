use magistr_engine::NativeEvent;
use magistr_engine::native::{Chunk, Signal};
use serde_json::json;

use super::*;
use crate::cockpit::Eased;
use crate::harness::fixtures;

/// The record for a chunk of `status` and `priority` in `group` standing
/// on `deps`, as the wire would carry it — the store's real statuses,
/// plus `stuck` for work in progress that waits on a person.
fn chunk_json(
    id: &str,
    status: &str,
    priority: u32,
    group: &str,
    deps: &[&str],
) -> serde_json::Value {
    let mut chunk = json!({
        "id": id,
        "name": id,
        "title": format!("Chunk {id}"),
        "status": status,
        "priority": priority,
        "group": group,
        "deps": deps,
        "description": "desc",
        "acceptance": ["works"],
        "cross_refs": [],
    });
    if status == "stuck" {
        chunk["status"] = json!("in_progress");
        chunk["blocked_by"] = json!({"kind": "awaiting_human", "reason": "needs a decision"});
    }
    chunk
}

fn chunk(id: &str, status: &str, priority: u32, group: &str, deps: &[&str]) -> Chunk {
    fixtures::chunk_from(chunk_json(id, status, priority, group, deps))
}

fn signal(id: &str, status: &str) -> Signal {
    fixtures::signal_from(json!({
        "id": id,
        "kind": "idea",
        "from": "alice",
        "body": "make it faster",
        "status": status,
        "enrichment": [{"confidence": 0.8}, {"confidence": 0.4}],
    }))
}

fn frame(family: &str, kind: &str, key: &str, payload: serde_json::Value) -> NativeEvent {
    fixtures::frame(family, kind, json!({ key: payload }))
}

/// The roadmap event of `kind` carrying `chunk`.
fn chunk_frame(kind: &str, chunk: Chunk) -> NativeEvent {
    frame(
        "roadmap",
        kind,
        "chunk",
        serde_json::to_value(chunk).expect("a chunk serializes"),
    )
}

/// The signal event of `kind` carrying `signal`.
fn signal_frame(kind: &str, signal: Signal) -> NativeEvent {
    frame(
        "signal",
        kind,
        "signal",
        serde_json::to_value(signal).expect("a signal serializes"),
    )
}

/// A dependency chain `c0 → c1 → …`, wide enough to overflow a small
/// graph block.
fn chained(n: usize) -> Roadmap {
    let ids: Vec<String> = (0..n).map(|i| format!("c{i}")).collect();
    let chunks: Vec<Chunk> = ids
        .iter()
        .enumerate()
        .map(|(i, id)| {
            let deps: Vec<&str> = if i == 0 {
                vec![]
            } else {
                vec![ids[i - 1].as_str()]
            };
            chunk(id, "pending", 100, "Core", &deps)
        })
        .collect();
    let mut roadmap = Roadmap::default();
    roadmap.seed(&chunks, &[]);
    roadmap
}

fn seeded() -> Roadmap {
    let mut roadmap = Roadmap::default();
    roadmap.seed(
        &[
            chunk("base", "done", 100, "Core", &[]),
            chunk("mid", "in_progress", 100, "Core", &["base"]),
            chunk("ui", "pending", 200, "Views", &["mid"]),
            chunk("old", "obsoleted", 300, "Views", &[]),
        ],
        &[signal("s1", "researched"), signal("s2", "dismissed")],
    );
    roadmap
}

#[test]
fn a_seed_lists_the_working_set_and_collapses_done_to_a_count() {
    let roadmap = seeded();
    let ids: Vec<&str> = roadmap.rows().iter().map(|c| c.id.as_str()).collect();
    assert_eq!(ids, vec!["mid", "ui"], "done and obsoleted take no rows");
    assert_eq!(roadmap.done_count(), 1);
    assert_eq!(
        roadmap.summary_line(),
        "1 done · 1 active · 1 pending · 0 backlog · 1 signals"
    );
    let signals: Vec<&str> = roadmap.signals().iter().map(|s| s.id.as_str()).collect();
    assert_eq!(signals, vec!["s1"], "terminal signals left the inbox");
    assert_eq!(roadmap.signals()[0].confidence, Some(0.8), "max enrichment");
}

#[test]
fn a_viewport_fits_the_tree_and_the_floor_overflows_into_the_pan() {
    let mut roadmap = seeded();
    // Plenty of room: the fit caps the pitch and everything is inside.
    roadmap.set_viewport(Some((60, 12)));
    let nodes = roadmap.dag_nodes();
    for node in &nodes {
        assert!(node.x >= 0.0 && node.x <= 60.0, "{node:?} inside");
        assert!(node.y >= 0.0 && node.y <= 12.0, "{node:?} inside");
    }
    let pitch = nodes[1].x - nodes[0].x;
    assert!(
        pitch <= 15.0 + f32::EPSILON,
        "the cap keeps a small tree from stretching: {pitch}"
    );
    roadmap.pan_by(-100.0, -100.0);
    assert_eq!(roadmap.pan(), (0.0, 0.0), "a fitted tree cannot pan");
    // A tiny viewport: the pitch floor overflows and the pan takes over.
    roadmap.set_viewport(Some((8, 4)));
    let pitch = {
        let nodes = roadmap.dag_nodes();
        nodes[1].x - nodes[0].x
    };
    assert!(
        (pitch - 3.0).abs() < 0.01,
        "the floor holds the pitch clickable: {pitch}"
    );
    roadmap.pan_by(-100.0, 0.0);
    let (pan_x, _) = roadmap.pan();
    assert!(pan_x > 0.0, "the overflow pans");
    // Back to a roomy viewport: the pan re-clamps to the new limits.
    roadmap.set_viewport(Some((60, 12)));
    assert_eq!(roadmap.pan(), (0.0, 0.0));
}

#[test]
fn frames_upsert_chunks_and_signals() {
    let mut roadmap = seeded();
    // A change to a known chunk replaces it.
    roadmap.apply(
        &chunk_frame(
            "chunk_changed",
            chunk("ui", "in_progress", 200, "Views", &["mid"]),
        ),
        0.0,
        false,
    );
    assert_eq!(
        roadmap.chunk("ui").map(|c| c.status.as_str()),
        Some("in_progress")
    );
    // A new chunk appends.
    roadmap.apply(
        &chunk_frame("chunk_added", chunk("new", "pending", 300, "Views", &[])),
        0.0,
        false,
    );
    assert!(roadmap.chunk("new").is_some());
    // A dismissal empties the inbox.
    roadmap.apply(
        &signal_frame("signal_dismissed", signal("s1", "dismissed")),
        0.0,
        false,
    );
    assert!(roadmap.signals().is_empty());
    // Unknown families are ignored.
    roadmap.apply(
        &frame("think", "step_appended", "step", json!({})),
        0.0,
        false,
    );
}

#[test]
fn row_toggle_opens_one_detail_at_a_time() {
    let mut roadmap = seeded();
    roadmap.toggle_row(0, 0.0, false);
    assert_eq!(roadmap.selected().map(|c| c.id.as_str()), Some("mid"));
    roadmap.toggle_row(1, 0.0, false);
    assert_eq!(roadmap.selected().map(|c| c.id.as_str()), Some("ui"));
    roadmap.toggle_row(1, 0.0, false);
    assert!(roadmap.selected().is_none(), "a second click folds");
    assert!(roadmap.select("base", 0.0, false));
    assert!(!roadmap.select("missing", 0.0, false));
}

#[test]
fn the_tech_tree_layers_by_deps_and_bands_by_group() {
    let roadmap = seeded();
    let nodes = roadmap.dag_nodes();
    assert_eq!(nodes.len(), 3, "active chunks only");
    // Store order: base, mid, ui. Deps make columns: base 0, mid 1, ui 2.
    assert!(nodes[0].x < nodes[1].x && nodes[1].x < nodes[2].x);
    // Core (best priority 100) bands above Views (200).
    assert!(nodes[0].y < nodes[2].y && nodes[1].y < nodes[2].y);
    assert_eq!(nodes[0].region, nodes[1].region);
    assert_ne!(nodes[0].region, nodes[2].region);
    assert!(nodes[1].here, "in_progress is you-are-here");
    let edges = roadmap.dag_edges();
    assert_eq!(edges, vec![(0, 1), (1, 2)]);
}

#[test]
fn a_cycle_in_deps_still_places_every_node() {
    let mut roadmap = Roadmap::default();
    roadmap.seed(
        &[
            chunk("a", "pending", 100, "", &["b"]),
            chunk("b", "pending", 100, "", &["a"]),
        ],
        &[],
    );
    assert_eq!(roadmap.dag_nodes().len(), 2);
    assert_eq!(roadmap.dag_edges().len(), 2);
}

#[test]
fn clicks_and_hovers_take_the_nearest_node_within_reach() {
    let mut roadmap = seeded();
    let nodes = roadmap.dag_nodes();
    let (x, y) = (nodes[1].x, nodes[1].y);
    roadmap.hover_at(x + 0.5, y + 0.5, 0.0);
    assert_eq!(
        roadmap.hovered_title().as_deref(),
        Some("mid — Chunk mid"),
        "hover shows the title"
    );
    assert!(roadmap.click_at(x, y, 0.0));
    assert_eq!(roadmap.selected().map(|c| c.id.as_str()), Some("mid"));
    assert!(roadmap.dag_nodes()[1].selected);
    // Far from every node: hover clears, a click changes nothing.
    roadmap.hover_at(x + 100.0, y + 100.0, 0.0);
    assert!(roadmap.hovered_title().is_none());
    assert!(!roadmap.click_at(x + 100.0, y + 100.0, 0.0));
    assert_eq!(roadmap.selected().map(|c| c.id.as_str()), Some("mid"));
}

#[test]
fn panning_shifts_nodes_and_clamps_at_the_extent() {
    let mut roadmap = seeded();
    let before = roadmap.dag_nodes()[0].x;
    roadmap.pan_by(-4.0, 0.0);
    assert!((roadmap.dag_nodes()[0].x - (before - 4.0)).abs() < f32::EPSILON);
    // Panning back past the origin clamps to it.
    roadmap.pan_by(1000.0, 1000.0);
    assert_eq!(roadmap.pan(), (0.0, 0.0));
}

#[test]
fn proposals_list_every_open_suggestion_with_its_chunk() {
    let mut roadmap = seeded();
    let mut with_proposal = chunk_json("ui", "pending", 200, "Views", &["mid"]);
    with_proposal["reprioritize"] = json!({"suggested_priority": 100, "reason": "sooner"});
    with_proposal["title_proposal"] = json!({"suggested_title": "Better", "reason": "clearer"});
    roadmap.apply(
        &frame("roadmap", "chunk_changed", "chunk", with_proposal),
        0.0,
        false,
    );
    let proposals = roadmap.proposals();
    assert_eq!(proposals.len(), 2);
    assert_eq!(proposals[0].0.id, "ui");
    assert_eq!(proposals[0].1, ProposalKind::Reprioritize);
    assert_eq!(proposals[0].2, "100");
    assert_eq!(proposals[1].1, ProposalKind::Title);
    assert_eq!(proposals[1].2, "Better");
}

#[test]
fn an_empty_tab_says_so() {
    assert!(Roadmap::default().is_empty());
    assert!(!seeded().is_empty());
}

/// A 15-chunk chain c00 ← c01 ← … ← c14 with `here` in progress.
fn chain(here: usize) -> Roadmap {
    let chunks: Vec<Chunk> = (0..15)
        .map(|n| {
            let deps: Vec<String> = if n > 0 {
                vec![format!("c{:02}", n - 1)]
            } else {
                Vec::new()
            };
            let status = match n {
                _ if n == here => "in_progress",
                _ if n < here => "done",
                _ => "pending",
            };
            fixtures::chunk_from(json!({
                "id": format!("c{n:02}"), "name": format!("c{n:02}"),
                "status": status, "priority": 100, "group": "Core", "deps": deps,
            }))
        })
        .collect();
    let mut roadmap = Roadmap::default();
    roadmap.seed(&chunks, &[]);
    roadmap
}

#[test]
fn the_mini_dag_keeps_the_neighbourhood_of_you_are_here() {
    let roadmap = chain(7);
    let (nodes, edges) = roadmap.mini_dag(24, 6);
    assert_eq!(nodes.len(), 10, "capped at the mini size");
    assert_eq!(
        nodes.iter().filter(|node| node.here).count(),
        1,
        "you-are-here is marked once"
    );
    assert_eq!(edges.len(), 9, "the kept chain's edges survive");
    for node in &nodes {
        assert!(node.x >= 0.0 && node.x <= 24.0, "{node:?} inside the block");
        assert!(node.y >= 0.0 && node.y <= 6.0, "{node:?} inside the block");
    }
}

#[test]
fn the_mini_dag_without_a_here_takes_the_working_sets_head() {
    let chunks: Vec<Chunk> = (0..12)
        .map(|n| {
            fixtures::chunk_from(json!({
                "id": format!("p{n:02}"), "name": format!("p{n:02}"),
                "status": "pending", "priority": 100 + n, "group": "Core", "deps": [],
            }))
        })
        .collect();
    let mut roadmap = Roadmap::default();
    roadmap.seed(&chunks, &[]);
    let (nodes, edges) = roadmap.mini_dag(24, 6);
    assert_eq!(nodes.len(), 10, "the head of the working set");
    assert!(
        nodes.iter().all(|node| !node.here),
        "nothing is in progress"
    );
    assert!(edges.is_empty());
}

#[test]
fn the_mini_summary_names_you_are_here_and_its_neighbours() {
    assert_eq!(
        seeded().mini_summary(&crate::theme::Icons::UNICODE),
        ["▲ mid".to_owned(), "1 before · 1 after".to_owned()]
    );
    let mut idle = Roadmap::default();
    idle.seed(&[chunk("a", "pending", 100, "Core", &[])], &[]);
    assert_eq!(
        idle.mini_summary(&crate::theme::Icons::UNICODE),
        ["▲ nothing in progress".to_owned(), "1 pending".to_owned()]
    );
}

#[test]
#[ignore = "eyeball render, run by hand"]
fn eyeball_a_hundred_chunk_tree() {
    use crate::pixel::{CellSize, PixelSurface, RoadmapDag};
    let mut roadmap = Roadmap::default();
    let groups = [
        "Engine",
        "GUI rewrite",
        "Scaffold",
        "Frontends",
        "Desktop",
        "Agents",
        "Enterprise",
    ];
    let mut chunks = Vec::new();
    for i in 0..100 {
        let group = groups[i % groups.len()];
        let deps: Vec<String> = if i >= 7 {
            vec![format!("c{}", i - 7)]
        } else {
            vec![]
        };
        let status = match i % 9 {
            0..=3 => "done",
            4 => "in_progress",
            5 | 6 => "pending",
            7 => "backlog",
            _ => "blocked",
        };
        chunks.push(fixtures::chunk_from(json!({
            "id": format!("c{i}"), "status": status,
            "priority": 100 + (i % 4) * 100, "group": group, "deps": deps,
        })));
    }
    roadmap.seed(&chunks, &[]);
    roadmap.set_viewport(Some((64, 14)));
    roadmap.select("c40", 0.0, false);
    let dag = RoadmapDag::new(roadmap.dag_nodes(), roadmap.dag_edges(), (10, 20));
    let mut surface = PixelSurface::new(
        crate::layout::Rect::new(0, 0, 64, 14),
        CellSize::new(10, 20),
    );
    surface.paint(&dag);
    let pixmap = tiny_skia::Pixmap::from_vec(
        surface.pixels().to_vec(),
        tiny_skia::IntSize::from_wh(640, 280).expect("size"),
    )
    .expect("pixmap");
    let output_dir = tempfile::Builder::new()
        .prefix("magistr-roadmap-render-")
        .tempdir()
        .expect("output directory")
        .keep();
    let output_path = output_dir.join("dag_eyeball.png");
    pixmap.save_png(&output_path).expect("write");
    eprintln!("Roadmap render: {}", output_path.display());
}

#[test]
fn the_body_rows_count_the_sections_the_rows_the_done_line_and_the_inboxes() {
    let roadmap = seeded();
    // in progress: spacer + header + mid; pending: spacer + header + ui;
    // done: spacer + the count line; signals: spacer + heading + s1.
    assert_eq!(roadmap.body_rows(), 11);
}

#[test]
fn selecting_a_row_grows_the_body_by_its_buttons_and_its_detail() {
    let mut roadmap = seeded();
    let plain = roadmap.body_rows();
    assert!(roadmap.select("mid", 0.0, false));
    let open = roadmap.chunk("mid").expect("mid").detail_rows();
    assert_eq!(
        roadmap.body_rows(),
        plain + 1 + open,
        "the button row and the card"
    );
}

#[test]
fn a_chunk_selected_off_the_list_takes_its_own_card_under_the_graph() {
    let mut roadmap = seeded();
    let plain = roadmap.body_rows();
    assert!(
        roadmap.select("base", 0.0, false),
        "done chunks are still selectable"
    );
    let card = roadmap.chunk("base").expect("base").detail_rows();
    assert_eq!(
        roadmap.body_rows(),
        plain + 2 + card,
        "the spacer and the head line"
    );
}

#[test]
fn a_hover_takes_the_tooltip_row() {
    let mut roadmap = seeded();
    roadmap.set_viewport(Some((40, 20)));
    let plain = roadmap.body_rows();
    let (x, y) = {
        let nodes = roadmap.dag_nodes();
        (nodes[1].x, nodes[1].y)
    };
    roadmap.hover_at(x, y, 0.0);
    assert!(roadmap.hovered_title().is_some(), "the hover took a node");
    assert_eq!(roadmap.body_rows(), plain + 1);
}

#[test]
fn an_empty_roadmap_bodies_to_its_one_note() {
    assert_eq!(Roadmap::default().body_rows(), 1);
}

#[test]
fn the_signals_inbox_takes_a_heading_and_a_row_each() {
    let mut roadmap = Roadmap::default();
    roadmap.seed(&[], &[signal("s1", "researched")]);
    assert_eq!(
        roadmap.body_rows(),
        3,
        "the spacer, the heading and the one open signal"
    );
}

#[test]
fn a_fresh_roadmap_sits_at_unit_zoom() {
    assert!((Roadmap::default().zoom() - 1.0).abs() < f32::EPSILON);
}

#[test]
fn zooming_in_spreads_the_tree_and_zooming_back_out_restores_it() {
    let mut roadmap = seeded();
    roadmap.set_viewport(Some((60, 20)));
    let before = roadmap.dag_nodes();
    roadmap.zoom_at(2.0, 10.0, 5.0);
    let zoomed = roadmap.dag_nodes();
    assert!(roadmap.zoom() > 1.0, "the wheel notch zoomed in");
    let spread = |nodes: &[DagNode]| nodes[2].x - nodes[0].x;
    assert!(
        spread(&zoomed) > spread(&before),
        "the columns sit further apart: {} vs {}",
        spread(&zoomed),
        spread(&before)
    );
    roadmap.zoom_at(0.5, 10.0, 5.0);
    assert!((roadmap.zoom() - 1.0).abs() < 1e-5, "back where it started");
    for (was, now) in before.iter().zip(roadmap.dag_nodes()) {
        assert!((was.x - now.x).abs() < 1e-3 && (was.y - now.y).abs() < 1e-3);
    }
}

#[test]
fn the_zoom_holds_the_point_under_the_pointer_still() {
    // A tree that overflows its block, so the pan has room to hold the
    // anchor — a tree that fits has nowhere to move and cannot.
    let mut roadmap = chained(10);
    roadmap.set_viewport(Some((20, 8)));
    let (x, y) = {
        let nodes = roadmap.dag_nodes();
        (nodes[4].x, nodes[4].y)
    };
    roadmap.zoom_at(1.5, x, y);
    let after = {
        let nodes = roadmap.dag_nodes();
        (nodes[4].x, nodes[4].y)
    };
    assert!(
        (after.0 - x).abs() < 1e-3 && (after.1 - y).abs() < 1e-3,
        "the node under the pointer did not move: {after:?} vs ({x}, {y})"
    );
}

#[test]
fn hit_testing_follows_the_zoom() {
    let mut roadmap = seeded();
    roadmap.set_viewport(Some((60, 20)));
    roadmap.zoom_at(2.0, 0.0, 0.0);
    let (x, y) = {
        let nodes = roadmap.dag_nodes();
        (nodes[2].x, nodes[2].y)
    };
    roadmap.hover_at(x, y, 0.0);
    assert_eq!(
        roadmap
            .hovered_title()
            .as_deref()
            .map(|t| t.starts_with("ui")),
        Some(true),
        "the third node is still the one under its own coordinates"
    );
}

#[test]
fn the_zoom_is_bounded_at_both_ends() {
    let mut roadmap = seeded();
    roadmap.set_viewport(Some((60, 20)));
    for _ in 0..40 {
        roadmap.zoom_at(2.0, 5.0, 5.0);
    }
    assert!(roadmap.zoom() <= 8.0 + 1e-5, "capped: {}", roadmap.zoom());
    for _ in 0..80 {
        roadmap.zoom_at(0.5, 5.0, 5.0);
    }
    assert!(roadmap.zoom() >= 0.25 - 1e-5, "floored: {}", roadmap.zoom());
}

#[test]
fn a_reseed_keeps_the_selection_the_pan_and_the_zoom() {
    let mut roadmap = seeded();
    roadmap.set_viewport(Some((20, 8)));
    assert!(roadmap.select("ui", 0.0, false));
    roadmap.zoom_at(2.0, 4.0, 4.0);
    roadmap.pan_by(-3.0, -1.0);
    let (pan, zoom) = (roadmap.pan(), roadmap.zoom());
    roadmap.seed(
        &[
            chunk("base", "done", 100, "Core", &[]),
            chunk("mid", "done", 100, "Core", &["base"]),
            chunk("ui", "in_progress", 200, "Views", &["mid"]),
        ],
        &[],
    );
    assert_eq!(roadmap.selected().map(|c| c.id.as_str()), Some("ui"));
    assert_eq!(roadmap.pan(), pan, "the pan survives the re-read");
    assert!((roadmap.zoom() - zoom).abs() < f32::EPSILON);
    assert_eq!(roadmap.done_count(), 2, "and the new state landed");
}

// ---------------------------------------------------------------------
// the tech-tree moves like a camera.
// ---------------------------------------------------------------------

/// A long dependency chain in a block far too small for it, so every
/// node but the first few is off screen and a focus has somewhere to go.
fn overflowing() -> Roadmap {
    let mut roadmap = chained(40);
    roadmap.set_viewport(Some((30, 10)));
    roadmap
}

/// Whether the node for `cN` of a [`chained`] roadmap is inside the
/// block at step `now`. The graph's index space is the store's active
/// order, which `chained` seeds as `c0, c1, …`.
fn framed(roadmap: &Roadmap, id: &str, now: u64) -> bool {
    let at: usize = id[1..].parse().expect("cN");
    let node = &roadmap.dag_nodes_at(now as f32)[at];
    node.x >= 0.0 && node.x <= 30.0 && node.y >= 0.0 && node.y <= 10.0
}

#[test]
fn a_focus_glides_the_camera_until_the_block_frames_the_node() {
    let mut roadmap = overflowing();
    assert!(!framed(&roadmap, "c39", 0), "the far end starts off screen");
    assert!(roadmap.focus("c39", 0.0, true));
    // The camera has settled on the target at once...
    assert!(roadmap.pan().0 > 0.0, "the settled pan moved onto the node");
    // ...but the paint starts where it was and arrives over Eased::STEPS.
    assert!(!framed(&roadmap, "c39", 0), "the glide starts where it was");
    assert!(roadmap.is_gliding(0.0));
    assert!(roadmap.is_gliding((Eased::STEPS - 1) as f32));
    assert!(
        !roadmap.is_gliding(Eased::STEPS as f32),
        "and then gives the tick back"
    );
    assert!(
        framed(&roadmap, "c39", Eased::STEPS),
        "the glide ends on a frame that contains the node"
    );
}

#[test]
fn a_focus_under_reduced_motion_lands_on_the_frame_in_one_step() {
    let mut roadmap = overflowing();
    assert!(roadmap.focus("c39", 0.0, false));
    assert!(
        framed(&roadmap, "c39", 0),
        "no glide, so it is already there"
    );
    assert!(!roadmap.is_gliding(0.0), "and nothing asks for a tick");
}

#[test]
fn a_focus_on_a_node_already_in_frame_leaves_the_camera_alone() {
    let mut roadmap = overflowing();
    let before = (roadmap.pan(), roadmap.zoom());
    assert!(framed(&roadmap, "c3", 0), "the near end starts on screen");
    assert!(
        !roadmap.focus("c3", 0.0, true),
        "nothing to bring into view"
    );
    assert_eq!((roadmap.pan(), roadmap.zoom()), before);
    assert!(!roadmap.is_gliding(0.0));
}

#[test]
fn a_focus_from_zoomed_out_comes_back_to_the_fit_on_the_way() {
    let mut roadmap = overflowing();
    for _ in 0..4 {
        roadmap.zoom_at(0.5, 15.0, 5.0);
    }
    assert!(roadmap.zoom() < 1.0, "zoomed out past the fit");
    assert!(roadmap.focus("c20", 0.0, true));
    assert!((roadmap.zoom() - 1.0).abs() < 1e-5, "back at the fit");
    assert!(
        roadmap.dag_nodes_at(0.0) != roadmap.dag_nodes_at(Eased::STEPS as f32),
        "and the zoom glided rather than cutting"
    );
}

#[test]
fn a_focus_from_zoomed_in_keeps_the_scale_the_reader_chose() {
    let mut roadmap = overflowing();
    roadmap.zoom_at(2.0, 15.0, 5.0);
    let zoom = roadmap.zoom();
    assert!(roadmap.focus("c30", 0.0, true));
    assert!(
        (roadmap.zoom() - zoom).abs() < 1e-5,
        "the wheel's zoom stands"
    );
}

#[test]
fn a_cell_only_tab_has_no_camera_to_move() {
    let mut roadmap = chained(40);
    assert!(
        !roadmap.focus("c39", 0.0, true),
        "no graph block, no camera, no tick"
    );
    assert!(!roadmap.is_gliding(0.0));
    assert_eq!(roadmap.pan(), (0.0, 0.0));
}

#[test]
fn opening_a_row_focuses_its_node_and_folding_it_leaves_the_camera() {
    let mut roadmap = overflowing();
    let far = roadmap
        .rows()
        .iter()
        .position(|c| c.id == "c39")
        .expect("c39 is in the list");
    roadmap.toggle_row(far, 0.0, true);
    assert_eq!(roadmap.selected().map(|c| c.id.as_str()), Some("c39"));
    let moved = roadmap.pan();
    assert!(moved.0 > 0.0, "opening brought the node into frame");
    roadmap.toggle_row(far, Eased::STEPS as f32, true);
    assert!(roadmap.selected().is_none(), "a second click folds");
    assert_eq!(roadmap.pan(), moved, "and folding moves nothing");
}

#[test]
fn a_frame_that_puts_a_chunk_in_progress_takes_the_camera_with_it() {
    let mut roadmap = overflowing();
    roadmap.apply(
        &chunk_frame(
            "chunk_changed",
            chunk("c39", "in_progress", 100, "Core", &["c38"]),
        ),
        0.0,
        true,
    );
    assert!(
        roadmap.is_gliding(0.0),
        "the camera moved onto the new here"
    );
    assert!(framed(&roadmap, "c39", Eased::STEPS));
    // The same chunk broadcasting again is not a transition: the reader
    // keeps the view they have.
    roadmap.pan_by(-5.0, 0.0);
    let held = roadmap.pan();
    roadmap.apply(
        &chunk_frame(
            "chunk_changed",
            chunk("c39", "in_progress", 100, "Core", &["c38"]),
        ),
        Eased::STEPS as f32,
        true,
    );
    assert_eq!(roadmap.pan(), held, "a re-broadcast does not re-frame");
}

#[test]
fn a_drag_and_a_wheel_notch_never_start_a_glide() {
    let mut roadmap = overflowing();
    roadmap.pan_by(-6.0, -2.0);
    assert!(!roadmap.is_gliding(0.0), "a drag is instant");
    roadmap.zoom_at(2.0, 15.0, 5.0);
    assert!(!roadmap.is_gliding(0.0), "and so is a notch");
}

// ---------------------------------------------------------------------
// the keyboard's way in and out of the zoom, and the readout
// that says where it stands.
// ---------------------------------------------------------------------

#[test]
fn a_zoom_step_spreads_the_tree_the_way_a_wheel_notch_does() {
    let mut roadmap = overflowing();
    let before = roadmap.dag_nodes();
    assert!(
        roadmap.zoom_step(1.5, 0.0, true),
        "there is a camera to move"
    );
    assert!(roadmap.zoom() > 1.0);
    let spread = |nodes: &[DagNode]| nodes[3].x - nodes[0].x;
    assert!(spread(&roadmap.dag_nodes()) > spread(&before));
}

#[test]
fn a_zoom_step_glides_the_paint_and_asks_for_the_tick() {
    let mut roadmap = overflowing();
    assert!(roadmap.zoom_step(1.5, 0.0, true));
    assert!(roadmap.is_gliding(0.0), "the paint is still catching up");
    assert!(!roadmap.is_gliding(Eased::STEPS as f32));
}

#[test]
fn a_zoom_step_under_reduced_motion_never_starts_the_tick() {
    let mut roadmap = overflowing();
    assert!(roadmap.zoom_step(1.5, 0.0, false));
    assert!(roadmap.zoom() > 1.0, "the scale still changed");
    assert!(!roadmap.is_gliding(0.0));
}

#[test]
fn the_zoom_step_is_bounded_at_both_ends_like_the_wheel() {
    let mut roadmap = overflowing();
    for _ in 0..40 {
        roadmap.zoom_step(1.5, 0.0, false);
    }
    assert!(roadmap.zoom() <= 8.0 + 1e-5, "capped: {}", roadmap.zoom());
    for _ in 0..80 {
        roadmap.zoom_step(1.0 / 1.5, 0.0, false);
    }
    assert!(roadmap.zoom() >= 0.25 - 1e-5, "floored: {}", roadmap.zoom());
}

#[test]
fn the_fit_puts_the_zoom_back_to_one_and_the_pan_to_nothing() {
    let mut roadmap = overflowing();
    roadmap.zoom_at(2.0, 25.0, 8.0);
    roadmap.pan_by(-6.0, -3.0);
    assert!(roadmap.zoom() > 1.0 && roadmap.pan().0 > 0.0);
    assert!(roadmap.reset_view(0.0, true), "there is a camera to reset");
    assert!((roadmap.zoom() - 1.0).abs() < 1e-5);
    assert_eq!(roadmap.pan(), (0.0, 0.0));
    assert!(
        roadmap.is_gliding(0.0),
        "and it glides back rather than cutting"
    );
}

#[test]
fn a_cell_only_tab_has_no_zoom_to_step_or_reset() {
    let mut roadmap = chained(40);
    assert!(
        !roadmap.zoom_step(1.5, 0.0, true),
        "no graph block, no camera"
    );
    assert!(!roadmap.reset_view(0.0, true));
    assert!((roadmap.zoom() - 1.0).abs() < f32::EPSILON);
    assert!(!roadmap.is_gliding(0.0));
}

#[test]
fn the_scale_reads_as_a_share_of_the_fit_and_is_silent_at_the_fit() {
    let mut roadmap = overflowing();
    assert_eq!(roadmap.scale_label(), None, "the fit grows no chrome");
    roadmap.zoom_step(2.0, 0.0, false);
    assert_eq!(roadmap.scale_label().as_deref(), Some("zoom 200%"));
    roadmap.reset_view(0.0, false);
    assert_eq!(roadmap.scale_label(), None);
}

#[test]
fn a_pair_of_notches_that_cancel_leaves_the_readout_silent() {
    // The acceptance is byte-identity, not float identity: a zoom that
    // rounds to the fit must not print `zoom 100%` off a residue.
    let mut roadmap = overflowing();
    for _ in 0..6 {
        roadmap.zoom_at(1.19, 15.0, 5.0);
    }
    for _ in 0..6 {
        roadmap.zoom_at(1.0 / 1.19, 15.0, 5.0);
    }
    assert_eq!(roadmap.scale_label(), None, "{}", roadmap.zoom());
}

#[test]
fn the_readout_names_a_zoomed_out_scale_without_rounding_it_away() {
    let mut roadmap = overflowing();
    for _ in 0..40 {
        roadmap.zoom_step(1.0 / 1.5, 0.0, false);
    }
    assert_eq!(roadmap.scale_label().as_deref(), Some("zoom 25%"));
}

#[test]
fn a_ref_resolves_to_the_chunk_that_names_it_or_to_no_chunk_at_all() {
    // Three shapes reach the same chunk: the explicit `chunk:<id>` a
    // cross-ref carries, the bare id, and — the shape the loop actually
    // links with — a `task:<id>` the chunk names back in its own
    // cross_refs. Everything else is not a link, which is what keeps an
    // unresolvable ref plain text.
    let mut linked = chunk("tui2-5b", "pending", 100, "TUI", &[]);
    linked.cross_refs = vec!["task:trace-jump".to_owned(), "think:261".to_owned()];
    let mut roadmap = Roadmap::default();
    roadmap.seed(&[linked, chunk("other", "done", 90, "TUI", &[])], &[]);

    for reference in ["chunk:tui2-5b", "tui2-5b", "task:trace-jump", "think:261"] {
        assert_eq!(
            roadmap.chunk_for_ref(reference).map(|c| c.id.as_str()),
            Some("tui2-5b"),
            "{reference}"
        );
    }
    for reference in ["task:nobody", "chunk:ghost", "", "think:1"] {
        assert!(roadmap.chunk_for_ref(reference).is_none(), "{reference}");
    }
}

#[test]
fn a_done_chunk_is_still_a_jump_target() {
    // `rows()` drops done chunks, but a trace step pointing at one must
    // still reach it — the card opens off the list.
    let mut roadmap = Roadmap::default();
    roadmap.seed(&[chunk("shipped", "done", 100, "TUI", &[])], &[]);
    assert_eq!(
        roadmap
            .chunk_for_ref("chunk:shipped")
            .map(|c| c.id.as_str()),
        Some("shipped")
    );
    assert!(roadmap.select("shipped", 0.0, false));
    assert_eq!(roadmap.selected().map(|c| c.id.as_str()), Some("shipped"));
}

// ── The ship moment ────────────────────────────────────────────────

/// `a` shipped; `b` depended on it alone and `c` on it and on `d`,
/// which has not shipped. Only `b` was unlocked.
fn shipping() -> Roadmap {
    let mut roadmap = Roadmap::default();
    roadmap.seed(
        &[
            chunk("a", "in_progress", 100, "Core", &[]),
            chunk("b", "pending", 200, "Core", &["a"]),
            chunk("c", "pending", 200, "Core", &["a", "d"]),
            chunk("d", "pending", 300, "Ops", &[]),
        ],
        &[],
    );
    roadmap
}

fn completed(id: &str, group: &str, deps: &[&str]) -> NativeEvent {
    chunk_frame("chunk_completed", chunk(id, "done", 100, group, deps))
}

/// A completion names exactly the chunks it made ready — not the ones
/// that were ready already, and not the ones still waiting on someone
/// else.
#[test]
fn a_completion_names_what_it_unlocked() {
    let mut roadmap = shipping();
    roadmap.apply(&completed("a", "Core", &[]), 10.0, true);
    let run = roadmap.ship_run(10.0).expect("the moment is running");
    let id = |index: usize| roadmap.graph_chunk(index).expect("in the graph").id.clone();
    assert_eq!(id(run.star), "a");
    let unlocked: Vec<String> = run.unlocked.iter().map(|&index| id(index)).collect();
    assert_eq!(
        unlocked,
        vec!["b".to_owned()],
        "c still waits on d, and d was ready before a shipped"
    );
}

/// The hue is the shipped chunk's own workstream ordinal — the same
/// `region` its star is drawn with, not a colour the moment invents.
#[test]
fn the_moment_ignites_in_the_chunks_own_workstream() {
    let mut roadmap = shipping();
    roadmap.apply(&completed("d", "Ops", &[]), 4.0, true);
    let run = roadmap.ship_run(4.0).expect("running");
    let region = roadmap.dag_nodes()[run.star].region;
    assert_eq!(run.region, region);
}

/// Reduced motion records no run at all, so the sky is structurally the
/// end frame and nothing asks the runtime for a tick.
#[test]
fn reduced_motion_ships_without_a_moment() {
    let mut roadmap = shipping();
    roadmap.apply(&completed("a", "Core", &[]), 10.0, false);
    assert!(roadmap.ship_run(10.0).is_none());
    assert!(!roadmap.is_gliding(10.0));
    assert_eq!(
        roadmap.chunk("a").expect("still folded").status,
        "done",
        "the completion itself lands either way"
    );
}

/// Only a completion opens a moment: a chunk merely changing does not,
/// however its readiness moved.
#[test]
fn only_a_completion_opens_a_moment() {
    let mut roadmap = shipping();
    roadmap.apply(
        &chunk_frame("chunk_changed", chunk("a", "done", 100, "Core", &[])),
        10.0,
        true,
    );
    assert!(roadmap.ship_run(10.0).is_none());
}

/// The moment ends, and when it does the sky draws what it drew before
/// anything shipped — and stops asking for ticks.
#[test]
fn the_moment_ends_and_the_tick_stops() {
    let mut roadmap = shipping();
    roadmap.apply(&completed("a", "Core", &[]), 10.0, true);
    assert!(roadmap.is_gliding(10.0 + ShipRun::STEPS as f32 - 1.0));
    assert!(roadmap.ship_run(10.0 + ShipRun::STEPS as f32).is_none());
    assert!(!roadmap.is_gliding(10.0 + ShipRun::STEPS as f32));
}

/// A chunk that leaves the graph between the completion and the end of
/// the run takes its light with it rather than lighting whoever now
/// stands at its old index.
#[test]
fn a_target_that_left_the_graph_is_not_lit() {
    let mut roadmap = shipping();
    roadmap.apply(&completed("a", "Core", &[]), 10.0, true);
    roadmap.apply(
        &chunk_frame(
            "chunk_obsoleted",
            chunk("b", "obsoleted", 200, "Core", &["a"]),
        ),
        11.0,
        true,
    );
    let run = roadmap.ship_run(11.0).expect("the moment is still running");
    assert!(run.unlocked.is_empty());
}

#[test]
fn a_small_graphs_sky_keeps_every_shipped_star() {
    // At fixture scale — no more shipped chunks than the recent-ships
    // floor — the constellation is exactly what it always was, which is
    // what keeps every golden and the view-03 template byte-identical.
    let mut roadmap = Roadmap::default();
    let mut chunks: Vec<Chunk> = (0..7)
        .map(|n| chunk(&format!("d{n}"), "done", 100, "Core", &[]))
        .collect();
    chunks.push(chunk("work", "in_progress", 100, "Core", &["d6"]));
    roadmap.seed(&chunks, &[]);
    assert_eq!(roadmap.dag_nodes().len(), 8, "seven ships and the work");
}

#[test]
fn a_deep_history_collapses_to_the_frontier_its_context_and_the_newest_ships() {
    // Thirty shipped chunks, one of them ancient but load-bearing (a
    // direct dep of the frontier): the sky keeps the living set, that
    // context star, and the newest twelve ships — the rest live in the
    // counter alone.
    let mut chunks: Vec<Chunk> = (0..30)
        .map(|n| chunk(&format!("d{n:02}"), "done", 100, "Core", &[]))
        .collect();
    chunks.push(chunk("work", "in_progress", 100, "Core", &["d02"]));
    chunks.push(chunk("next", "pending", 200, "Core", &["work"]));
    chunks.push(chunk("someday", "backlog", 300, "Views", &[]));
    let mut roadmap = Roadmap::default();
    roadmap.seed(&chunks, &[]);

    let ids: Vec<String> = roadmap
        .dag_nodes()
        .iter()
        .enumerate()
        .map(|(index, _)| roadmap.graph_chunk(index).expect("in the graph").id.clone())
        .collect();
    // The living set rides whole.
    for living in ["work", "next", "someday"] {
        assert!(ids.contains(&living.to_owned()), "{living} is on the sky");
    }
    // The frontier's own ground rides as context, however old.
    assert!(
        ids.contains(&"d02".to_owned()),
        "the dep the work stands on"
    );
    // The newest ships ride; the ancient unreferenced ones do not.
    assert!(ids.contains(&"d29".to_owned()), "the newest ship");
    assert!(ids.contains(&"d18".to_owned()), "the twelfth-newest ship");
    assert!(
        !ids.contains(&"d17".to_owned()),
        "the thirteenth is history"
    );
    assert!(
        !ids.contains(&"d00".to_owned()),
        "deep history is the counter's"
    );
    // 3 living + 12 newest ships + 1 context dep outside them.
    assert_eq!(ids.len(), 16, "{ids:?}");
    // The counter still tells the whole story.
    assert_eq!(roadmap.done_count(), 30);
}

#[test]
fn the_filtered_skys_edges_never_dangle_and_the_frontiers_ground_rides_as_context() {
    // Every edge names indices inside the filtered graph, and a
    // frontier chunk's shipped ground is always shown — so readiness
    // is never judged against a star that is not there.
    let mut chunks: Vec<Chunk> = (0..20)
        .map(|n| chunk(&format!("d{n:02}"), "done", 100, "Core", &[]))
        .collect();
    // Depends on the oldest ship, which the sky hides.
    chunks.push(chunk("ready", "pending", 200, "Core", &["d00"]));
    let mut roadmap = Roadmap::default();
    roadmap.seed(&chunks, &[]);
    let nodes = roadmap.dag_nodes();
    for (from, to) in roadmap.dag_edges() {
        assert!(
            from < nodes.len() && to < nodes.len(),
            "({from},{to}) dangles"
        );
    }
    let ready = (0..nodes.len())
        .find(|&i| roadmap.graph_chunk(i).expect("in the graph").id == "ready")
        .expect("the pending chunk is on the sky");
    assert_eq!(nodes[ready].state, StarState::Ready);
    // d00 is the frontier's direct dep, so it rides as context.
    assert!(
        (0..nodes.len()).any(|i| roadmap.graph_chunk(i).expect("chunk").id == "d00"),
        "the dep the ready chunk stands on is shown"
    );
}
