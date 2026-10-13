use super::*;

#[test]
fn only_the_exact_in_flight_observation_releases_credit() {
    let credit = Credit::default();
    assert!(credit.offer(3));
    assert!(!credit.offer(4));
    assert!(credit.acknowledge(2).is_err());
    assert!(credit.acknowledge(4).is_err());
    assert!(!credit.offer(5));
    assert!(credit.acknowledge(3).is_ok());
    assert!(credit.acknowledge(3).is_err());
    assert!(credit.offer(8));
}
