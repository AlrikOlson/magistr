use super::*;

use std::collections::BTreeSet;

/// The 27 supported icon names, in display order.
const ACCEPTANCE: [&str; 27] = [
    "edit",
    "run",
    "read",
    "commit",
    "gate",
    "checkpoint",
    "session",
    "trace",
    "report",
    "recording",
    "filmstrip",
    "image",
    "signal",
    "search",
    "settings",
    "projects",
    "roadmap",
    "folder",
    "clone",
    "done",
    "alert",
    "time",
    "keys",
    "pin",
    "cost",
    "agent",
    "stack",
];

#[test]
fn the_set_is_the_twenty_seven_the_design_names() {
    let names: Vec<&str> = Icon::ALL.iter().copied().map(Icon::name).collect();
    assert_eq!(names, ACCEPTANCE);
}

/// `ALL` is the table everything else is driven from, so a variant
/// missing from it would silently drop out of the specimen, the sweeps
/// and the goldens.
#[test]
fn every_variant_is_in_all() {
    let listed: BTreeSet<Icon> = Icon::ALL.into_iter().collect();
    assert_eq!(listed.len(), Icon::ALL.len(), "no duplicates");
    // `Ord` is derived from declaration order, so the first and last
    // variants bracket the enum; anything added outside the range would
    // fail here rather than quietly.
    assert_eq!(listed.first(), Some(&Icon::Edit));
    assert_eq!(listed.last(), Some(&Icon::Stack));
}

/// Every glyph is the sprite's own geometry: it has elements, each of
/// them builds a path, and every number is on the 16-unit grid.
#[test]
fn every_glyph_is_the_sprites_own_geometry() {
    for icon in Icon::ALL {
        let shapes = icon.shapes();
        assert!(!shapes.is_empty(), "{} has no elements", icon.name());
        for (index, shape) in shapes.iter().enumerate() {
            let path = shape
                .path()
                .unwrap_or_else(|| panic!("{} element {index} does not build", icon.name()));
            let bounds = path.bounds();
            assert!(
                bounds.left() >= -0.6
                    && bounds.top() >= -0.6
                    && bounds.right() <= Icon::GRID + 0.6
                    && bounds.bottom() <= Icon::GRID + 0.6,
                "{} element {index} leaves the grid: {bounds:?}",
                icon.name()
            );
        }
    }
}

/// The signature of the set: a stroke terminal ends in a star-dot. Not every glyph has
/// a free end — a rectangle, a ring and a closed outline have none — but most do, and a
/// set that had lost the signature would show here as a collapse in the count.
#[test]
fn the_set_carries_its_star_dots() {
    let dotted: Vec<&str> = Icon::ALL
        .into_iter()
        .filter(|icon| {
            icon.shapes()
                .iter()
                .any(|shape| matches!(shape, Shape::Star { .. }))
        })
        .map(Icon::name)
        .collect();
    assert_eq!(dotted.len(), 20, "{dotted:?}");
    // Seven reference icons lack terminal dots: report, recording, folder, and cost use
    // closed outlines; settings, clone, and stack have undotted free ends. Keep this
    // exception list explicit so changes require review.
    let undotted: Vec<&str> = Icon::ALL
        .into_iter()
        .filter(|icon| !dotted.contains(&icon.name()))
        .map(Icon::name)
        .collect();
    assert_eq!(
        undotted,
        vec![
            "report",
            "recording",
            "settings",
            "folder",
            "clone",
            "cost",
            "stack"
        ]
    );
}

/// Every variant names a distinct symbol in the volume's sprite, so the
/// port can be checked against the page one row at a time.
#[test]
fn every_glyph_names_its_symbol_in_the_sprite() {
    let ids: BTreeSet<&str> = Icon::ALL.iter().copied().map(Icon::sprite_id).collect();
    assert_eq!(ids.len(), Icon::ALL.len());
    for icon in Icon::ALL {
        assert!(
            icon.sprite_id().starts_with("ic-"),
            "{} names {}",
            icon.name(),
            icon.sprite_id()
        );
    }
}

/// `ic-read` and `ic-search` share the same magnifier geometry in the reference sprite.
/// Exactly one pair may collide; adding or removing a collision requires updating this
/// expectation.
#[test]
fn the_one_pair_the_volume_draws_alike_is_read_and_search() {
    let mut collisions = Vec::new();
    for (index, first) in Icon::ALL.iter().enumerate() {
        for second in &Icon::ALL[index + 1..] {
            if first.shapes() == second.shapes() {
                collisions.push((first.name(), second.name()));
            }
        }
    }
    assert_eq!(collisions, vec![("read", "search")]);
}
