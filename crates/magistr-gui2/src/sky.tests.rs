use super::*;

use magistr_theme::Palette;
use qs_gpu::frame::PrimKind;

/// A field big enough that nothing clips, on the design's own grid.
const FIELD: PxRect = PxRect {
    x: 0.0,
    y: 0.0,
    width: 800.0,
    height: 400.0,
};
const CELL: (f32, f32) = (9.0, 20.0);

fn palette(medium: Medium) -> Palette {
    match medium {
        Medium::Night => magistr_theme::catppuccin_mocha(),
        Medium::Day => magistr_theme::catppuccin_latte(),
    }
}

fn sky(medium: Medium) -> Sky {
    Sky::new(FIELD, CELL, 1.0, palette(medium), medium).at(0.0, true)
}

fn star(x: f32, y: f32, state: StarState) -> DagNode {
    DagNode {
        x,
        y,
        state,
        status: magistr_cockpit::cockpit::ChunkState::Pending,
        region: 0,
        here: false,
        selected: false,
    }
}

/// One of each state, spread across the field.
fn every_state() -> Vec<DagNode> {
    StarState::ALL
        .into_iter()
        .enumerate()
        .map(|(index, state)| star(4.0 + index as f32 * 6.0, 8.0, state))
        .collect()
}

fn drawn(sky: &Sky, nodes: &[DagNode], edges: &[(usize, usize, EdgeKind)]) -> DrawList {
    let mut list = DrawList::default();
    sky.show(nodes, edges, &mut list);
    list
}

fn kinds(list: &DrawList) -> Vec<PrimKind> {
    list.instances
        .iter()
        .filter_map(|i| PrimKind::from_raw(i.kind))
        .collect()
}

fn count(list: &DrawList, kind: PrimKind) -> usize {
    kinds(list).into_iter().filter(|k| *k == kind).count()
}

/// Every field of every instance, in order — what "byte-identical"
/// means for a frame that is a draw list rather than a picture.
fn bytes(list: &DrawList) -> Vec<u32> {
    list.instances
        .iter()
        .flat_map(|i| {
            i.rect
                .iter()
                .chain(i.uv.iter())
                .chain([i.radius, i.param].iter())
                .map(|f| f.to_bits())
                .chain([i.color, i.kind])
                .collect::<Vec<u32>>()
        })
        .collect()
}

// ── The two media are two drawings ─────────────────────────────────

/// Night's medium is light and day's is ink: a working star carries a real halo at
/// night and none at all by day, whatever else the two share. Asserted over
/// the draw list rather than reviewed.
#[test]
fn only_night_emits_light() {
    let working = [star(10.0, 8.0, StarState::Executing)];
    let night = drawn(&sky(Medium::Night), &working, &[]);
    let day = drawn(&sky(Medium::Day), &working, &[]);
    assert!(
        count(&night, PrimKind::Glow) > 0,
        "the night sky draws the halo as light"
    );
    assert_eq!(
        count(&day, PrimKind::Glow),
        0,
        "the chart has no light in it — the halo is a ring"
    );
}

/// Day's field is graticule paper and night's is flat ground ("no ambient washes"), so
/// the ruling is drawn in exactly one medium.
#[test]
fn only_day_is_drawn_on_paper() {
    let nodes = every_state();
    let day = drawn(&sky(Medium::Day), &nodes, &[]);
    let night = drawn(&sky(Medium::Night), &nodes, &[]);
    assert!(
        count(&day, PrimKind::Rect) > count(&night, PrimKind::Rect),
        "the graticule's hairlines are rects the night sky does not draw"
    );
}

/// **Ink never twinkles.** A day sky drawn at two different steps is
/// byte-for-byte the same list; a night sky is not. The medium is the
/// first term of the predicate, so no amount of ticking can reach the
/// motion by day.
#[test]
fn ink_never_twinkles() {
    let shipped: Vec<DagNode> = (0..5)
        .map(|i| star(4.0 + i as f32 * 6.0, 8.0, StarState::Done))
        .collect();
    let at = |medium, now| drawn(&sky(medium).at(now, true), &shipped, &[]);
    assert_eq!(
        bytes(&at(Medium::Day, 0.0)),
        bytes(&at(Medium::Day, 9.0)),
        "a chart's marks are the same ink on every frame"
    );
    assert_ne!(
        bytes(&at(Medium::Night, 0.0)),
        bytes(&at(Medium::Night, 9.0)),
        "shipped stars twinkle at night"
    );
}

/// Each shipped star twinkles at its own phase: two stars on the same
/// frame are at different points of the same wave, which is what stops
/// a constellation blinking as one lamp.
#[test]
fn every_star_twinkles_at_its_own_phase() {
    let shipped = [
        star(4.0, 8.0, StarState::Done),
        star(20.0, 8.0, StarState::Done),
    ];
    let list = drawn(&sky(Medium::Night).at(5.0, true), &shipped, &[]);
    let inks: Vec<u32> = list.instances.iter().map(|i| i.color).collect();
    assert_eq!(inks.len(), 2);
    assert_ne!(inks[0], inks[1], "the two stars share a phase");
}

// ── The state vocabulary ───────────────────────────────────────────

/// Every one of the seven states draws something, and the four the
/// design gives an outline or an alert ring to are the four that emit a
/// dashed stroke — pending and ghost for "nothing has happened", stuck
/// and blocked for "this needs a person".
#[test]
fn every_state_draws_and_only_the_four_outlined_ones_are_dashed() {
    for state in StarState::ALL {
        let list = drawn(&sky(Medium::Night), &[star(10.0, 8.0, state)], &[]);
        assert!(
            !list.instances.is_empty(),
            "{} drew nothing at all",
            state.label()
        );
        let dashed = count(&list, PrimKind::DashedStroke);
        let wants_dash = !state.has_core() || state.alerts();
        assert_eq!(
            dashed > 0,
            wants_dash,
            "{} drew {dashed} dashed strokes",
            state.label()
        );
    }
}

/// The stuck star and the blocked star both wear the alert ring — so
/// the constellation and the sessions strip cannot disagree about who
/// needs a person — and the *core* is what tells them apart: stuck
/// breathes, blocked does not.
#[test]
fn the_alert_ring_is_shared_and_the_breath_is_not() {
    let ring = |state| {
        count(
            &drawn(&sky(Medium::Night), &[star(10.0, 8.0, state)], &[]),
            PrimKind::DashedStroke,
        )
    };
    assert_eq!(ring(StarState::Stuck), 1);
    assert_eq!(ring(StarState::Blocked), 1);
    let moved = |state| {
        let one = drawn(
            &sky(Medium::Night).at(0.0, true),
            &[star(10.0, 8.0, state)],
            &[],
        );
        let two = drawn(
            &sky(Medium::Night).at(6.0, true),
            &[star(10.0, 8.0, state)],
            &[],
        );
        bytes(&one) != bytes(&two)
    };
    assert!(moved(StarState::Stuck), "the stuck star breathes");
    assert!(
        !moved(StarState::Blocked),
        "a blocked star is held, not working"
    );
}

/// A workstream is the only thing that tints a star, and only where the
/// state does not name its own colour: two ready chunks on two
/// workstreams are two hues, two shipped ones are one green.
#[test]
fn the_workstream_is_the_only_decoration() {
    let tinted = |state, region| {
        let mut node = star(10.0, 8.0, state);
        node.region = region;
        let list = drawn(&sky(Medium::Night), &[node], &[]);
        list.instances.last().expect("a star").color
    };
    assert_ne!(
        tinted(StarState::Ready, 0),
        tinted(StarState::Ready, 1),
        "core and api are two hues"
    );
    assert_eq!(
        tinted(StarState::Done, 0),
        tinted(StarState::Done, 1),
        "shipped is green wherever it shipped"
    );
}

// ── The two edge kinds ─────────────────────────────────────────────

/// A dependency already walked is a solid line and one still ahead is a
/// dashed one — the same walk with the gaps skipped, so the road ahead
/// costs strictly fewer stamps over the same distance.
#[test]
fn the_road_ahead_is_dashed_and_the_road_behind_is_not() {
    let nodes = [
        star(4.0, 4.0, StarState::Done),
        star(40.0, 16.0, StarState::Pending),
    ];
    let stamps = |kind| {
        let mut list = DrawList::default();
        sky(Medium::Night).show(&nodes, &[(0, 1, kind)], &mut list);
        // Everything but the two stars' own instances.
        list.instances.len()
    };
    assert!(
        stamps(EdgeKind::Ahead) < stamps(EdgeKind::Done),
        "a dashed edge lays down fewer stamps than a solid one"
    );
}

// ── The camera ─────────────────────────────────────────────────────

/// Parallax: the paper travels further than the constellation on it.
/// The stars carry the pan in their placement, so the assertion is that
/// the graticule's own drift is larger than the pan itself.
#[test]
fn the_paper_outruns_the_stars() {
    let first = |list: &DrawList| {
        list.instances
            .first()
            .map(|i| i.rect[0])
            .unwrap_or_default()
    };
    let still = drawn(&sky(Medium::Day), &every_state(), &[]);
    let panned = drawn(&sky(Medium::Day).panned((2.0, 0.0)), &every_state(), &[]);
    let paper = (first(&still) - first(&panned)).abs();
    let stars = 2.0 * CELL.0;
    assert!(
        paper > stars,
        "the paper moved {paper} where the stars moved {stars}"
    );
}

/// A star panned off the field is neither drawn nor clickable: the
/// cockpit's one batch carries no scissor, so a star half a screen away
/// would paint over the rail.
#[test]
fn a_star_off_the_field_is_not_drawn_and_not_a_target() {
    let mut list = DrawList::default();
    let targets = sky(Medium::Night).show(
        &[
            star(10.0, 8.0, StarState::Done),
            star(-40.0, 8.0, StarState::Done),
            star(4000.0, 8.0, StarState::Done),
        ],
        &[],
        &mut list,
    );
    assert_eq!(targets.len(), 1, "only the star on the field is a target");
    assert_eq!(targets[0].0, 0, "the target carries its graph index");
}

/// A star's target is at least as big as the design's minimum, so a
/// three-pixel dot is still something a pointer can land on.
#[test]
fn a_star_is_bigger_to_click_than_to_look_at() {
    let mut list = DrawList::default();
    let targets = sky(Medium::Night).show(&[star(10.0, 8.0, StarState::Done)], &[], &mut list);
    let (_, rect) = targets[0];
    assert!(rect.width >= TARGET_PX && rect.height >= TARGET_PX);
    // Where the star was actually drawn — a lone star is centred in the
    // field ([`Sky::centring`]), so its placement is not its cell
    // coordinate multiplied out.
    let core = list.instances.last().expect("a star");
    let drawn_at = (
        core.rect[0] + core.rect[2] * 0.5,
        core.rect[1] + core.rect[3] * 0.5,
    );
    assert!(
        rect.contains(f64::from(drawn_at.0), f64::from(drawn_at.1)),
        "the target is centred on the star"
    );
}

// ── Loading, and reduced motion ────────────────────────────────────

/// A sky with no roadmap yet is not a blank one: it wears ghost stars,
/// and none of them is clickable, because there is no chunk behind a
/// ghost.
#[test]
fn a_loading_sky_wears_ghosts_that_nothing_can_open() {
    let mut list = DrawList::default();
    let targets = sky(Medium::Night).show(&[], &[], &mut list);
    assert!(targets.is_empty());
    assert_eq!(count(&list, PrimKind::DashedStroke), GHOST_STARS);
}

/// Reduced motion is structural: every wave is pinned to the foot it
/// settles at, so the sky at step 0 and the sky a thousand steps later
/// are the same bytes. Nothing here is a curve nobody ticks.
#[test]
fn reduced_motion_holds_every_wave_at_its_foot() {
    let nodes = every_state();
    let at = |now| drawn(&sky(Medium::Night).at(now, false), &nodes, &[]);
    assert_eq!(bytes(&at(0.0)), bytes(&at(1000.0)));
}

/// An empty field draws nothing rather than dividing by zero — the
/// frame before the layout has placed the block.
#[test]
fn a_field_with_no_room_draws_nothing() {
    let mut list = DrawList::default();
    let empty = PxRect::new(0.0, 0.0, 0.0, 0.0);
    let targets = Sky::new(empty, CELL, 1.0, palette(Medium::Night), Medium::Night).show(
        &every_state(),
        &[],
        &mut list,
    );
    assert!(targets.is_empty());
    assert!(list.instances.is_empty());
}

// ── The ship moment ────────────────────────────────────────────────

/// Two stars with an edge between them: `0` shipped and unlocked `1`.
fn shipped_pair() -> Vec<DagNode> {
    vec![
        star(10.0, 6.0, StarState::Done),
        star(30.0, 6.0, StarState::Ready),
    ]
}

fn run(ignite: f32, edge_run: f32) -> ShipRun {
    ShipRun {
        star: 0,
        region: 1,
        unlocked: vec![1],
        ignite,
        edge_run,
    }
}

fn shipping(medium: Medium, run: ShipRun) -> DrawList {
    let mut list = DrawList::default();
    sky(medium)
        .shipping(Some(run))
        .show(&shipped_pair(), &[(0, 1, EdgeKind::Done)], &mut list);
    list
}

fn settled(medium: Medium) -> DrawList {
    drawn(&sky(medium), &shipped_pair(), &[(0, 1, EdgeKind::Done)])
}

/// A sky with no run draws exactly what it drew before anything
/// shipped — the moment is the only thing that adds instances, so a
/// settled still cannot carry a flare nobody asked for.
#[test]
fn a_sky_with_no_run_is_the_settled_sky() {
    for medium in Medium::ALL {
        let mut list = DrawList::default();
        sky(medium)
            .shipping(None)
            .show(&shipped_pair(), &[(0, 1, EdgeKind::Done)], &mut list);
        assert_eq!(
            list.instances.len(),
            settled(medium).instances.len(),
            "{medium}"
        );
    }
}

/// Night's ignite is light: a real glow behind a core far bigger than
/// the settled dot, both collapsing onto it as the flare settles.
#[test]
fn night_ignites_with_light_that_collapses_to_the_shipped_dot() {
    let flare = shipping(Medium::Night, run(0.0, 0.0));
    let settling = shipping(Medium::Night, run(1.0, 0.0));
    assert!(
        count(&flare, PrimKind::Glow) > count(&settling, PrimKind::Glow),
        "the light goes out as the star settles"
    );
    let widest = |list: &DrawList| {
        list.instances
            .iter()
            .filter(|i| PrimKind::from_raw(i.kind) == Some(PrimKind::Rect))
            .map(|i| i.rect[2])
            .fold(0.0_f32, f32::max)
    };
    assert!(
        widest(&flare) > widest(&settling),
        "the flare is bigger than what it settles to"
    );
}

/// Day's ignite is ink: no light at all, and the settled mark arriving
/// on the page rather than a flare shrinking onto it. A premultiplied
/// colour of zero is a pen that has not touched the paper.
#[test]
fn day_inks_the_mark_in_and_never_flares() {
    let opening = shipping(Medium::Day, run(0.0, 0.0));
    let inked = shipping(Medium::Day, run(1.0, 0.0));
    assert_eq!(count(&opening, PrimKind::Glow), 0, "ink casts no light");
    assert_eq!(count(&inked, PrimKind::Glow), 0);
    assert_ne!(
        opening.instances, inked.instances,
        "the pen has not touched the page yet"
    );
    assert_eq!(
        inked.instances,
        settled(Medium::Day).instances,
        "and what it settles to is exactly the mark every shipped star wears"
    );
}

/// The light runs to the star the completion unlocked and to no other:
/// an edge whose head is not the shipped star carries nothing.
#[test]
fn the_light_runs_only_down_the_edge_it_unlocked() {
    let nodes = shipped_pair();
    let mut lit = DrawList::default();
    sky(Medium::Night).shipping(Some(run(1.0, 0.5))).show(
        &nodes,
        &[(0, 1, EdgeKind::Done)],
        &mut lit,
    );
    let mut dark = DrawList::default();
    sky(Medium::Night)
        .shipping(Some(ShipRun {
            unlocked: Vec::new(),
            ..run(1.0, 0.5)
        }))
        .show(&nodes, &[(0, 1, EdgeKind::Done)], &mut dark);
    assert!(
        lit.instances.len() > dark.instances.len(),
        "an edge to nothing the completion unlocked stays a hairline"
    );
}

/// The light is the shipped chunk's own workstream hue — literally the
/// colour a working star in that workstream wears, so the unlock is
/// signed by the work that caused it and no second taxonomy exists.
#[test]
fn the_light_wears_the_workstream_that_shipped_it() {
    let inks = |region: usize| {
        let mut list = DrawList::default();
        sky(Medium::Day)
            .shipping(Some(ShipRun {
                region,
                ..run(1.0, 0.5)
            }))
            .show(&shipped_pair(), &[(0, 1, EdgeKind::Done)], &mut list);
        list.instances.iter().map(|i| i.color).collect::<Vec<_>>()
    };
    assert_ne!(inks(0), inks(1), "two workstreams ship in two colours");
    let working = |region: usize| {
        let mut node = star(10.0, 8.0, StarState::Ready);
        node.region = region;
        drawn(&sky(Medium::Day), &[node], &[])
            .instances
            .last()
            .expect("a star")
            .color
    };
    for region in 0..4 {
        assert!(
            inks(region).contains(&working(region)),
            "region {region} runs its light in its own workstream hue"
        );
    }
}

/// The travel moves in both media, and only ink stays on the page:
/// day's stroke lengthens as the pen covers the edge, night's lit
/// window slides along it instead of growing.
#[test]
fn the_run_travels_in_both_media_and_only_ink_stays_on_the_page() {
    let stamps = |medium, at: f32| {
        shipping(medium, run(1.0, at))
            .instances
            .into_iter()
            .map(|i| [i.rect[0], i.rect[1]])
            .collect::<Vec<_>>()
    };
    for medium in Medium::ALL {
        let early = stamps(medium, 0.2);
        let late = stamps(medium, 0.9);
        assert_ne!(early, late, "{medium}: the light has not moved");
        assert!(
            early.len() > settled(medium).instances.len(),
            "{medium}: the run adds nothing to the edge"
        );
    }
    assert!(
        stamps(Medium::Day, 0.9).len() > stamps(Medium::Day, 0.2).len(),
        "the pen's stroke lengthens"
    );
    assert!(
        stamps(Medium::Night, 0.9).len() <= stamps(Medium::Night, 0.2).len() + 1,
        "night's lit window slides rather than growing"
    );
}

/// A long dependency edge is never smeared: stamps walk at the ink
/// pitch from both ends and the middle stays open, so a field of long
/// edges cannot dissolve into dot-fog (the real 400-chunk sky did).
#[test]
fn a_long_edge_draws_endpoint_runs_and_leaves_the_middle_open() {
    // Two shipped stars a whole field apart — cells × cell width makes
    // this a ~700 px edge, far past what STAMPS_MAX covers contiguously.
    let nodes = vec![
        star(2.0, 8.0, StarState::Done),
        star(80.0, 8.0, StarState::Done),
    ];
    let list = drawn(&sky(Medium::Night), &nodes, &[(0, 1, EdgeKind::Done)]);
    // An edge stamp is a hairline-sized circle-rect; star cores are
    // three times its size, so the width tells them apart.
    let mut xs: Vec<f32> = list
        .instances
        .iter()
        .filter(|i| PrimKind::from_raw(i.kind) == Some(PrimKind::Rect) && i.rect[2] < 2.0)
        .map(|i| i.rect[0])
        .collect();
    assert!(!xs.is_empty(), "the edge is drawn");
    // Contiguous ink: adjacent stamps in each run sit at the ink pitch,
    // never stretched apart.
    xs.sort_by(f32::total_cmp);
    let pitch = EDGE_PX;
    let mut gaps = 0;
    for pair in xs.windows(2) {
        let gap = pair[1] - pair[0];
        assert!(
            gap <= pitch * 1.1 || gap > pitch * 4.0,
            "a stamp at neither the ink pitch nor a run break: {gap}px"
        );
        if gap > pitch * 4.0 {
            gaps += 1;
        }
    }
    assert_eq!(gaps, 1, "one open middle between the two endpoint runs");
    // And the middle third really is open.
    let (lo, hi) = (xs[0], xs[xs.len() - 1]);
    let third = (hi - lo) / 3.0;
    assert!(
        !xs.iter().any(|&x| x > lo + third && x < hi - third),
        "no stamp in the middle third"
    );
}

/// A short edge is one unbroken run — the fix must not break the
/// fixture-scale roads the goldens hold.
#[test]
fn a_short_edge_stays_one_unbroken_run() {
    let nodes = vec![
        star(2.0, 8.0, StarState::Done),
        star(8.0, 8.0, StarState::Done),
    ];
    let list = drawn(&sky(Medium::Night), &nodes, &[(0, 1, EdgeKind::Done)]);
    let mut xs: Vec<f32> = list
        .instances
        .iter()
        .filter(|i| PrimKind::from_raw(i.kind) == Some(PrimKind::Rect) && i.rect[2] < 2.0)
        .map(|i| i.rect[0])
        .collect();
    xs.sort_by(f32::total_cmp);
    assert!(xs.len() > 2, "a drawn road");
    for pair in xs.windows(2) {
        assert!(
            pair[1] - pair[0] <= EDGE_PX * 1.1,
            "no break in a short road"
        );
    }
}
