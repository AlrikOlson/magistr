use super::*;

#[test]
fn refuses_api_key_and_missing_account_without_billing_fallback() {
    for account in [
        Value::Null,
        json!({"type":"apiKey"}),
        json!({"type":"future"}),
    ] {
        let error =
            Handshake::check_account(&json!({"account":account})).expect_err("must require login");
        assert!(error.to_string().contains("ChatGPT"));
    }
    assert!(
        Handshake::check_account(&json!({"account":{"type":"chatgpt","planType":"pro"}})).is_ok()
    );
}

#[test]
fn catalog_defaults_and_efforts_come_from_runtime() {
    let mut handshake = Handshake::default();
    handshake.add_page(&json!({"data":[
        {"id":"a","model":"a","displayName":"A","isDefault":false,"supportedReasoningEfforts":[{"reasoningEffort":"low"}],"defaultReasoningEffort":"low"},
        {"id":"b","model":"b","displayName":"B","isDefault":true,"supportedReasoningEfforts":[{"reasoningEffort":"high"}],"defaultReasoningEffort":"high"}
    ]})).expect("catalog");
    assert_eq!(handshake.models[0].id, "b");
    assert_eq!(handshake.effort("b", None).expect("default"), "high");
    assert!(handshake.effort("b", Some("low")).is_err());
    assert!(handshake.effort("absent", None).is_err());
}
