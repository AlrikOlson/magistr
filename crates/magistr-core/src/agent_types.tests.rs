use super::*;

#[test]
fn plan_entry_priority_indicator() {
    assert_eq!(PlanEntryPriority::High.indicator(), "!");
    assert_eq!(PlanEntryPriority::Medium.indicator(), "-");
    assert_eq!(PlanEntryPriority::Low.indicator(), "·");
}

#[test]
fn plan_entry_status_icon() {
    assert_eq!(PlanEntryStatus::Pending.icon(), "○");
    assert_eq!(PlanEntryStatus::InProgress.icon(), "◐");
    assert_eq!(PlanEntryStatus::Completed.icon(), "✓");
}

#[test]
fn config_option_current_value_name_found() {
    let opt = AcpConfigOption {
        id: "model".into(),
        name: "Model".into(),
        description: None,
        category: Some(ConfigOptionCategory::Model),
        current_value: "sonnet".into(),
        options: vec![
            ConfigSelectValue {
                value_id: "sonnet".into(),
                name: "Sonnet".into(),
            },
            ConfigSelectValue {
                value_id: "opus".into(),
                name: "Opus".into(),
            },
        ],
    };
    assert_eq!(opt.current_value_name(), "Sonnet");
}

#[test]
fn config_option_current_value_name_fallback() {
    let opt = AcpConfigOption {
        id: "model".into(),
        name: "Model".into(),
        description: None,
        category: None,
        current_value: "unknown-id".into(),
        options: vec![],
    };
    assert_eq!(opt.current_value_name(), "unknown-id");
}
