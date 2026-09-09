use super::*;
use crate::element::Kind;
use crate::fixture::Fixture;
use crate::medium::Medium;

fn texts(root: &Element) -> Vec<String> {
    let mut out = Vec::new();
    root.each(&mut |_, element| {
        if let Kind::Text { content, .. } = &element.kind {
            out.push(content.clone());
        }
    });
    out
}

/// The tile is the 16/10 box at the card radius with its ring, the
/// picture inside it at a height that fits, and the cap naming the
/// still's family.
#[test]
fn the_tile_is_a_ringed_box_with_the_picture_fitted_and_the_cap_in_the_corner() {
    let lex = Lexicon::of(Medium::Night);
    let shelf = Fixture::shelf();
    let still = &shelf.stills[0];
    let tile = StillTile::el(still, 139.0, &lex);
    assert_eq!(tile.width, Some(139.0));
    assert_eq!(tile.height, Some(87.0), "16/10, rounded");
    assert_eq!(tile.radius, 8.0);
    assert!(tile.stroke.is_some(), "the volume's hairline ring");
    assert!(tile.id.is_none(), "a tile takes no click");
    let mut picture = None;
    tile.each(&mut |_, element| {
        if matches!(element.kind, Kind::Picture { .. }) {
            picture = Some((element.width, element.height));
        }
    });
    let (w, h) = picture.expect("the picture is drawn");
    assert!(h.expect("a height") <= 85.0, "inside the ring: {h:?}");
    assert!(w.expect("a width") <= 137.0, "inside the ring: {w:?}");
    assert_eq!(texts(&tile), vec!["still \u{b7} ", "turn"]);
}

/// A wide golden is fitted by its width, a tall one by its height —
/// contain, never cover, and never scaled up.
#[test]
fn a_wide_golden_fits_by_width_and_a_tall_one_by_height() {
    let lex = Lexicon::of(Medium::Day);
    let shelf = Fixture::shelf();
    let filmstrip = shelf
        .stills
        .iter()
        .find(|s| s.kind == magistr_cockpit::cockpit::StillKind::Filmstrip)
        .expect("a filmstrip on the fixture");
    let (w, h) = filmstrip.size();
    assert!(w > 2 * h, "the fixture's filmstrip is wide");
    let tile = StillTile::el(filmstrip, 139.0, &lex);
    let mut picture = None;
    tile.each(&mut |_, element| {
        if matches!(element.kind, Kind::Picture { .. }) {
            picture = Some((element.width, element.height));
        }
    });
    let (pw, ph) = picture.expect("drawn");
    assert!(
        pw.expect("w") <= 137.0 && ph.expect("h") < 85.0,
        "{pw:?}x{ph:?}"
    );
}

#[test]
fn the_stem_is_the_goldens_family() {
    assert_eq!(stem("turn-night.png"), "turn");
    assert_eq!(stem("sky-rest-night.png"), "sky");
    assert_eq!(stem("roadmap-mocha-80x24.png"), "roadmap");
    assert_eq!(stem("plain.png"), "plain");
    assert_eq!(stem("noext"), "noext");
}
