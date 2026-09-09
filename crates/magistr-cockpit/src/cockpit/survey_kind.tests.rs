use super::*;

#[test]
fn the_four_facts_wear_the_volumes_keys_in_sweep_order() {
    let keys: Vec<&str> = SurveyKind::ALL.iter().map(|k| k.label()).collect();
    assert_eq!(keys, ["stack", "tests", "git", "agent"]);
}
