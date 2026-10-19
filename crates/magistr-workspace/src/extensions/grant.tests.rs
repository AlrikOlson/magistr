use super::grant_covers;
use crate::sandbox::CAP_SUBPROCESS;

#[test]
fn grants_authorize_only_the_exact_interface_contract() {
    assert!(grant_covers(CAP_SUBPROCESS, CAP_SUBPROCESS));
    for different in [
        "observe",
        "magistr:subprocess/subprocess@0.1.0",
        "magistr:subprocess/subprocess@0.3.0",
        "magistr:tool/host@0.2.0",
    ] {
        assert!(!grant_covers(different, CAP_SUBPROCESS), "{different}");
    }
}
