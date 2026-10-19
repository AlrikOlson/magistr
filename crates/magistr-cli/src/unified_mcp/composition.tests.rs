use super::*;

#[test]
fn source_and_results_are_encoded_as_data_in_the_sandbox() {
    let receipt =
        json!({"operation":"read","arguments":{"file":"a"},"result":{"text":"quote\"\n"}});
    let wrapped = program("return magistr.call('read', {file:'a'});", &[receipt]);
    assert!(wrapped.contains("JSON.stringify(prior.arguments)"));
    assert!(wrapped.contains("history.length"));
    assert!(wrapped.contains(")(magistr, undefined)"));
}

#[test]
fn failure_retains_completed_effects_and_prohibits_automatic_replay() {
    let result = stopped(
        vec![json!({"operation":"write","failed":false})],
        json!({"code":"cancelled"}),
    );
    let value = result.structured_content.unwrap();
    assert_eq!(value["receipts"].as_array().unwrap().len(), 1);
    assert_eq!(value["automaticReplay"], false);
    assert_eq!(result.is_error, Some(true));
}
