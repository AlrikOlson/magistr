use super::*;

#[test]
fn an_edit_with_a_diff_keeps_its_actual_execution_status() {
    for status in [
        ToolStatus::Failed,
        ToolStatus::InProgress,
        ToolStatus::Completed,
    ] {
        let mut turn = Turn::new(80);
        turn.push_edit("edit", "source.rs", status, "@@ -1 +1 @@\n-old\n+new");
        let card = ConversationCard::of(&turn, &turn.lines()[0]).unwrap();
        assert!(
            card.status.starts_with(status_label(status)),
            "{}",
            card.status
        );
        assert!(card.status.contains("+1 −1"), "{}", card.status);
        assert_eq!(card.failed, status == ToolStatus::Failed);
    }
}
