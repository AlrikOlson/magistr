use super::*;

#[tokio::test]
async fn a_missing_admitted_directory_refuses_to_launch_the_command() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let absent = tmp.path().join("absent");
    let outcome = run_check_command("exit 0", Some(&absent)).await;
    assert!(!outcome.passed);
    assert_eq!(outcome.exit_code, None);
    assert!(matches!(
        outcome.execution,
        CheckExecution::LaunchFailed { .. }
    ));
}

#[tokio::test]
async fn an_explicit_directory_does_not_change_the_process_directory() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let before = std::env::current_dir().expect("process directory");
    let outcome = run_check_command("exit 0", Some(tmp.path())).await;
    assert!(outcome.passed);
    assert_eq!(outcome.exit_code, Some(0));
    assert_eq!(std::env::current_dir().expect("process directory"), before);
}
