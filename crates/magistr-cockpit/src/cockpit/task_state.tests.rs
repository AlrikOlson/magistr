use magistr_engine::native::TaskStatus;

use super::*;

#[test]
fn every_ship_status_maps_to_its_state() {
    assert_eq!(TaskState::from(TaskStatus::Active), TaskState::Active);
    assert_eq!(TaskState::from(TaskStatus::Completed), TaskState::Completed);
    assert_eq!(TaskState::from(TaskStatus::Blocked), TaskState::Blocked);
    assert_eq!(TaskState::from(TaskStatus::Skipped), TaskState::Skipped);
    assert_eq!(TaskState::from(TaskStatus::Planned), TaskState::Planned);
    assert_eq!(TaskState::default(), TaskState::Planned);
}
