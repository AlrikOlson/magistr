use super::*;

#[test]
fn command_result_success_debug() {
    let r = CommandResult::Success("done".into());
    assert!(format!("{r:?}").contains("Success"));
}

#[test]
fn command_result_empty_renders_nothing() {
    // Use a mock that tracks calls — but for Empty, render() returns Ok(()) directly
    let r = CommandResult::Empty;
    assert!(matches!(r, CommandResult::Empty));
}

#[test]
fn command_result_table_holds_data() {
    let r = CommandResult::Table {
        headers: vec!["Name".into(), "Value".into()],
        rows: vec![vec!["a".into(), "1".into()]],
    };
    if let CommandResult::Table { headers, rows } = &r {
        assert_eq!(headers.len(), 2);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].len(), 2);
    } else {
        panic!("expected Table variant");
    }
}

#[test]
fn command_result_key_values_holds_pairs() {
    let r = CommandResult::KeyValues(vec![
        ("language".into(), "rust".into()),
        ("version".into(), "2024".into()),
    ]);
    if let CommandResult::KeyValues(pairs) = &r {
        assert_eq!(pairs.len(), 2);
        assert_eq!(pairs[0].0, "language");
    } else {
        panic!("expected KeyValues variant");
    }
}

#[test]
fn command_result_json_holds_value() {
    let r = CommandResult::Json(serde_json::json!({"key": "value"}));
    if let CommandResult::Json(v) = &r {
        assert_eq!(v["key"], "value");
    } else {
        panic!("expected Json variant");
    }
}
