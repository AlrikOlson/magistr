use super::*;
use crate::team::directory::DirectoryConfig;
use magistr_engine::cost::{BudgetPeriod, BudgetRule, BudgetScope};
use magistr_engine::policy::PolicyRequest;

fn identity(name: &str, email: Option<&str>) -> Identity {
    Identity {
        name: name.to_string(),
        email: email.map(String::from),
        provider: None,
    }
}

fn member(id: &str, role: TeamRole) -> TeamMember {
    TeamMember {
        id: id.to_string(),
        name: None,
        role,
        active: true,
    }
}

fn two_owner_team() -> TeamConfig {
    TeamConfig {
        budgets: Vec::new(),
        version: 1,
        default_role: TeamRole::Viewer,
        identity_provider: None,
        policy: GovernancePolicy::default(),
        members: vec![
            member("alice@example.dev", TeamRole::Owner),
            member("bob@example.dev", TeamRole::Owner),
        ],
        directory: DirectoryConfig::default(),
    }
}

#[test]
fn load_absent_file_is_solo_mode() {
    let tmp = tempfile::tempdir().unwrap();
    assert_eq!(TeamConfig::load(tmp.path()).unwrap(), None);
}

#[test]
fn save_load_roundtrip() {
    let tmp = tempfile::tempdir().unwrap();
    let config = TeamConfig::new_with_owner(&identity("Alice", Some("alice@example.dev")));
    config.save(tmp.path()).unwrap();

    let loaded = TeamConfig::load(tmp.path()).unwrap().unwrap();
    assert_eq!(loaded, config);
    assert_eq!(loaded.members.len(), 1);
    assert_eq!(loaded.members[0].id, "alice@example.dev");
    assert_eq!(loaded.members[0].role, TeamRole::Owner);
}

#[test]
fn load_malformed_json_is_parse_error() {
    let tmp = tempfile::tempdir().unwrap();
    let path = TeamConfig::path(tmp.path());
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, "{not json").unwrap();

    let err = TeamConfig::load(tmp.path()).unwrap_err();
    assert!(matches!(err, MagistrError::Parse { .. }), "{err}");
}

#[test]
fn minimal_json_fills_defaults() {
    let config: TeamConfig = serde_json::from_str("{\"members\": []}").unwrap();
    assert_eq!(config.version, 1);
    assert_eq!(config.default_role, TeamRole::Viewer);
    assert!(config.members.is_empty());
}

#[test]
fn member_for_matches_email_case_insensitively() {
    let config = two_owner_team();
    let found = config
        .member_for(&identity("Someone", Some("ALICE@example.dev")))
        .unwrap();
    assert_eq!(found.id, "alice@example.dev");
}

#[test]
fn member_for_matches_name_when_no_email() {
    let mut config = two_owner_team();
    config.members.push(member("carol", TeamRole::Developer));
    let found = config.member_for(&identity("Carol", None)).unwrap();
    assert_eq!(found.role, TeamRole::Developer);
}

#[test]
fn member_for_misses_unknown_identity() {
    let config = two_owner_team();
    assert!(
        config
            .member_for(&identity("mallory", Some("mallory@example.dev")))
            .is_none()
    );
}

#[test]
fn add_member_refuses_duplicate_id() {
    let mut config = two_owner_team();
    let err = config
        .add_member(member("Alice@Example.dev", TeamRole::Viewer))
        .unwrap_err();
    assert!(err.to_string().contains("already"), "{err}");
}

#[test]
fn remove_member_refuses_unknown_and_last_owner() {
    let mut config = TeamConfig::new_with_owner(&identity("Alice", Some("alice@example.dev")));
    assert!(config.remove_member("nobody").is_err());

    let err = config.remove_member("alice@example.dev").unwrap_err();
    assert!(err.to_string().contains("last Owner"), "{err}");
}

#[test]
fn remove_member_allows_owner_when_another_remains() {
    let mut config = two_owner_team();
    config.remove_member("bob@example.dev").unwrap();
    assert_eq!(config.members.len(), 1);
}

#[test]
fn set_role_refuses_demoting_last_owner() {
    let mut config = TeamConfig::new_with_owner(&identity("Alice", Some("alice@example.dev")));
    let err = config
        .set_role("alice@example.dev", TeamRole::Viewer)
        .unwrap_err();
    assert!(err.to_string().contains("last Owner"), "{err}");
}

#[test]
fn set_role_allows_demotion_with_two_owners() {
    let mut config = two_owner_team();
    config
        .set_role("bob@example.dev", TeamRole::Developer)
        .unwrap();
    assert_eq!(config.members[1].role, TeamRole::Developer);
}

#[test]
fn a_roster_without_budgets_publishes_an_empty_policy() {
    let config = two_owner_team();
    assert!(config.budgets.is_empty());
    assert!(!config.policy().is_set());
}

#[test]
fn a_pre_e6_roster_still_loads() {
    let json = r#"{"version":1,"default_role":"viewer","members":[]}"#;
    let config: TeamConfig = serde_json::from_str(json).expect("deserialize");
    assert!(config.budgets.is_empty());
}

#[test]
fn the_roster_carries_the_teams_spending_caps() {
    let json = r#"{
        "version": 1,
        "default_role": "developer",
        "budgets": [
            {"scope": "team", "period": "monthly", "limit": 500.0, "warn_threshold": 400.0},
            {"scope": "session", "limit": 5.0}
        ],
        "members": []
    }"#;
    let config: TeamConfig = serde_json::from_str(json).expect("deserialize");
    let policy = config.policy();
    assert!(policy.is_set());
    assert_eq!(policy.rules.len(), 2);
    assert_eq!(policy.rules[0].scope, BudgetScope::Team);
    assert_eq!(policy.rules[0].period, BudgetPeriod::Monthly);
    assert_eq!(policy.rules[1].period, BudgetPeriod::Session);
    assert_eq!(policy.rules[1].limit, Some(5.0));
}

#[test]
fn budgets_survive_a_save_and_reload() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut config = two_owner_team();
    config.budgets = vec![BudgetRule::new(BudgetScope::Org, BudgetPeriod::Monthly).limit(1000.0)];
    config.save(dir.path()).expect("save");
    let back = TeamConfig::load(dir.path())
        .expect("load")
        .expect("present");
    assert_eq!(back.budgets, config.budgets);
}

// ── Governance policy ─────────────────────────────────────────

#[test]
fn a_roster_with_no_policy_governs_nothing() {
    let config = two_owner_team();
    assert!(config.policy.is_unset());
    assert!(config.policy_engine().is_empty());
}

#[test]
fn the_policy_block_is_omitted_from_an_ungoverned_roster() {
    let json = serde_json::to_string(&two_owner_team()).expect("serialize");
    assert!(!json.contains("\"policy\""), "{json}");
}

#[test]
fn a_pre_e7_roster_still_loads() {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::create_dir_all(crate::paths::magistr_dir(dir.path())).expect("mkdir");
    std::fs::write(
        TeamConfig::path(dir.path()),
        r#"{"version":1,"default_role":"viewer","members":[{"id":"a@b.dev","role":"owner"}]}"#,
    )
    .expect("write");
    let config = TeamConfig::load(dir.path())
        .expect("load")
        .expect("present");
    assert!(config.policy.is_unset());
}

#[test]
fn the_roster_compiles_its_policy_into_an_engine() {
    let mut config = two_owner_team();
    config.policy = GovernancePolicy {
        tool_denylist: vec!["WebFetch".into()],
        protected_paths: vec!["*team.json".into()],
        ..GovernancePolicy::default()
    };
    let engine = config.policy_engine();
    assert_eq!(engine.len(), 2);
    assert_eq!(
        engine.evaluate(&PolicyRequest::tool("WebFetch")).effect,
        magistr_engine::policy::PolicyEffect::Deny
    );
}

#[test]
fn the_policy_survives_a_save_and_reload() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut config = two_owner_team();
    config.policy = GovernancePolicy {
        tool_allowlist: vec!["Read".into(), "Write".into()],
        required_gates: vec!["test".into()],
        approved_models: vec!["claude-*".into()],
        fallback_model: Some("claude-sonnet-5".into()),
        ..GovernancePolicy::default()
    };
    config.save(dir.path()).expect("save");
    let back = TeamConfig::load(dir.path())
        .expect("load")
        .expect("present");
    assert_eq!(back.policy, config.policy);
}

#[test]
fn governance_for_an_absent_roster_is_the_empty_policy() {
    let dir = tempfile::tempdir().expect("tempdir");
    assert!(TeamConfig::governance_for(dir.path()).is_unset());
}

#[test]
fn governance_for_reads_the_roster_when_it_is_there() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut config = two_owner_team();
    config.policy = GovernancePolicy {
        tool_denylist: vec!["Bash".into()],
        ..GovernancePolicy::default()
    };
    config.save(dir.path()).expect("save");
    let policy = TeamConfig::governance_for(dir.path());
    assert_eq!(policy.tool_denylist, vec!["Bash".to_string()]);
}
