use super::*;

#[test]
fn composed_results_preserve_both_wire_representations_and_failure() {
    let data = serde_json::json!({"ok":false,"receipts":[{"operation":"write","result":{"applied":true}}]});
    let result = response(data.clone(), true);
    assert_eq!(result.structured_content, Some(data.clone()));
    assert_eq!(
        serde_json::from_str::<Value>(&result.content[0].as_text().unwrap().text).unwrap(),
        data
    );
    assert_eq!(result.is_error, Some(true));
}
