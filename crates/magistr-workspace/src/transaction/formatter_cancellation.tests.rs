use super::*;

use std::{os::unix::fs::PermissionsExt, time::Duration};

#[tokio::test]
async fn caller_cancellation_retains_the_formatter_child_until_the_shared_tracker_drains() {
    let root = tempfile::tempdir().expect("root");
    let binary = root.path().join("slow-formatter");
    std::fs::write(
        &binary,
        "#!/bin/sh\nprintf '%s\\n' \"${$}\" > formatter.pid\nexec /bin/sleep 30\n",
    )
    .expect("fixture");
    std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o700)).expect("executable");
    let tasks = TaskTracker::new();
    let formatter = NativeFormatter::new(
        root.path().to_path_buf(),
        FormatterOptions {
            biome: Some(binary),
            ..Default::default()
        },
    )
    .expect("formatter")
    .with_tasks(tasks.clone());
    let formatting = tokio::spawn(async move {
        formatter
            .format("fixture.ts", "export const value = 1;\n")
            .await
    });
    let pid: i32 = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if let Some(pid) = std::fs::read_to_string(root.path().join("formatter.pid"))
                .ok()
                .and_then(|text| text.trim().parse().ok())
            {
                break pid;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("formatter started");
    formatting.abort();
    assert!(
        formatting
            .await
            .expect_err("cancelled caller")
            .is_cancelled()
    );
    tasks.close();
    tokio::time::timeout(Duration::from_secs(5), tasks.wait())
        .await
        .expect("tracked cancellation reaped the child");
    // SAFETY: signal zero only observes this fixture's owned pid.
    assert_eq!(
        unsafe { libc::kill(pid, 0) },
        -1,
        "tracker drained before its child was reaped"
    );
    assert_eq!(
        std::io::Error::last_os_error().raw_os_error(),
        Some(libc::ESRCH)
    );
}
