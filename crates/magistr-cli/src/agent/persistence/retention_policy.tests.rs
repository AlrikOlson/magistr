use super::*;

#[test]
fn retention_policy_default() {
    let p = RetentionPolicy::default();
    assert_eq!(p.max_age_days, 30);
}
