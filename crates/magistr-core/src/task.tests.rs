use super::*;

#[test]
fn active_task_lifecycle() {
    let mut task = ActiveTask {
        id: "tool_123".into(),
        description: "Searching codebase".into(),
        status: TaskStatus::Running,
    };
    assert_eq!(task.status, TaskStatus::Running);

    task.status = TaskStatus::Done;
    assert_eq!(task.status, TaskStatus::Done);
}

#[test]
fn task_status_equality() {
    assert_eq!(TaskStatus::Running, TaskStatus::Running);
    assert_ne!(TaskStatus::Running, TaskStatus::Failed);
    assert_ne!(TaskStatus::Done, TaskStatus::Failed);
}
