use chrono::{Duration, Utc};

use super::*;
use crate::permissions::risk_level::RiskLevel;

fn entry_aged(tool: &str, age_days: i64) -> AuditEntry {
    AuditEntry {
        timestamp: Utc::now() - Duration::days(age_days),
        tool_name: tool.to_string(),
        risk: RiskLevel::Low,
        allowed: true,
        reason: "test".to_string(),
        project: None,
        input_summary: None,
        actor: None,
    }
}

#[test]
fn default_is_unlimited() {
    assert!(AuditRetention::default().is_unlimited());
    assert!(
        !AuditRetention {
            days: Some(30),
            max_entries: None,
        }
        .is_unlimited()
    );
    assert!(
        !AuditRetention {
            days: None,
            max_entries: Some(100),
        }
        .is_unlimited()
    );
}

#[test]
fn unlimited_apply_keeps_everything() {
    let entries = vec![entry_aged("Old", 400), entry_aged("New", 0)];
    let kept = AuditRetention::default().apply(entries, Utc::now());
    assert_eq!(kept.len(), 2);
}

#[test]
fn days_bound_drops_older_entries() {
    let entries = vec![
        entry_aged("Ancient", 40),
        entry_aged("Recent", 5),
        entry_aged("Today", 0),
    ];
    let retention = AuditRetention {
        days: Some(30),
        max_entries: None,
    };
    let kept = retention.apply(entries, Utc::now());
    assert_eq!(kept.len(), 2);
    assert_eq!(kept[0].tool_name, "Recent");
}

#[test]
fn max_entries_keeps_the_tail() {
    let entries = vec![entry_aged("A", 3), entry_aged("B", 2), entry_aged("C", 1)];
    let retention = AuditRetention {
        days: None,
        max_entries: Some(2),
    };
    let kept = retention.apply(entries, Utc::now());
    assert_eq!(kept.len(), 2);
    assert_eq!(kept[0].tool_name, "B");
    assert_eq!(kept[1].tool_name, "C");
}

#[test]
fn both_bounds_compose() {
    let entries = vec![
        entry_aged("Ancient", 40),
        entry_aged("A", 3),
        entry_aged("B", 2),
        entry_aged("C", 1),
    ];
    let retention = AuditRetention {
        days: Some(30),
        max_entries: Some(2),
    };
    let kept = retention.apply(entries, Utc::now());
    assert_eq!(kept.len(), 2);
    assert_eq!(kept[0].tool_name, "B");
    assert_eq!(kept[1].tool_name, "C");
}

#[test]
fn toml_round_trip_inside_permission_config() {
    use crate::permissions::permission_config::PermissionConfig;

    let config = PermissionConfig {
        retention: AuditRetention {
            days: Some(30),
            max_entries: Some(1000),
        },
        ..Default::default()
    };
    let toml = config.to_toml().unwrap();
    assert!(toml.contains("[retention]"));

    let back = PermissionConfig::from_toml(&toml).unwrap();
    assert_eq!(back.retention, config.retention);

    // A config written before retention existed still loads.
    let old = PermissionConfig::from_toml("profile = \"standard\"\n").unwrap();
    assert!(old.retention.is_unlimited());
}
