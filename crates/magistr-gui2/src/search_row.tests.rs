use super::*;
use crate::element::Kind;
use crate::medium::Medium;

fn title_run(root: &Element) -> Option<(u16, Srgba)> {
    let mut found = None;
    root.each(&mut |_, element| {
        if let Kind::Text { content, voice } = &element.kind
            && content == "Every light palette clears its floor"
        {
            found = Some((voice.weight, element.fill.expect("a text run has ink")));
        }
    });
    found
}

#[test]
fn the_chosen_row_is_washed_and_titled_in_the_text_ink_the_idle_one_bare_and_subtle() {
    let lex = Lexicon::of(Medium::Night);
    let line = || Element::row();
    let chosen = SearchRow::el(
        Some("Every light palette clears its floor"),
        line(),
        true,
        7.0,
        lex.base,
        ElementId(1),
        &lex,
    );
    assert!(chosen.fill.is_some(), "the chosen row wears the wash");
    assert_eq!(title_run(&chosen), Some((550, lex.tx)));
    let idle = SearchRow::el(
        Some("Every light palette clears its floor"),
        line(),
        false,
        10.0,
        lex.base,
        ElementId(2),
        &lex,
    );
    assert!(idle.fill.is_none(), "an idle row sits on nothing");
    assert_eq!(title_run(&idle), Some((400, lex.sb1)));
    assert_eq!(
        idle.pad,
        [10.0, 7.0, 10.0, 10.0],
        "the last row's deeper foot"
    );
    assert_eq!(idle.id, Some(ElementId(2)));
    assert!(idle.focusable);
}

#[test]
fn a_row_without_a_title_is_the_line_alone() {
    let lex = Lexicon::of(Medium::Day);
    let row = SearchRow::el(
        None,
        Element::row(),
        false,
        7.0,
        lex.base,
        ElementId(3),
        &lex,
    );
    assert_eq!(row.children.len(), 1);
    assert!(title_run(&row).is_none());
}
