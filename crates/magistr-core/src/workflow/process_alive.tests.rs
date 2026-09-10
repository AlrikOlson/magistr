use super::*;

#[test]
fn this_process_is_alive_and_a_reaped_child_is_not() {
    assert_eq!(is_process_alive(std::process::id()), Some(true));
    let mut child = std::process::Command::new("true").spawn().unwrap();
    let pid = child.id();
    child.wait().unwrap();
    assert_eq!(is_process_alive(pid), Some(false));
    assert_eq!(is_process_alive(0), None);
}
