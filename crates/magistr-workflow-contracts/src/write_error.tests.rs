use super::*;

#[test]
fn failed_mutations_retain_evidence_without_certifying_it() {
    use crate::{Observed, ReadSource, ReadState};
    for error in [
        WriteError::read(ReadError::new(
            ReadErrorKind::Schema,
            "This saved work cannot be opened.",
        )),
        WriteError::before_commit("Your changes could not be saved. Restore access and try again."),
        WriteError::uncertain(
            "Saving could not be confirmed. Read the saved work before retrying.",
        ),
    ] {
        let mut state = ReadState::Current(Observed {
            value: "last saved",
            source: ReadSource::Memory,
        });
        state.record(Err(error.as_read_error()));
        assert!(state.current().is_err());
        assert_eq!(state.last_successful().unwrap().value, "last saved");
        let serialized = serde_json::to_value(&error).unwrap();
        let restored: WriteError = serde_json::from_value(serialized).unwrap();
        assert_eq!(restored, error);
        assert_eq!(
            error.effect(),
            if matches!(error.kind, WriteErrorKind::Uncertain) {
                "unknown"
            } else {
                "none"
            }
        );
    }
}
