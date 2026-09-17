use super::*;

use crate::element::Kind;
use crate::element_id::ElementId;
use crate::medium::Medium;
use crate::switch::Switch;

fn texts(root: &Element) -> Vec<String> {
    let mut out = Vec::new();
    root.each(&mut |_, e| {
        if let Kind::Text { content, .. } = &e.kind {
            out.push(content.clone());
        }
    });
    out
}

#[test]
fn the_row_names_the_setting_notes_it_and_seats_the_control() {
    let lex = Lexicon::of(Medium::Night);
    let row = SettingRow::el(
        "Show activity heatmap",
        "On the project view",
        Switch::el(true, ElementId(1), &lex),
        false,
        lex.base,
        &lex,
    );
    assert_eq!(
        texts(&row),
        ["Show activity heatmap", "On the project view"]
    );
    let mut ids = Vec::new();
    row.each(&mut |_, e| {
        if let Some(id) = e.id {
            ids.push(id);
        }
    });
    assert_eq!(ids, [ElementId(1)], "the control keeps its id");
    assert_eq!(row.children.len(), 2, "the words and the control");
}

#[test]
fn a_divided_row_carries_its_hairline_and_the_last_does_not() {
    let lex = Lexicon::of(Medium::Day);
    let divided = SettingRow::el(
        "Density",
        "Row height",
        Element::surface(),
        true,
        lex.base,
        &lex,
    );
    assert_eq!(divided.children.len(), 2);
    assert_eq!(divided.children[1].height, Some(1.0));
    let last = SettingRow::el(
        "Density",
        "Row height",
        Element::surface(),
        false,
        lex.base,
        &lex,
    );
    assert_eq!(last.children.len(), 2, "the row itself: words + control");
    assert!(last.children[1].height.is_none());
}
