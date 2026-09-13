use super::*;

#[test]
fn a_handle_round_trips_through_its_wire_form() {
    let handle = RuntimeHandle::new(4321, Some(1_700_000_000));
    assert_eq!(handle.to_string(), "pid:4321@1700000000");
    assert_eq!(RuntimeHandle::parse(&handle.to_string()), Some(handle));
    let bare = RuntimeHandle::new(7, None);
    assert_eq!(bare.to_string(), "pid:7");
    assert_eq!(RuntimeHandle::parse("pid:7"), Some(bare));
    assert_eq!(RuntimeHandle::parse("receipt:abc"), None);
    assert_eq!(RuntimeHandle::parse("pid:x"), None);
    assert_eq!(RuntimeHandle::parse("pid:7@later"), None);
    assert_eq!(RuntimeHandle::parse("pid:0"), None);
}

#[test]
fn an_observed_handle_names_the_particular_process() {
    let mut child = std::process::Command::new("sleep")
        .arg("30")
        .spawn()
        .expect("a stand-in runtime");
    let handle = RuntimeHandle::observe(child.id()).expect("a live child has a handle");
    assert_eq!(handle.pid(), child.id());
    assert_eq!(handle.is_live(), Some(true));
    // The same pid with a start identity that is not this process's: a
    // reused id, not this runtime.
    let other = RuntimeHandle::new(child.id(), handle.start().map(|start| start ^ 1));
    if handle.start().is_some() {
        assert_eq!(other.is_live(), Some(false));
    }
    child.kill().unwrap();
    child.wait().unwrap();
    assert_eq!(handle.is_live(), Some(false));
    assert_eq!(RuntimeHandle::observe(0), None);
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
#[test]
fn this_host_reports_a_start_identity() {
    let handle = RuntimeHandle::observe(std::process::id()).expect("this process");
    assert!(handle.start().is_some(), "{handle}");
    assert_eq!(
        crate::workflow::process_start::process_start(std::process::id()),
        handle.start()
    );
}
