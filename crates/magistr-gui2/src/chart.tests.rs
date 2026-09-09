use magistr_cockpit::cockpit::ProposedChunk;
use qs_gpu::frame::PrimKind;

use super::*;

fn proposal() -> Proposal {
    Proposal {
        chunks: vec![
            ProposedChunk::new("a", "core", 100),
            ProposedChunk::new("b", "core", 110).standing_on(&["a"]),
            ProposedChunk::new("c", "api", 120).standing_on(&["b"]),
            ProposedChunk::new("d", "ops", 130).standing_on(&["a"]),
        ],
        sources: Vec::new(),
    }
}

/// A star's column is its dependency depth and its row its order at
/// that depth; the placement stays inside the field's margins.
#[test]
fn stars_are_placed_by_depth_and_order() {
    let placed = Chart::place(&proposal(), (800.0, 500.0));
    assert_eq!(placed.len(), 4);
    let (ax, _) = placed[0];
    let (bx, by) = placed[1];
    let (cx, _) = placed[2];
    let (dx, dy) = placed[3];
    assert!(ax < bx && bx < cx, "deeper stars sit further right");
    assert!((bx - dx).abs() < 1e-3, "b and d share a depth");
    assert!(by < dy, "and stack in list order");
    for (x, y) in placed {
        assert!((800.0 * 0.16..=800.0 * 0.84).contains(&x));
        assert!((500.0 * 0.22..=500.0 * 0.78).contains(&y));
    }
    assert_eq!(
        Chart::place(&Proposal::default(), (800.0, 500.0)),
        Vec::new()
    );
}

/// A star not yet born draws nothing and neither does an edge to it;
/// a born star is a disc in its workstream's hue.
#[test]
fn unborn_stars_and_their_edges_are_not_drawn() {
    let lex = Lexicon::of(Medium::Night);
    let field = PxRect::new(0.0, 0.0, 800.0, 500.0);
    let chart = Chart::new(field, 1.0, lex, Medium::Night);
    let mut list = DrawList::default();
    chart.show(&proposal(), &[1.0, 0.0, 0.0, 0.0], &mut list);
    let discs: Vec<_> = list
        .instances
        .iter()
        .filter(|i| i.kind == PrimKind::Rect as u32 && (i.rect[2] - 10.0).abs() < 1e-3)
        .collect();
    assert_eq!(discs.len(), 1, "only the first star is born");
    assert!(
        list.instances
            .iter()
            .all(|i| (i.rect[2] - 10.0).abs() < 1e-3),
        "no edge reaches an unborn star"
    );
    let mut all = DrawList::default();
    chart.show(&proposal(), &[1.0, 1.0, 1.0, 1.0], &mut all);
    let discs = all
        .instances
        .iter()
        .filter(|i| (i.rect[2] - 10.0).abs() < 1e-3)
        .count();
    assert_eq!(discs, 4);
    assert!(all.instances.len() > 4, "the three edges are stamped");
    assert_eq!(chart.hue(0), lex.mauve);
    assert_eq!(chart.hue(1), lex.blue);
    assert_eq!(chart.hue(2), lex.teal);
    assert_eq!(chart.hue(3), lex.mauve);
}
