//! Closing an owner joins a cancelled attached request beyond rmcp's loop.
use super::*;

use std::{os::unix::fs::PermissionsExt, time::Duration};

use rmcp::{ServiceExt, model::CallToolRequestParams};

async fn owner_with_slow_formatter(root: &std::path::Path) -> Connection {
    let bin = root.join("fake-bin");
    std::fs::create_dir(&bin).expect("fake formatter directory");
    let formatter = bin.join("biome");
    // exec leaves one owned formatter process, never an orphaned shell child.
    std::fs::write(
        &formatter,
        "#!/bin/sh\nprintf '%s\\n' \"${$}\" > \"$MAGISTR_TEST_FORMATTER_PID\"\nexec /bin/sleep 8\n",
    )
    .expect("formatter fixture");
    std::fs::set_permissions(&formatter, std::fs::Permissions::from_mode(0o700))
        .expect("formatter executable");
    let mut paths = vec![bin];
    if let Some(path) = std::env::var_os("PATH") {
        paths.extend(std::env::split_paths(&path));
    }
    let mut child = command(root)
        .env("PATH", std::env::join_paths(paths).expect("fixture PATH"))
        .env("MAGISTR_TEST_FORMATTER_PID", root.join("formatter.pid"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .kill_on_drop(true)
        .spawn()
        .expect("owner");
    let input = child.stdin.take().expect("stdin");
    let output = child.stdout.take().expect("stdout");
    let client = tokio::time::timeout(WAIT, ().serve((output, input)))
        .await
        .expect("initialization deadline")
        .expect("owner initialize");
    Connection { child, client }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn owner_eof_joins_a_cancelled_attached_edit_before_releasing_admission() {
    let dir = workspace();
    let original = "export const value = 1;\n";
    std::fs::write(dir.path().join("fixture.ts"), original).expect("source fixture");
    let owner = owner_with_slow_formatter(dir.path()).await;
    let attached = Connection::open(dir.path()).await;
    let peer = attached.client.peer().clone();
    let pending = tokio::spawn(async move {
        let mut request = CallToolRequestParams::new("edit");
        request.arguments =
            json!({"op":"replace_text", "file":"fixture.ts", "old":"= 1;", "new":"= 2;"})
                .as_object()
                .cloned();
        peer.call_tool(request).await
    });
    let pid_file = dir.path().join("formatter.pid");
    let pid: i32 = tokio::time::timeout(WAIT, async {
        loop {
            if let Some(pid) = std::fs::read_to_string(&pid_file)
                .ok()
                .and_then(|text| text.trim().parse().ok())
            {
                break pid;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("edit reached formatter preparation");

    owner.close().await;
    // SAFETY: signal 0 only observes this fixture's pid; it sends no signal.
    assert_eq!(
        unsafe { libc::kill(pid, 0) },
        -1,
        "formatter outlived owner shutdown"
    );
    assert_eq!(
        std::io::Error::last_os_error().raw_os_error(),
        Some(libc::ESRCH)
    );
    let _ = tokio::time::timeout(WAIT, pending)
        .await
        .expect("pending call settles")
        .expect("pending request task");
    assert_eq!(
        file_text(dir.path(), "fixture.ts"),
        original,
        "cancelled preparation cannot apply later"
    );
    // The attached bridge outlives the owner; its next call admits and the
    // cancelled preparation is not replayed.
    assert_ok(&attached.call("workspace_status", json!({})).await);
    assert_eq!(generation(dir.path()), 2);
    assert_eq!(file_text(dir.path(), "fixture.ts"), original);
    attached.close().await;
}
