use super::*;
use crate::sandbox::{CAP_SUBPROCESS, is_subprocess_import};

#[test]
fn grants_cover_exact_and_same_major_never_across_majors_or_packages() {
    assert!(grant_covers(CAP_SUBPROCESS, CAP_SUBPROCESS));
    assert!(grant_covers(
        "workbench:subprocess/subprocess@0.1.0",
        CAP_SUBPROCESS
    ));
    assert!(grant_covers(
        CAP_SUBPROCESS,
        "workbench:subprocess/subprocess@0.1.0"
    ));
    assert!(!grant_covers(
        "workbench:subprocess/subprocess@1.0.0",
        CAP_SUBPROCESS
    ));
    assert!(!grant_covers("workbench:tool/host@0.2.0", CAP_SUBPROCESS));
    assert!(grant_covers("observe", "observe"));
    assert!(!grant_covers("observe", CAP_SUBPROCESS));
    assert!(!grant_covers("pkg:iface@0.1", "pkg:iface@0.1.0"));
}

#[test]
fn the_subprocess_stamp_check_accepts_every_major_zero_stamp_and_nothing_else() {
    assert!(is_subprocess_import(
        "workbench:subprocess/subprocess@0.1.0"
    ));
    assert!(is_subprocess_import(
        "workbench:subprocess/subprocess@0.9.4"
    ));
    assert!(!is_subprocess_import(
        "workbench:subprocess/subprocess@1.0.0"
    ));
    assert!(!is_subprocess_import("workbench:tool/host@0.1.0"));
}
