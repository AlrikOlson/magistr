use super::*;

#[cfg(unix)]
#[test]
fn legacy_launch_is_refused_before_any_process_starts() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let marker = tmp.path().join("launched");
    let cli = stub_cli(tmp.path(), JOURNAL_BODY, "", 0);
    let mut workbench = McpWorkbench::new(
        cli,
        vec!["run".into(), "src/main.ts".into(), ".".into()],
        vec![("ARGS_FILE".into(), marker.to_string_lossy().into_owned())],
    );
    let error = workbench.journal().expect_err("legacy server is refused");
    assert!(error.contains("magistr init"), "{error}");
    assert!(
        !marker.exists(),
        "must not launch a standalone journal process"
    );
    assert_eq!(workbench.map().expect_err("same refusal"), error);
}

/// The envelope a real workbench-mcp answered `read {file: "README.md"}`
/// with — captured from the server, not invented.
fn read_envelope() -> Value {
    json!({
        "v": 1,
        "ok": true,
        "freshness": "warm",
        "budget": { "requested": 4000, "used": 59 },
        "results": [
            {
                "kind": "unit",
                "file": "README.md",
                "range": { "start": 1, "end": 2 },
                "totalLines": 2,
                "contentHash": "a948904f2f0f479b8f8197694b30184b0d2ed1c1cd2a1ec0fb85d299a192a447",
            },
            { "kind": "lines", "file": "README.md", "start": 1, "end": 2, "text": "hello world\nsecond" },
        ],
    })
}

/// The envelope a real workbench-mcp answered `search {query: "hello"}` with.
fn search_envelope() -> Value {
    json!({
        "v": 1,
        "ok": true,
        "results": [{
            "file": "README.md",
            "junk": false,
            "totalHits": 1,
            "hits": [{ "line": 1, "column": 1, "excerpt": "  hello world  ", "contextBefore": [], "contextAfter": [""] }],
        }],
    })
}

/// The envelope a real workbench-mcp answered `map` with.
fn map_envelope() -> Value {
    json!({
        "v": 1,
        "ok": true,
        "results": [
            { "kind": "overview", "path": ".", "files": 2, "dirs": 1, "languages": { "rust": 1 } },
            { "kind": "dir", "path": "src", "files": 1, "dirs": 0 },
            { "kind": "file", "path": "README.md", "language": "markdown" },
            { "kind": "file", "path": "src/main.rs", "language": "rust" },
        ],
    })
}

#[test]
fn a_read_comes_back_as_the_files_lines() {
    assert_eq!(content(&read_envelope()), "hello world\nsecond");
}

#[test]
fn a_read_of_a_file_with_no_line_segments_is_empty_not_a_panic() {
    assert_eq!(content(&json!({ "v": 1, "ok": true, "results": [] })), "");
}

#[test]
fn a_searchs_hits_read_as_path_line_and_the_matching_line() {
    assert_eq!(hits(&search_envelope()), vec!["README.md:1: hello world"]);
}

#[test]
fn the_maps_tree_marks_directories_and_drops_the_overview() {
    assert_eq!(
        entries(&map_envelope()),
        vec!["src/", "README.md", "src/main.rs"]
    );
}

#[test]
fn a_runners_report_lists_its_failures_ahead_of_the_verdict() {
    let envelope = json!({
        "v": 1,
        "ok": true,
        "results": [{
            "kind": "report",
            "command": "test",
            "resolvedLine": "cargo nextest run",
            "adapter": "cargo-nextest",
            "exitCode": 1,
            "timedOut": false,
            "durationMs": 4200,
            "summary": "1 failed, 41 passed",
            "failures": [{
                "testName": "workbench::mcp_workbench::tests::a_read",
                "file": "crates/magistr-desktop/src/workbench/mcp_workbench.tests.rs",
                "assertion": "assertion `left == right` failed",
                "trace": "",
            }],
        }],
    });
    let rows = rows(&envelope, "test");
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].name, "workbench::mcp_workbench::tests::a_read");
    assert!(!rows[0].passed);
    assert_eq!(rows[0].detail, "assertion `left == right` failed");
    assert_eq!(rows[1].name, "1 failed, 41 passed");
    assert!(!rows[1].passed);
    assert_eq!(rows[1].detail, "exit 1");
}

#[test]
fn a_compilers_diagnostics_are_named_by_where_they_are() {
    let envelope = json!({
        "v": 1,
        "ok": true,
        "results": [{
            "kind": "diagnostics",
            "command": "check",
            "extractor": "cargo",
            "exitCode": 101,
            "timedOut": false,
            "durationMs": 900,
            "summary": "1 error",
            "diagnostics": [{
                "level": "error",
                "code": "E0308",
                "message": "mismatched types",
                "file": "src/lib.rs",
                "line": 12,
                "column": 5,
                "detail": "",
            }],
        }],
    });
    let rows = rows(&envelope, "check");
    assert_eq!(rows[0].name, "src/lib.rs:12");
    assert_eq!(rows[0].detail, "E0308: mismatched types");
    assert!(!rows[0].passed);
    assert_eq!(rows[1].name, "1 error");
}

#[test]
fn a_run_that_passed_is_one_row_with_nothing_to_say() {
    let envelope = json!({
        "v": 1,
        "ok": true,
        "results": [{
            "kind": "output",
            "command": "fmt",
            "resolvedLine": "cargo fmt",
            "exitCode": 0,
            "timedOut": false,
            "durationMs": 300,
            "stdoutHead": [], "stdoutTail": [], "stderrHead": [], "stderrTail": [],
        }],
    });
    let rows = rows(&envelope, "fmt");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].name, "fmt");
    assert!(rows[0].passed);
    assert_eq!(rows[0].detail, "");
}

#[test]
fn a_run_that_timed_out_says_so_rather_than_naming_an_exit_code() {
    let envelope = json!({
        "v": 1,
        "ok": true,
        "results": [{ "kind": "output", "command": "test", "exitCode": null, "timedOut": true }],
    });
    let rows = rows(&envelope, "test");
    assert!(!rows[0].passed);
    assert_eq!(rows[0].detail, "timed out");
}

// ── Workspace owner journal and refusal fixtures ──

/// A stub `workbench-mcp` CLI: prints `body` on stdout, `stderr` on
/// stderr, exits with `code`, and appends its argv to `$ARGS_FILE`
/// when that variable is set.
#[cfg(unix)]
fn stub_cli(dir: &std::path::Path, body: &str, stderr: &str, code: i32) -> String {
    let path = dir.join("workbench-stub.sh");
    let script = format!(
        "#!/bin/sh\n\
         if [ -n \"$ARGS_FILE\" ]; then printf '%s\\n' \"$*\" >> \"$ARGS_FILE\"; fi\n\
         printf '%s' '{}'\n\
         if [ -n '{}' ]; then printf '%s\\n' '{}' >&2; fi\n\
         exit {}\n",
        body.replace('\'', "'\\''"),
        stderr,
        stderr,
        code
    );
    std::fs::write(&path, script).expect("stub written");
    let mut permissions = std::fs::metadata(&path).expect("stub meta").permissions();
    use std::os::unix::fs::PermissionsExt as _;
    permissions.set_mode(0o755);
    std::fs::set_permissions(&path, permissions).expect("stub executable");
    path.to_string_lossy().into_owned()
}

/// The envelope the real CLI answered `journal . --json --limit 3` with
/// in the workbench-mcp repo — captured, not invented. Newest first.
#[cfg(unix)]
const JOURNAL_BODY: &str = r#"{"v":1,"ok":true,"freshness":"warm","budget":{"requested":0,"used":311},"results":[{"kind":"op","seq":2,"at":1787417454700,"tool":"undo","sessionId":"","undoable":false,"undoOf":[1],"parseOk":true,"path":"src/b.ts","change":"write","added":1,"removed":1,"diff":"--- a/src/b.ts\n+++ b/src/b.ts\n@@ -1 +1 @@\n-old\n+new\n"},{"kind":"op","seq":1,"at":1787417442181,"tool":"edit","sessionId":"","undoable":false,"undoneBy":2,"parseOk":true,"path":"src/a.ts","change":"write","added":2,"removed":0,"diff":"--- a/src/a.ts\n+++ b/src/a.ts\n@@ -1 +1,2 @@\n hello\n+world\n"}]}"#;

// ── Undo, by operation sequence ──

#[test]
fn an_undo_of_a_non_seq_id_is_refused_in_words() {
    let mut workbench =
        McpWorkbench::new("magistr-no-such-workbench-server", Vec::new(), Vec::new());
    let refused = workbench.undo("op-7").expect_err("not a seq");
    assert!(refused.contains("op seq"), "got {refused:?}");
    assert!(
        workbench.refused.is_none(),
        "a bad id never spawns the server"
    );
}

#[test]
fn an_undo_while_the_server_cannot_be_opened_names_who_owns_the_workspace() {
    let mut workbench =
        McpWorkbench::new("magistr-no-such-workbench-server", Vec::new(), Vec::new());
    let refused = workbench.undo("7").expect_err("no server to open");
    assert!(
        refused.contains("ask the agent to undo"),
        "a reason, not a capability error: {refused:?}"
    );
}

#[test]
fn an_undo_answer_with_a_conflict_row_is_a_refusal() {
    let envelope = json!({
        "v": 1, "ok": true,
        "results": [{ "kind": "conflict", "path": "src/a.ts", "opSeq": 7, "reason": "the file moved on" }],
        "notes": [],
    });
    let refused = undone(&envelope, 7).expect_err("a conflict is not a rewind");
    assert!(refused.contains("src/a.ts"), "got {refused:?}");
}

#[test]
fn an_undo_answer_that_skipped_the_named_op_is_a_refusal() {
    let envelope = json!({
        "v": 1, "ok": true,
        "results": [],
        "notes": ["undo: op(s) 7 are not undoable in workspace scope — already undone, an undo's own op, or another session's"],
    });
    let refused = undone(&envelope, 7).expect_err("a skipped op is not a rewind");
    assert!(refused.contains("not undoable"), "got {refused:?}");
}

#[test]
fn an_undo_answer_that_reverted_is_ok() {
    let envelope = json!({
        "v": 1, "ok": true,
        "results": [{ "kind": "op", "seq": 8, "tool": "undo", "undoOf": [7], "path": "src/a.ts" }],
        "notes": [],
    });
    assert_eq!(undone(&envelope, 7), Ok(()));
}

#[test]
fn a_server_that_cannot_be_connected_to_is_refused_once_and_remembered() {
    let mut workbench = McpWorkbench::new(
        "magistr-no-such-workbench-server",
        vec!["mcp".into(), "--dir".into(), ".".into()],
        Vec::new(),
    );
    let first = workbench.map().expect_err("the command does not exist");
    assert!(workbench.refused.is_some(), "the refusal is remembered");
    let second = workbench.map().expect_err("the refusal is answered again");
    assert_eq!(first, second);
}

#[cfg(unix)]
#[test]
fn magistrs_own_launch_line_lists_through_the_owner_connection() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let cli = stub_cli(tmp.path(), "", "", 0);
    let requests = tmp.path().join("requests.txt");
    std::fs::write(&cli, format!(r#"#!/bin/sh
[ "$1" = "mcp" ] && [ "$2" = "--dir" ] || exit 64
while IFS= read -r line; do
  printf '%s\n' "$line" >> "$REQUESTS"
  case "$line" in
    *'"method":"initialize"'*)
      echo '{{"jsonrpc":"2.0","id":1,"result":{{"protocolVersion":"2025-06-18","capabilities":{{}}}}}}'
      ;;
    *'"method":"tools/call"'*)
      printf '%s\n' '{{"jsonrpc":"2.0","id":2,"result":{{"structuredContent":{{"ok":true,"receipts":[],"value":{JOURNAL_BODY}}}}}}}'
      ;;
  esac
done
"#)).expect("bridge");
    let mut workbench = McpWorkbench::new(
        cli,
        vec!["mcp".into(), "--dir".into(), ".".into()],
        vec![("REQUESTS".into(), requests.to_string_lossy().into_owned())],
    );
    let entries = workbench.journal().expect("the owner's journal answers");
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].id, "1");
    assert_eq!(entries[0].path, "src/a.ts");
    assert_eq!(entries[0].added, 2);
    assert_eq!(entries[0].removed, 0);
    assert!(entries[0].parse_ok);
    assert_eq!(entries[0].diff, "@@ -1 +1,2 @@\n hello\n+world");
    assert_eq!(entries[1].id, "2");
    assert_eq!(entries[1].path, "src/b.ts");
    let calls: Vec<Value> = std::fs::read_to_string(requests)
        .expect("requests")
        .lines()
        .map(|line| serde_json::from_str(line).expect("request"))
        .collect();
    let call = calls
        .iter()
        .find(|call| call["method"] == "tools/call")
        .expect("call");
    assert_eq!(call["params"]["name"], "execute");
    let source = call["params"]["arguments"]["source"].as_str().unwrap();
    for expected in ["undo", "list", "workspace", "diff", "200"] {
        assert!(source.contains(expected), "{source}");
    }
}
