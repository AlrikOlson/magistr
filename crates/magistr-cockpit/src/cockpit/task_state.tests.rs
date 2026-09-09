use super::*;

#[test]
fn parses_every_named_state() {
    assert_eq!(TaskState::parse("active"), TaskState::Active);
    assert_eq!(TaskState::parse("completed"), TaskState::Completed);
    assert_eq!(TaskState::parse("blocked"), TaskState::Blocked);
    assert_eq!(TaskState::parse("skipped"), TaskState::Skipped);
    assert_eq!(TaskState::parse("planned"), TaskState::Planned);
}

#[test]
fn unknown_names_read_as_planned() {
    assert_eq!(TaskState::parse(""), TaskState::Planned);
    assert_eq!(TaskState::parse("finished"), TaskState::Planned);
}

#[test]
fn every_state_has_a_one_column_glyph() {
    for state in [
        TaskState::Planned,
        TaskState::Active,
        TaskState::Completed,
        TaskState::Blocked,
        TaskState::Skipped,
    ] {
        assert_eq!(state.glyph(&Icons::UNICODE).chars().count(), 1, "{state:?}");
    }
}
