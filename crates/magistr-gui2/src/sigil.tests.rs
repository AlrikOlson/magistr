use super::*;

/// The volume's four, and the names the specimen sheet draws.
const VOLUME: [&str; 4] = Sigil::VOLUME_PROJECTS;

#[test]
fn the_same_name_is_always_the_same_mark() {
    for name in VOLUME {
        assert_eq!(Sigil::of(name), Sigil::of(name));
    }
}

/// A breadcrumb reads `Magistr` and a path reads `magistr`; they are
/// one project, so they wear one mark.
#[test]
fn case_and_surrounding_space_do_not_change_the_mark() {
    assert_eq!(Sigil::of("magistr"), Sigil::of("  Magistr "));
    assert_eq!(Sigil::of("think-and-ship"), Sigil::of("Think-And-Ship"));
}

/// A different project is a different mark — otherwise the sigil says
/// nothing. Positions *and* the order the chain visits them: the second
/// is what the specimen sheet showed to be load-bearing
/// (`a_sigil_is_visibly_a_different_mark_per_project` is the same claim
/// in pixels).
#[test]
fn the_volumes_four_are_four_different_marks() {
    for (index, first) in VOLUME.iter().enumerate() {
        for second in &VOLUME[index + 1..] {
            assert_ne!(
                Sigil::of(first),
                Sigil::of(second),
                "{first} and {second} wear the same mark"
            );
        }
    }
}

/// The derivation must not change shape when the toolchain moves, so
/// the hash is written out rather than borrowed from `std`. This pins
/// what it currently answers: a failure here is a **redesign of every
/// project's identity mark**, never an incidental refactor.
#[test]
fn the_derivation_is_pinned_across_builds() {
    let stars: Vec<(i32, i32)> = Sigil::of("magistr")
        .stars()
        .iter()
        .map(|&(x, y)| ((x * 100.0).round() as i32, (y * 100.0).round() as i32))
        .collect();
    assert_eq!(
        stars,
        vec![(1209, 834), (332, 814), (890, 279), (761, 1269)]
    );
}

/// Every star, plus its own radius, stays on the 16-unit grid the icon
/// set is drawn on — a sigil has to sit in the same box as an icon.
#[test]
fn every_star_stays_on_the_grid() {
    for name in ["magistr", "a", "", "a project with a rather long name", "🜁"] {
        let sigil = Sigil::of(name);
        for &(x, y) in sigil.stars() {
            assert!(
                x - Sigil::STAR >= 0.0
                    && y - Sigil::STAR >= 0.0
                    && x + Sigil::STAR <= Icon::GRID
                    && y + Sigil::STAR <= Icon::GRID,
                "{name:?} puts a star at ({x}, {y})"
            );
        }
    }
}

/// The sector rule is what makes separation structural rather than a
/// rejection loop, so it is checked over a spread of names rather than
/// on one.
#[test]
fn two_stars_never_land_on_each_other() {
    for seed in 0..500u32 {
        let name = format!("project-{seed}");
        let sigil = Sigil::of(&name);
        let stars = sigil.stars();
        assert!((3..=4).contains(&stars.len()), "{name}: {stars:?}");
        for (index, first) in stars.iter().enumerate() {
            for second in &stars[index + 1..] {
                let apart = (first.0 - second.0).hypot(first.1 - second.1);
                assert!(
                    apart > Sigil::STAR * 2.0,
                    "{name}: two stars {apart} apart, closer than their own diameter"
                );
            }
        }
    }
}

/// The mark is a chain over its stars: one segment fewer than there are
/// stars, then every star drawn over the chain so a dot reads as a body
/// rather than as a line cap.
#[test]
fn the_mark_is_a_chain_with_a_star_on_every_node() {
    for name in VOLUME {
        let sigil = Sigil::of(name);
        let shapes = sigil.shapes();
        let segments = shapes
            .iter()
            .filter(|s| matches!(s, Shape::Segment { .. }))
            .count();
        let stars = shapes
            .iter()
            .filter(|s| matches!(s, Shape::Star { .. }))
            .count();
        assert_eq!(stars, sigil.stars().len(), "{name}");
        assert_eq!(segments, sigil.stars().len() - 1, "{name}");
        // Layering: every segment precedes every star.
        let first_star = shapes
            .iter()
            .position(|s| matches!(s, Shape::Star { .. }))
            .expect("a sigil has stars");
        assert_eq!(first_star, segments, "{name}: a star is under the chain");
    }
}

/// Each segment stops short of the stars it runs between, which is what
/// the volume's own four do and what keeps the dot legible.
#[test]
fn a_segment_stops_short_of_the_stars_it_joins() {
    let sigil = Sigil::of("magistr");
    let stars = sigil.stars();
    for (index, shape) in sigil.shapes().iter().enumerate() {
        let Shape::Segment { from, to } = shape else {
            continue;
        };
        let (a, b) = (stars[index], stars[index + 1]);
        let gap = |p: (f32, f32), star: (f32, f32)| (p.0 - star.0).hypot(p.1 - star.1);
        assert!((gap(*from, a) - Sigil::GAP).abs() < 0.01);
        assert!((gap(*to, b) - Sigil::GAP).abs() < 0.01);
    }
}

/// Every element a sigil emits builds a path — a mark that lost a
/// segment would rasterize nearly right, which is exactly what must
/// not happen.
#[test]
fn every_element_of_a_derived_mark_builds() {
    for name in VOLUME {
        for shape in Sigil::of(name).shapes() {
            assert!(shape.path().is_some(), "{name}: {shape:?}");
        }
    }
}
