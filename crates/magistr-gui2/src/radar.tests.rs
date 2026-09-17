use qs_gpu::frame::PrimKind;

use super::*;

/// The wedge is a sixth of a turn from its leading edge: with the edge
/// at the top, only pixels up and to the right of the centre carry
/// tint; every point is a disc at its thousandths; an empty field
/// draws nothing.
#[test]
fn the_wedge_covers_a_sixth_of_a_turn_and_points_are_discs() {
    let lex = Lexicon::of(Medium::Night);
    let field = PxRect::new(0.0, 50.0, 1100.0, 590.0);
    let mut list = DrawList::default();
    Radar::new(field, 1.0, lex, Medium::Night)
        .at(0.0)
        .show(&[(500, 500), (1000, 0)], &mut list);
    let (cx, cy) = (550.0, 345.0);
    let tinted: Vec<_> = list
        .instances
        .iter()
        .filter(|i| i.kind == PrimKind::Rect as u32 && i.radius == 0.0)
        .collect();
    assert!(!tinted.is_empty(), "the wedge is drawn");
    for run in &tinted {
        let [x, y, w, _] = run.rect;
        assert!(y + 1.0 <= cy, "a run sits above the centre: {y}");
        assert!(x + w > cx - 4.0, "and to its right: {x}+{w}");
    }
    assert!(
        tinted.iter().all(|i| i.rect[1] >= 50.0),
        "no run above the field"
    );
    let discs: Vec<_> = list
        .instances
        .iter()
        .filter(|i| (i.rect[2] - 4.0).abs() < 1e-3 && (i.rect[3] - 4.0).abs() < 1e-3)
        .collect();
    assert_eq!(discs.len(), 2);
    assert!((discs[0].rect[0] + 2.0 - 550.0).abs() < 1e-3);
    assert!((discs[1].rect[1] + 2.0 - 50.0).abs() < 1e-3);
    // Half a turn on, the wedge sits below and to the left.
    let mut turned = DrawList::default();
    Radar::new(field, 1.0, lex, Medium::Night)
        .at(0.5)
        .show(&[], &mut turned);
    assert!(!turned.instances.is_empty());
    assert!(
        turned
            .instances
            .iter()
            .all(|i| i.rect[1] + 1.0 >= cy && i.rect[0] < cx + 4.0),
        "the wedge turned with the phase"
    );
    let mut empty = DrawList::default();
    Radar::new(PxRect::new(0.0, 0.0, 0.0, 0.0), 1.0, lex, Medium::Day).show(&[(1, 1)], &mut empty);
    assert!(empty.instances.is_empty());
}
