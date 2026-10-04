use super::*;
#[test]
fn unsupported_addresses_are_unavailable_and_never_executed() {
    let model = Model::new(crate::cockpit::Size::new(80, 24));
    assert_eq!(
        link(&model, "javascript:alert(1)", "run", 0).target,
        RecordTarget::Unavailable
    );
    assert_eq!(
        link(&model, "https://example.com", "docs", 0).target,
        RecordTarget::Address("https://example.com".into())
    );
}

#[test]
fn a_tools_output_is_one_document_section() {
    use crate::cockpit::{Msg, Size, update};
    use magistr_application::sessions::{AgentEvent, ToolRecord, ToolStatus, ToolVerb};
    let mut model = Model::new(Size::new(120, 40));
    update(
        &mut model,
        Msg::Agent(AgentEvent::ToolCall {
            id: "read".into(),
            title: "Read saved results".into(),
            verb: ToolVerb::Read,
            status: ToolStatus::Completed,
            diff: None,
            images: vec![],
            record: Box::new(ToolRecord {
                text: vec!["First line".into(), "Second line".into()],
                ..Default::default()
            }),
        }),
    );
    model.inspect(Selected::Tool("read".into()));
    let card = Inspection::of(&model).expect("tool document");
    let contents: Vec<_> = card
        .sections
        .iter()
        .filter(|s| s.heading == "Contents")
        .collect();
    assert_eq!(contents.len(), 1, "one heading for the output body");
    assert_eq!(contents[0].text.source(), "First line\nSecond line");
}
