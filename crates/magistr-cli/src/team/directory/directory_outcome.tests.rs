use super::*;

use crate::team::TeamRole;

fn mapping() -> Vec<GroupRole> {
    vec![GroupRole {
        group: "eng-*".into(),
        role: TeamRole::Developer,
    }]
}

#[test]
fn a_lookup_that_maps_grants_the_role() {
    let lookup = Ok(Some(vec!["eng-platform".to_string()]));
    assert_eq!(
        DirectoryOutcome::from_lookup(lookup, &mapping()),
        DirectoryOutcome::Granted(TeamRole::Developer)
    );
}

#[test]
fn a_lookup_whose_groups_map_to_nothing_is_an_answer() {
    let lookup = Ok(Some(vec!["finance".to_string()]));
    assert_eq!(
        DirectoryOutcome::from_lookup(lookup, &mapping()),
        DirectoryOutcome::NoMatch
    );
}

/// "I have never heard of this person" is an answer, and an answer is
/// not a refusal — the roster's own fallback applies.
#[test]
fn a_subject_the_directory_does_not_know_is_an_answer_not_a_refusal() {
    assert_eq!(
        DirectoryOutcome::from_lookup(Ok(None), &mapping()),
        DirectoryOutcome::NoMatch
    );
}

/// The whole point of the port's return type: a failure can never be
/// read as "no groups".
#[test]
fn a_lookup_that_failed_is_unavailable_and_never_no_match() {
    let outcome = DirectoryOutcome::from_lookup(
        Err(MagistrError::Other("connection refused".into())),
        &mapping(),
    );
    match outcome {
        DirectoryOutcome::Unavailable(why) => assert!(why.contains("connection refused")),
        other => panic!("a failed lookup must be Unavailable, got {other:?}"),
    }
}

#[test]
fn groups_asserted_directly_take_the_same_mapping() {
    assert_eq!(
        DirectoryOutcome::from_groups(&["eng-tools".to_string()], &mapping()),
        DirectoryOutcome::Granted(TeamRole::Developer)
    );
    assert_eq!(
        DirectoryOutcome::from_groups(&[], &mapping()),
        DirectoryOutcome::NoMatch
    );
}
