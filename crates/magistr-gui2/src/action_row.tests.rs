use super::*;
use crate::element_id::ElementId;

/// one lit exit, counted over a built tree: however a composition is assembled, at most
/// one element wears a gradient in the primary's green — and the type makes more than
/// one impossible to ask for, since the primary is a single `Option` slot.
#[test]
fn at_most_one_element_wears_the_primary_gradient() {
    let row = ActionRow::new(
        Some(Action::new("Approve & merge", ElementId(1)).cap("A")),
        vec![
            Action::new("Request changes…", ElementId(2)),
            Action::new("Deny", ElementId(3)).cap("D").negative(),
        ],
    )
    .el(Medium::Night);
    let lex = Lexicon::of(Medium::Night);
    let primary_top = oklab::mix(WHITE, lex.green, 0.18);
    let mut lit = 0;
    row.each(&mut |_, element| {
        if let Some([top, _]) = element.gradient
            && (top.r - primary_top.r).abs() < 1e-6
            && (top.g - primary_top.g).abs() < 1e-6
        {
            lit += 1;
        }
    });
    assert_eq!(lit, 1, "one lit exit");
}

/// placement law by construction: a labelled action carries a verb and maybe its key
/// cap, never a mark — no ActionRow tree holds a mark node, whatever mix of faces it is
/// built from. The kind positions (a severity cap, a receipt key, a crumb) each carry
/// their one mark; a button cannot.
#[test]
fn a_labelled_action_cannot_carry_a_mark() {
    let row = ActionRow::new(
        Some(Action::new("Approve & merge", ElementId(1)).cap("A")),
        vec![
            Action::new("Promote", ElementId(2)).accent(),
            Action::new("Deny", ElementId(3)).negative(),
            Action::new("Later", ElementId(4)).disabled(),
        ],
    )
    .el(Medium::Night);
    let mut marks = 0;
    row.each(&mut |_, element| {
        if matches!(element.kind, crate::element::Kind::Mark { .. }) {
            marks += 1;
        }
    });
    assert_eq!(marks, 0, "buttons stay text verbs");
}

/// The accent face is the mauve raise, distinct from both the primary
/// and the neutral secondary, in both media.
#[test]
fn the_accent_face_is_its_own_raise() {
    for medium in [Medium::Night, Medium::Day] {
        let accent =
            ActionRow::new(None, vec![Action::new("Promote", ElementId(5)).accent()]).el(medium);
        let neutral = ActionRow::new(None, vec![Action::new("Promote", ElementId(5))]).el(medium);
        let top_of = |row: &Element| {
            let mut top = None;
            row.each(&mut |_, element| {
                if let Some([t, _]) = element.gradient {
                    top = Some((t.r, t.g, t.b));
                }
            });
            top.expect("a raised face")
        };
        assert_ne!(top_of(&accent), top_of(&neutral), "{medium}");
    }
}

/// A row can also offer no exit at all — every button neutral.
#[test]
fn a_row_without_a_primary_has_no_lit_exit() {
    let row = ActionRow::new(None, vec![Action::new("Later", ElementId(9))]).el(Medium::Night);
    let lex = Lexicon::of(Medium::Night);
    let primary_top = oklab::mix(WHITE, lex.green, 0.18);
    row.each(&mut |_, element| {
        if let Some([top, _]) = element.gradient {
            assert!((top.r - primary_top.r).abs() > 1e-6, "no green ramp here");
        }
    });
}

/// A disabled action keeps its place and loses its answers: no id, no
/// focus, no gradient — the flat disabled face.
#[test]
fn a_disabled_action_answers_nothing() {
    let row =
        ActionRow::new(None, vec![Action::new("Deny", ElementId(4)).disabled()]).el(Medium::Night);
    let button = &row.children[0];
    assert!(button.disabled);
    assert!(button.id.is_none());
    assert!(!button.focusable);
    assert!(button.gradient.is_none(), "a disabled exit is not lit");
}

/// The key cap rides inside the button as a data pill in the button's
/// own ink at 85%.
#[test]
fn the_cap_rides_inside_the_button() {
    let row = ActionRow::new(
        Some(Action::new("Approve & merge", ElementId(1)).cap("A")),
        Vec::new(),
    )
    .el(Medium::Night);
    let button = &row.children[0];
    assert_eq!(button.children.len(), 2, "the verb and its cap");
    let cap = &button.children[1];
    assert_eq!(cap.radius, 5.0);
    assert!(
        cap.fill.is_some_and(|fill| fill.a < 1.0),
        "a tint, not a paint"
    );
}
