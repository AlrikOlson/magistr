use super::*;

fn auth(expires_in: u64, interval: Option<u64>) -> DeviceAuthorization {
    DeviceAuthorization {
        device_code: "dc".into(),
        user_code: "ABCD".into(),
        verification_uri: "https://x.dev".into(),
        verification_uri_complete: None,
        expires_in,
        interval,
    }
}

#[test]
fn a_missing_interval_defaults_to_five_seconds() {
    let schedule = PollSchedule::new(&auth(900, None));
    assert_eq!(schedule.interval(), Duration::from_secs(5));
}

#[test]
fn the_providers_interval_is_honoured() {
    let schedule = PollSchedule::new(&auth(900, Some(11)));
    assert_eq!(schedule.interval(), Duration::from_secs(11));
}

#[test]
fn slow_down_is_cumulative() {
    let mut schedule = PollSchedule::new(&auth(900, Some(5)));
    schedule.slow_down();
    assert_eq!(schedule.interval(), Duration::from_secs(10));
    schedule.slow_down();
    schedule.slow_down();
    assert_eq!(schedule.interval(), Duration::from_secs(20));
}

#[test]
fn charging_runs_out_at_the_device_codes_lifetime() {
    let mut schedule = PollSchedule::new(&auth(12, Some(5)));
    assert!(schedule.charge(), "5s of 12s");
    assert!(schedule.charge(), "10s of 12s");
    assert!(!schedule.charge(), "15s exceeds 12s");
}

#[test]
fn a_slowed_schedule_runs_out_sooner() {
    let mut schedule = PollSchedule::new(&auth(12, Some(5)));
    schedule.slow_down();
    assert!(schedule.charge(), "10s of 12s");
    assert!(!schedule.charge(), "20s exceeds 12s");
}

#[test]
fn remaining_never_underflows() {
    let mut schedule = PollSchedule::new(&auth(4, Some(5)));
    assert_eq!(schedule.remaining(), Duration::from_secs(4));
    assert!(!schedule.charge());
    assert_eq!(schedule.remaining(), Duration::ZERO);
}
