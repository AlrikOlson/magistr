use super::*;

#[test]
fn no_models_means_no_picker() {
    assert!(model_options(&[], None).is_none());
    assert!(model_options(&[], Some("x")).is_none());
}

#[test]
fn the_picker_lists_every_model_under_the_model_category_with_the_current_one_chosen() {
    let models = vec![
        ModelChoice::new("fast", "Fast"),
        ModelChoice::new("slow", "Slow"),
    ];
    let options = model_options(&models, Some("slow")).expect("a picker");
    assert_eq!(options.len(), 1);
    let json = serde_json::to_value(&options[0]).expect("serialize");
    assert_eq!(json["id"], "model");
    assert_eq!(json["category"], "model");
    assert_eq!(json["currentValue"], "slow");
    let ids: Vec<&str> = json["options"]
        .as_array()
        .expect("options")
        .iter()
        .filter_map(|o| o["value"].as_str())
        .collect();
    assert_eq!(ids, ["fast", "slow"]);
}

#[test]
fn the_first_model_is_the_default() {
    let models = vec![ModelChoice::new("fast", "Fast")];
    let options = model_options(&models, None).expect("a picker");
    let json = serde_json::to_value(&options[0]).expect("serialize");
    assert_eq!(json["currentValue"], "fast");
}
