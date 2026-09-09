use super::*;

#[test]
fn sort_priority_order() {
    assert!(FileStatus::Modified.sort_priority() < FileStatus::Created.sort_priority());
    assert!(FileStatus::Created.sort_priority() < FileStatus::Deleted.sort_priority());
    assert!(FileStatus::Deleted.sort_priority() < FileStatus::Renamed.sort_priority());
}

#[test]
fn badges() {
    assert_eq!(FileStatus::Modified.badge(), "M");
    assert_eq!(FileStatus::Created.badge(), "A");
    assert_eq!(FileStatus::Deleted.badge(), "D");
    assert_eq!(FileStatus::Renamed.badge(), "R");
}
