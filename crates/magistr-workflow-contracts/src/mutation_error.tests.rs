use super::*;

#[test]
fn owner_wire_preserves_the_uncertain_effect_and_attempted_record() {
    let error = WorkflowMutationError {
        code: "write_uncertain".into(),
        effect: "unknown".into(),
        message: "Read your saved work before retrying.".into(),
        store: Some(Box::new(WriteError::uncertain(
            "Read your saved work before retrying.",
        ))),
        record_id: Some("record-123".into()),
        committed_record: Some(Box::new(crate::Observed {
            value: "chunk-1".into(),
            source: crate::ReadSource::Memory,
        })),
    };
    let wire = serde_json::to_vec(&error).unwrap();
    let received: WorkflowMutationError = serde_json::from_slice(&wire).unwrap();
    assert_eq!(received, error);
    assert_eq!(received.effect(), "unknown");
    assert_eq!(received.record_id.as_deref(), Some("record-123"));
    assert_eq!(
        WorkflowMutationError::rejected("Fix the selection.").effect(),
        "none"
    );
}
