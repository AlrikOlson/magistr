use super::*;

#[test]
fn a_bare_state_is_clean_and_has_no_branch() {
    let repo = RepoState::at("0123456789abcdef");
    assert!(repo.is_clean());
    assert_eq!(repo.branch, None);
    assert_eq!(repo.dirty, 0);
}

#[test]
fn a_dirty_tree_is_not_described_by_its_head() {
    let repo = RepoState::at("0123456789abcdef").with_dirty(3);
    assert!(!repo.is_clean(), "three files are not in that commit");
}

#[test]
fn the_short_head_is_what_git_prints() {
    assert_eq!(RepoState::at("0123456789abcdef").short_head(), "0123456");
}

#[test]
fn a_head_shorter_than_seven_is_not_truncated_past_its_end() {
    assert_eq!(RepoState::at("abc").short_head(), "abc");
}

#[test]
fn a_state_round_trips_through_serialization() {
    let repo = RepoState::at("0123456789abcdef")
        .on_branch("master")
        .with_dirty(2);
    let json = serde_json::to_string(&repo).expect("serialize");
    let parsed: RepoState = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(parsed, repo);
}

#[test]
fn a_detached_head_omits_the_branch_rather_than_inventing_one() {
    let json = serde_json::to_string(&RepoState::at("abc")).expect("serialize");
    assert!(!json.contains("branch"), "{json}");
}

#[test]
fn a_record_carrying_only_a_head_parses() {
    let parsed: RepoState = serde_json::from_str(r#"{"head":"abc"}"#).expect("deserialize");
    assert_eq!(parsed, RepoState::at("abc"));
}
