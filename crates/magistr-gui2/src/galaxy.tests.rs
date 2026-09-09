use super::*;

use magistr_cockpit::cockpit::GalaxyProject;
use qs_gpu::frame::PrimKind;

use crate::galaxy_layout::GalaxyLayout;

const FIELD: PxRect = PxRect {
    x: 0.0,
    y: 52.0,
    width: 1100.0,
    height: 553.0,
};

fn projects() -> Vec<GalaxyProject> {
    [
        ("magistr", Some(211), 1, 0),
        ("think-and-ship", None, 1, 1),
        ("quicksilver", None, 0, 0),
    ]
    .into_iter()
    .map(|(name, done, running, asking)| GalaxyProject {
        name: name.to_owned(),
        done,
        running,
        asking,
        ..GalaxyProject::default()
    })
    .collect()
}

fn placed() -> Vec<PlacedCluster> {
    GalaxyLayout::place(&projects(), (FIELD.width, FIELD.height), 1.0)
}

fn galaxy(medium: Medium, now: f32, animate: bool) -> Galaxy {
    Galaxy::new(FIELD, 1.0, Lexicon::of(medium), medium).at(now, animate)
}

fn drawn(galaxy: &Galaxy) -> (DrawList, Vec<(usize, PxRect)>) {
    let mut list = DrawList::default();
    let targets = galaxy.show(&placed(), &mut list);
    (list, targets)
}

fn bytes(list: &DrawList) -> Vec<u32> {
    list.instances
        .iter()
        .flat_map(|i| {
            i.rect
                .iter()
                .chain([i.radius, i.param].iter())
                .map(|f| f.to_bits())
                .chain([i.color, i.kind])
                .collect::<Vec<u32>>()
        })
        .collect()
}

/// A tint as the draw list packs it — the same route the drawing's
/// own instances take, so a colour is matched by the bytes it became
/// and not by a decode this test could get wrong.
fn packed(tint: Srgba) -> u32 {
    Instance::rect(0.0, 0.0, 1.0, 1.0, 0.0, tint).color
}

/// The instances of `list` painted in exactly `tint`.
fn tinted(list: &DrawList, tint: Srgba) -> Vec<&Instance> {
    let packed = packed(tint);
    list.instances
        .iter()
        .filter(|i| i.color == packed)
        .collect()
}

/// The instances of `list` that are shipped stars: filled circles of
/// the shipped radius, whatever their twinkle alpha.
fn shipped(list: &DrawList) -> Vec<&Instance> {
    list.instances
        .iter()
        .filter(|i| {
            i.kind == PrimKind::Rect as u32
                && (i.rect[2] - SHIPPED_PX * 2.0).abs() < 0.01
                && (i.radius - SHIPPED_PX).abs() < 0.01
        })
        .collect()
}

/// Every project is a cluster and answers a click: one target per
/// project, in project order, each a box inside the field.
#[test]
fn every_project_is_a_cluster_with_a_click_target() {
    let (_, targets) = drawn(&galaxy(Medium::Night, 0.0, true));
    assert_eq!(
        targets.iter().map(|(i, _)| *i).collect::<Vec<_>>(),
        vec![0, 1, 2]
    );
    for (_, rect) in &targets {
        assert!(rect.x >= FIELD.x && rect.y >= FIELD.y);
        assert!(rect.x + rect.width <= FIELD.x + FIELD.width + 0.01);
        assert!(rect.y + rect.height <= FIELD.y + FIELD.height + 0.01);
    }
}

/// History is green, a live session breathes mauve and a question
/// breathes amber: the live stars swell between the foot and the crest
/// of the breath, and the shipped ones never change size.
#[test]
fn a_live_session_breathes_mauve_and_a_question_breathes_amber() {
    let lex = Lexicon::of(Medium::Night);
    let opaque = |c: Srgba| Srgba::new(c.r, c.g, c.b, 1.0);
    let foot = drawn(&galaxy(Medium::Night, 0.0, true)).0;
    let crest = drawn(&galaxy(
        Medium::Night,
        Motion::GLOW_STEPS as f32 / 2.0,
        true,
    ))
    .0;
    let radius = |list: &DrawList, color: Srgba| -> f32 {
        tinted(list, opaque(color))
            .iter()
            .map(|i| i.rect[2] / 2.0)
            .fold(0.0, f32::max)
    };
    assert!(
        (radius(&foot, lex.mauve) - LIVE_PX).abs() < 0.01,
        "working at rest is r 4.5"
    );
    assert!(
        radius(&crest, lex.mauve) > radius(&foot, lex.mauve),
        "and swells at the crest"
    );
    assert!(
        (radius(&foot, lex.peach) - LIVE_PX).abs() < 0.01,
        "asking at rest is r 4.5"
    );
    assert!(
        radius(&crest, lex.peach) > radius(&foot, lex.peach),
        "and swells further"
    );
    assert!(
        radius(&crest, lex.peach) > radius(&crest, lex.mauve),
        "amber breathes deeper"
    );
    assert_eq!(shipped(&foot).len(), shipped(&crest).len());
    assert!(!shipped(&foot).is_empty(), "history is on the field");
    assert_eq!(
        tinted(&foot, opaque(lex.mauve)).len(),
        2,
        "two projects have a session running"
    );
    assert_eq!(
        tinted(&foot, opaque(lex.peach)).len(),
        1,
        "one has a question"
    );
}

/// Night's shipped stars twinkle and day's sit still on paper: by day
/// every shipped star shares one ink between frames and the field
/// carries the graticule; at night the shipped stars' alphas move from
/// frame to frame and no paper is ruled.
#[test]
fn ink_never_twinkles_and_only_day_is_drawn_on_paper() {
    let day_a = drawn(&galaxy(Medium::Day, 0.0, true)).0;
    let day_b = drawn(&galaxy(Medium::Day, 5.0, true)).0;
    let night_a = drawn(&galaxy(Medium::Night, 0.0, true)).0;
    let night_b = drawn(&galaxy(Medium::Night, 5.0, true)).0;
    let paper = |list: &DrawList| {
        list.instances
            .iter()
            .filter(|i| {
                i.kind == PrimKind::Rect as u32
                    && (i.rect[2] >= FIELD.width || i.rect[3] >= FIELD.height)
            })
            .count()
    };
    assert!(paper(&day_a) > 10, "day's field is ruled");
    assert_eq!(paper(&night_a), 0, "night is flat ground");
    let inks = |list: &DrawList| -> Vec<u32> { shipped(list).iter().map(|i| i.color).collect() };
    assert!(!inks(&day_a).is_empty());
    assert_eq!(inks(&day_a), inks(&day_b), "ink never twinkles");
    assert!(
        inks(&day_a).iter().all(|c| *c == inks(&day_a)[0]),
        "day's stars share one ink"
    );
    assert_ne!(
        inks(&night_a),
        inks(&night_b),
        "night's shipped stars twinkle"
    );
    let lex = Lexicon::of(Medium::Night);
    let live = |list: &DrawList| {
        tinted(list, Srgba::new(lex.mauve.r, lex.mauve.g, lex.mauve.b, 1.0)).len()
    };
    assert_eq!(live(&night_a), live(&night_b), "the breath moves in both");
}

/// Reduced motion is structural: every wave is pinned to its foot, so
/// two frames at different steps are byte-identical, and the live
/// stars stand at their rest radius — the frame a settled still draws.
#[test]
fn reduced_motion_holds_every_wave_at_its_foot() {
    let a = drawn(&galaxy(Medium::Night, 0.0, false)).0;
    let b = drawn(&galaxy(Medium::Night, 11.0, false)).0;
    assert_eq!(bytes(&a), bytes(&b));
    let lex = Lexicon::of(Medium::Night);
    let opaque = |c: Srgba| Srgba::new(c.r, c.g, c.b, 1.0);
    for tint in [lex.mauve, lex.peach] {
        for star in tinted(&a, opaque(tint)) {
            assert!((star.rect[2] / 2.0 - LIVE_PX).abs() < 0.01, "at rest");
        }
    }
    let shipped_alphas: Vec<u32> = shipped(&a).iter().map(|i| i.color).collect();
    assert!(
        shipped_alphas.iter().all(|c| *c == shipped_alphas[0]),
        "no twinkle under reduced motion"
    );
}

/// A selected cluster wears a ring on its head star; nothing else
/// strokes a ring, and an empty field draws nothing and answers no
/// target.
#[test]
fn only_the_selected_cluster_wears_a_ring() {
    let none = drawn(&galaxy(Medium::Night, 0.0, false)).0;
    let some = {
        let mut list = DrawList::default();
        galaxy(Medium::Night, 0.0, false)
            .selecting(Some(1))
            .show(&placed(), &mut list);
        list
    };
    let strokes = |list: &DrawList| {
        list.instances
            .iter()
            .filter(|i| i.kind == PrimKind::Stroke as u32)
            .count()
    };
    assert_eq!(strokes(&none), 0);
    assert_eq!(strokes(&some), 1);
    let empty = Galaxy::new(
        PxRect::new(0.0, 0.0, 0.0, 0.0),
        1.0,
        Lexicon::of(Medium::Night),
        Medium::Night,
    );
    let mut list = DrawList::default();
    assert!(empty.show(&placed(), &mut list).is_empty());
    assert!(list.instances.is_empty());
}
