use super::*;

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
                "file": "crates/magistr-tui/src/workbench/mcp_workbench.tests.rs",
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

// ── The journal, through the CLI (workbench-journal-and-undo-via-cli) ──

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

#[cfg(unix)]
#[test]
fn the_journal_lists_ops_oldest_first_through_the_cli() {
    let tmp = tempfile::TempDir::new().expect("tempdir");
    let cli = stub_cli(tmp.path(), JOURNAL_BODY, "", 0);
    let mut workbench = McpWorkbench::new(cli, vec![".".to_owned()], Vec::new());

    let entries = workbench.journal().expect("the CLI answers");
    assert_eq!(entries.len(), 2);
    // The CLI is newest-first; the port's contract is oldest-first.
    assert_eq!(entries[0].id, "1");
    assert_eq!(entries[0].path, "src/a.ts");
    assert_eq!(entries[0].added, 2);
    assert_eq!(entries[0].removed, 0);
    assert!(entries[0].parse_ok);
    // The diff keeps its documented @@-body shape — no file headers.
    assert_eq!(entries[0].diff, "@@ -1 +1,2 @@\n hello\n+world");
    assert_eq!(entries[1].id, "2");
    assert_eq!(entries[1].path, "src/b.ts");
}

#[cfg(unix)]
#[test]
fn the_journal_cli_is_derived_from_the_server_launch_line() {
    // The server entry `bun run src/main.ts .` becomes
    // `bun run src/main.ts journal . --json --limit 200` — one source of
    // truth, so the CLI and the server cannot point at different
    // workbenches.
    let tmp = tempfile::TempDir::new().expect("tempdir");
    let cli = stub_cli(tmp.path(), r#"{"v":1,"ok":true,"results":[]}"#, "", 0);
    let argv = tmp.path().join("argv.txt");
    let mut workbench = McpWorkbench::new(
        cli,
        vec!["run".to_owned(), "src/main.ts".to_owned(), ".".to_owned()],
        vec![("ARGS_FILE".to_owned(), argv.to_string_lossy().into_owned())],
    );
    workbench.journal().expect("the CLI answers");
    let recorded = std::fs::read_to_string(&argv).expect("argv recorded");
    assert_eq!(
        recorded.trim(),
        "run src/main.ts journal . --json --limit 200"
    );
}

#[cfg(unix)]
#[test]
fn a_journal_written_by_a_newer_binary_is_told_apart() {
    let tmp = tempfile::TempDir::new().expect("tempdir");
    let cli = stub_cli(
        tmp.path(),
        "",
        "workbench-mcp: journal schema v9 is newer than this binary",
        65,
    );
    let mut workbench = McpWorkbench::new(cli, vec![".".to_owned()], Vec::new());
    let refused = workbench.journal().expect_err("exit 65 is a refusal");
    assert!(
        refused.contains("newer workbench-mcp"),
        "65 is the newer-binary case in words: {refused:?}"
    );
    assert!(
        refused.contains("journal schema v9"),
        "the CLI's own words ride along: {refused:?}"
    );
}

#[cfg(unix)]
#[test]
fn a_missing_workspace_is_told_apart() {
    let tmp = tempfile::TempDir::new().expect("tempdir");
    let cli = stub_cli(
        tmp.path(),
        "",
        "workbench-mcp: workspace root does not exist: /nowhere",
        66,
    );
    let mut workbench = McpWorkbench::new(cli, vec![".".to_owned()], Vec::new());
    let refused = workbench.journal().expect_err("exit 66 is a refusal");
    assert!(
        refused.contains("no workbench journal"),
        "66 is the no-workspace case in words: {refused:?}"
    );
    assert!(refused.contains("/nowhere"), "got {refused:?}");
}

#[test]
fn a_journal_failure_does_not_poison_the_servers_refusal_memo() {
    // The CLI is not the server: a workspace whose listing failed can
    // still connect for a read, and the journal must keep answering
    // while the agent's server holds the exclusive lock.
    let mut workbench =
        McpWorkbench::new("magistr-no-such-workbench-server", Vec::new(), Vec::new());
    let _ = workbench.journal();
    assert!(
        workbench.refused.is_none(),
        "the CLI path never touches the transport memo"
    );
}

// ── Undo, by op seq (workbench-journal-and-undo-via-cli) ────────────

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
    let mut workbench =
        McpWorkbench::new("magistr-no-such-workbench-server", Vec::new(), Vec::new());
    let first = workbench.map().expect_err("the command does not exist");
    assert!(workbench.refused.is_some(), "the refusal is remembered");
    let second = workbench.map().expect_err("the refusal is answered again");
    assert_eq!(first, second);
}

#[cfg(unix)]
#[test]
fn magistrs_own_launch_line_derives_the_native_journal_verb() {
    // `magistr mcp-launch workbench-mcp .` is the native workspace bridge;
    // its journal lives under `magistr workspace journal`.
    let tmp = tempfile::TempDir::new().expect("tempdir");
    let cli = stub_cli(tmp.path(), r#"{"v":1,"ok":true,"results":[]}"#, "", 0);
    let argv = tmp.path().join("argv.txt");
    let mut workbench = McpWorkbench::new(
        cli,
        vec![
            "mcp-launch".to_owned(),
            "workbench-mcp".to_owned(),
            ".".to_owned(),
        ],
        vec![("ARGS_FILE".to_owned(), argv.to_string_lossy().into_owned())],
    );
    workbench.journal().expect("the CLI answers");
    let recorded = std::fs::read_to_string(&argv).expect("argv recorded");
    assert_eq!(recorded.trim(), "workspace journal . --json --limit 200");
}
