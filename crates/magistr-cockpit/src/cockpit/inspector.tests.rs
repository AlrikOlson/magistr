use super::*;
use crate::cockpit::{Selected, Size};
use crate::harness::fixtures;
use serde_json::json;
#[test]
fn copy_and_named_navigation_leave_the_draft_intact() {
    let mut model = Model::new(Size::new(144, 50));
    model.roadmap.seed(&[fixtures::chunk_from(json!({"id":"one","title":"First","notes":"Private context","description":"Original **text**","deps":["two"]})),
        fixtures::chunk_from(json!({"id":"two","title":"Second"}))],&[]);
    model.inspect(Selected::Chunk("one".into()));
    model.session.prompt.insert("unfinished draft");
    let copied = Inspector::activate(&mut model, Widget::detail_copy()).unwrap();
    assert!(
        matches!(&copied[0],crate::cockpit::Cmd::Clipboard(text) if text.contains("Original **text**"))
    );
    Inspector::activate(&mut model, Widget::detail_link(0));
    assert_eq!(model.selected(), Some(&Selected::Chunk("two".into())));
    assert_eq!(model.prompt().text(), "unfinished draft");
}
#[test]
fn section_expansion_keeps_its_source_position() {
    let mut model = Model::new(Size::new(144, 50));
    model.roadmap.seed(
        &[fixtures::chunk_from(
            json!({"id":"one","title":"First","notes":"Private context"}),
        )],
        &[],
    );
    model.inspect(Selected::Chunk("one".into()));
    let card = Inspection::of(&model).unwrap();
    let at = card.sections.iter().position(|s| s.secondary).unwrap();
    Inspector::activate(&mut model, Widget::detail_section(at));
    assert!(card.is_expanded(&model, &card.sections[at]));
    assert_eq!(
        model
            .inspection_state
            .borrow()
            .anchor
            .as_ref()
            .unwrap()
            .section,
        card.sections[at].key
    );
}

#[test]
fn reading_keys_navigate_sections_and_links_with_an_unsent_draft() {
    use crate::cockpit::{Msg, update};
    let mut model = crate::harness::Scenes::rich_record(Size::new(144, 50));
    model.pane = Pane::Inspector;
    let draft = model.prompt().clone();
    let key = |model: &mut Model, code| update(model, Msg::Key(KeyEvent::new(code)));
    key(&mut model, KeyCode::PageDown);
    assert!(model.inspector_scroll > 0);
    update(
        &mut model,
        Msg::Key(KeyEvent::new(KeyCode::Down).with(Modifiers::ALT)),
    );
    let section = model
        .inspection_state
        .borrow()
        .anchor
        .clone()
        .unwrap()
        .section;
    key(&mut model, KeyCode::Right);
    assert!(model.inspection_state.borrow().focused_link.is_some());
    key(&mut model, KeyCode::Enter);
    assert_eq!(model.selected(), Some(&Selected::Chunk("storage".into())));
    assert!(!section.is_empty());
    assert_eq!(model.prompt(), &draft);
}
