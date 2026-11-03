use super::*;
use crate::people::directory::DirectoryConfig;
use magistr_engine::cost::{BudgetPeriod, BudgetRule, BudgetScope};
use magistr_engine::policy::PolicyRequest;

fn identity(name: &str, email: Option<&str>) -> Identity {
    Identity {
        name: name.to_string(),
        email: email.map(String::from),
        provider: None,
    }
}

fn member(id: &str, role: PersonRole) -> Person {
    Person {
        id: id.to_string(),
        name: None,
        role,
        active: true,
    }
}

fn two_owner_team() -> PeopleConfig {
    PeopleConfig {
        budgets: Vec::new(),
        version: 1,
        default_role: PersonRole::Viewer,
        identity_provider: None,
        policy: GovernancePolicy::default(),
        members: vec![
            member("alice@example.dev", PersonRole::Owner),
            member("bob@example.dev", PersonRole::Owner),
        ],
        directory: DirectoryConfig::default(),
    }
}

#[test]
fn load_absent_file_is_solo_mode() {
    let tmp = tempfile::tempdir().unwrap();
    assert_eq!(PeopleConfig::load(tmp.path()).unwrap(), None);
}

#[test]
fn save_load_roundtrip() {
    let tmp = tempfile::tempdir().unwrap();
    let config = PeopleConfig::new_with_owner(&identity("Alice", Some("alice@example.dev")));
    config.save(tmp.path()).unwrap();

    let loaded = PeopleConfig::load(tmp.path()).unwrap().unwrap();
    assert_eq!(loaded, config);
    assert_eq!(loaded.members.len(), 1);
    assert_eq!(loaded.members[0].id, "alice@example.dev");
    assert_eq!(loaded.members[0].role, PersonRole::Owner);
}

#[test]
fn load_malformed_json_is_parse_error() {
    let tmp = tempfile::tempdir().unwrap();
    let path = PeopleConfig::path(tmp.path());
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, "{not json").unwrap();

    let err = PeopleConfig::load(tmp.path()).unwrap_err();
    assert!(matches!(err, MagistrError::Parse { .. }), "{err}");
}

#[test]
fn minimal_json_fills_defaults() {
    let config: PeopleConfig = serde_json::from_str("{\"members\": []}").unwrap();
    assert_eq!(config.version, 1);
    assert_eq!(config.default_role, PersonRole::Viewer);
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
    config.members.push(member("carol", PersonRole::Developer));
    let found = config.member_for(&identity("Carol", None)).unwrap();
    assert_eq!(found.role, PersonRole::Developer);
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
        .add_member(member("Alice@Example.dev", PersonRole::Viewer))
        .unwrap_err();
    assert!(err.to_string().contains("already"), "{err}");
}

#[test]
fn remove_member_refuses_unknown_and_last_owner() {
    let mut config = PeopleConfig::new_with_owner(&identity("Alice", Some("alice@example.dev")));
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
    let mut config = PeopleConfig::new_with_owner(&identity("Alice", Some("alice@example.dev")));
    let err = config
        .set_role("alice@example.dev", PersonRole::Viewer)
        .unwrap_err();
    assert!(err.to_string().contains("last Owner"), "{err}");
}

#[test]
fn set_role_allows_demotion_with_two_owners() {
    let mut config = two_owner_team();
    config
        .set_role("bob@example.dev", PersonRole::Developer)
        .unwrap();
    assert_eq!(config.members[1].role, PersonRole::Developer);
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
    let config: PeopleConfig = serde_json::from_str(json).expect("deserialize");
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
    let config: PeopleConfig = serde_json::from_str(json).expect("deserialize");
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
    let back = PeopleConfig::load(dir.path())
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
        PeopleConfig::path(dir.path()),
        r#"{"version":1,"default_role":"viewer","members":[{"id":"a@b.dev","role":"owner"}]}"#,
    )
    .expect("write");
    let config = PeopleConfig::load(dir.path())
        .expect("load")
        .expect("present");
    assert!(config.policy.is_unset());
}

#[test]
fn the_roster_compiles_its_policy_into_an_engine() {
    let mut config = two_owner_team();
    config.policy = GovernancePolicy {
        tool_denylist: vec!["WebFetch".into()],
        protected_paths: vec!["*people.json".into()],
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
        required_checks: vec!["test".into()],
        approved_models: vec!["claude-*".into()],
        fallback_model: Some("claude-sonnet-5".into()),
        ..GovernancePolicy::default()
    };
    config.save(dir.path()).expect("save");
    let back = PeopleConfig::load(dir.path())
        .expect("load")
        .expect("present");
    assert_eq!(back.policy, config.policy);
}

#[test]
fn governance_for_an_absent_roster_is_the_empty_policy() {
    let dir = tempfile::tempdir().expect("tempdir");
    assert!(PeopleConfig::governance_for(dir.path()).is_unset());
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
    let policy = PeopleConfig::governance_for(dir.path());
    assert_eq!(policy.tool_denylist, vec!["Bash".to_string()]);
}

fn legacy_roster() -> serde_json::Value {
    serde_json::json!({
        "version": 1,
        "default_role": "viewer",
        "policy": {"protected_paths": ["*team.json", "*/migrations/*"]},
        "members": [{"id": "alice@example.dev", "role": "owner", "active": true}]
    })
}

#[test]
fn legacy_team_file_moves_to_people_once_keeping_a_backup() {
    let tmp = tempfile::tempdir().unwrap();
    let magistr = tmp.path().join(".magistr");
    std::fs::create_dir_all(&magistr).unwrap();
    let original = serde_json::to_string_pretty(&legacy_roster()).unwrap();
    std::fs::write(magistr.join("team.json"), &original).unwrap();

    let loaded = PeopleConfig::load(tmp.path()).unwrap().expect("roster");
    assert_eq!(loaded.members[0].id, "alice@example.dev");
    assert_eq!(
        loaded.policy.protected_paths,
        vec!["*people.json".to_string(), "*/migrations/*".to_string()],
        "a guard on the old roster file keeps guarding the roster"
    );
    assert!(!magistr.join("team.json").exists());
    assert!(PeopleConfig::path(tmp.path()).exists());
    let backup = magistr.join("team.json.backup");
    assert_eq!(std::fs::read_to_string(&backup).unwrap(), original);

    let again = PeopleConfig::load(tmp.path()).unwrap().expect("roster");
    assert_eq!(again, loaded, "the upgrade is idempotent");
    assert_eq!(std::fs::read_to_string(&backup).unwrap(), original);
}

#[test]
fn people_file_wins_and_a_leftover_team_file_is_untouched() {
    let tmp = tempfile::tempdir().unwrap();
    let magistr = tmp.path().join(".magistr");
    std::fs::create_dir_all(&magistr).unwrap();
    two_owner_team().save(tmp.path()).unwrap();
    std::fs::write(magistr.join("team.json"), "not json").unwrap();

    let loaded = PeopleConfig::load(tmp.path()).unwrap().expect("roster");
    assert_eq!(loaded, two_owner_team());
    assert_eq!(
        std::fs::read_to_string(magistr.join("team.json")).unwrap(),
        "not json"
    );
}

#[test]
fn a_policy_saved_before_the_check_words_takes_them_once_keeping_the_original() {
    let tmp = tempfile::tempdir().unwrap();
    let magistr = tmp.path().join(".magistr");
    std::fs::create_dir_all(&magistr).unwrap();
    let mut roster = legacy_roster();
    roster["policy"]["required_gates"] = serde_json::json!(["test", "lint"]);
    roster["policy"]["allow_gate_bypass"] = serde_json::json!(true);
    let original = serde_json::to_string_pretty(&roster).unwrap();
    std::fs::write(PeopleConfig::path(tmp.path()), &original).unwrap();

    let loaded = PeopleConfig::load(tmp.path()).unwrap().expect("roster");
    assert_eq!(loaded.policy.required_checks, vec!["test", "lint"]);
    assert!(loaded.policy.allow_check_bypass);
    let saved = std::fs::read_to_string(PeopleConfig::path(tmp.path())).unwrap();
    assert!(!saved.contains("_gate"), "{saved}");
    let backup = magistr.join("people.json.before-check-words");
    assert_eq!(std::fs::read_to_string(&backup).unwrap(), original);

    let again = PeopleConfig::load(tmp.path()).unwrap().expect("roster");
    assert_eq!(again, loaded, "the conversion is idempotent");
}

#[test]
fn a_policy_holding_both_check_spellings_is_refused() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(tmp.path().join(".magistr")).unwrap();
    let mut roster = legacy_roster();
    roster["policy"]["required_gates"] = serde_json::json!(["test"]);
    roster["policy"]["required_checks"] = serde_json::json!(["lint"]);
    std::fs::write(PeopleConfig::path(tmp.path()), roster.to_string()).unwrap();
    assert!(PeopleConfig::load(tmp.path()).is_err());
}
