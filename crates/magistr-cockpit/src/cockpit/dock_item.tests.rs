use super::*;

const ICONS: &Icons = &Icons::UNICODE;

#[test]
fn a_permission_card_names_the_tool() {
    let card = DockItem::Permission(Permission::new(1, "Bash", Vec::new()));
    assert_eq!(card.text(ICONS), "⚠ Bash needs permission");
}

#[test]
fn an_edit_permission_is_the_diff_approval_card() {
    let card = DockItem::Permission(Permission::new(1, "Edit README.md", Vec::new()).edit());
    assert_eq!(card.text(ICONS), "✎ Edit README.md — approve the diff");
}

#[test]
fn a_proposal_card_names_the_chunk_the_field_and_the_suggestion() {
    let card = DockItem::Proposal {
        chunk: "auth".to_owned(),
        kind: ProposalKind::Status,
        suggestion: "done".to_owned(),
        reason: "tracker closed it".to_owned(),
    };
    assert_eq!(
        card.text(ICONS),
        "◆ auth: status → done (tracker closed it)"
    );
}

#[test]
fn a_blocker_card_names_the_chunk_and_the_reason() {
    let card = DockItem::Blocker {
        chunk: "auth".to_owned(),
        reason: "needs a decision".to_owned(),
    };
    assert_eq!(card.text(ICONS), "⊗ auth blocked: needs a decision");
}
