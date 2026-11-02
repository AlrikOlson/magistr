use super::*;
#[test]
fn failed_report_parsing_and_test_failures_remain_independent_records() {
    let report: ReportRecord = serde_json::from_value(serde_json::json!({
        "format":"junit","path":"results.xml","parsed":false,"error":"Missing"
    }))
    .unwrap();
    assert!(!report.parsed);
    assert_eq!(report.error.as_deref(), Some("Missing"));
    let results: TestResults = serde_json::from_value(serde_json::json!({
        "total":2,"passed":1,"failed":1,"skipped":0,"failures":[{"name":"case"}]
    }))
    .unwrap();
    assert_eq!(results.failures[0].message, None);
    assert_eq!(results.duration_ms, None);
    assert_eq!(
        serde_json::from_value::<TestResults>(serde_json::to_value(&results).unwrap()).unwrap(),
        results
    );
}
