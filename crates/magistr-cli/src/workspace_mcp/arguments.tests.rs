use super::*;
use magistr_workspace::commands::{EditOp, FilesOp, UndoScope};
use serde_json::json;

fn object(value: Value) -> JsonObject {
    value.as_object().cloned().expect("object")
}

fn message(error: ErrorData) -> String {
    error.message.into_owned()
}

#[test]
fn read_requires_a_file_and_refuses_symbol_with_lines() {
    assert_eq!(
        message(read_input(&object(json!({}))).unwrap_err()),
        "read: file must be a non-empty string"
    );
    assert_eq!(
        message(
            read_input(&object(
                json!({"file": "a.rs", "symbol": "x", "lines": {"start": 1, "end": 2}})
            ))
            .unwrap_err()
        ),
        "read: symbol and lines are mutually exclusive (SDD §9.3)"
    );
    let input = read_input(&object(json!({
        "file": "a.rs", "lines": {"start": 3, "end": 9}, "surround": "imports", "budget": 12
    })))
    .expect("valid");
    assert_eq!(input.lines, Some(ReadRange { start: 3, end: 9 }));
    assert_eq!(input.surround, ReadSurround::Imports);
    assert_eq!(input.budget, 12);
    assert!(
        read_input(&object(
            json!({"file": "a.rs", "lines": {"start": 0, "end": 2}})
        ))
        .is_err()
    );
    assert!(read_input(&object(json!({"file": "a.rs", "surround": "sideways"}))).is_err());
}

#[test]
fn search_and_map_take_the_retained_defaults() {
    assert_eq!(
        message(search_input(&object(json!({"query": ""}))).unwrap_err()),
        "search: query must be a non-empty string"
    );
    let search = search_input(&object(json!({"query": "x", "mode": "lexical"}))).expect("valid");
    assert_eq!((search.context, search.budget), (2, 4000));
    let map = map_input(&object(json!({}))).expect("defaults");
    assert_eq!((map.path.as_str(), map.depth, map.budget), (".", 3, 6000));
    assert_eq!(map.detail, MapDetail::Outline);
    assert!(map_input(&object(json!({"depth": 17}))).is_err());
    assert!(map_input(&object(json!({"detail": "deep"}))).is_err());
}

#[test]
fn edit_admission_mirrors_the_retained_checks() {
    let cases = [
        (
            json!({}),
            "edit: op must be one of replace_body, insert_after, insert_before, rename, rewrite, replace_text",
        ),
        (
            json!({"op": "replace_text"}),
            "edit: replace_text requires a non-empty file",
        ),
        (
            json!({"op": "rename", "file": "a.rs"}),
            "edit: rename requires a non-empty symbol",
        ),
        (
            json!({"op": "rename", "file": "a.rs", "symbol": "s"}),
            "edit: rename requires a non-empty newName",
        ),
        (
            json!({"op": "replace_body", "file": "a.rs", "symbol": "s"}),
            "edit: replace_body requires content",
        ),
        (
            json!({"op": "rewrite", "pattern": "a"}),
            "edit: rewrite requires rewritePattern",
        ),
        (
            json!({"op": "replace_text", "file": "a.rs", "old": "x"}),
            "edit: replace_text requires new",
        ),
        (
            json!({"op": "replace_text", "file": "a.rs", "old": "x", "new": "y", "confirm": 7}),
            "edit: confirm must be a string token",
        ),
    ];
    for (args, expected) in cases {
        assert_eq!(message(edit_input(&object(args)).unwrap_err()), expected);
    }
    let input = edit_input(&object(json!({
        "op": "replace_text", "file": "a.rs", "old": "x", "new": "", "occurrence": 2, "dryRun": true
    })))
    .expect("valid");
    assert_eq!(input.op, EditOp::ReplaceText);
    assert_eq!(input.new.as_deref(), Some(""));
    assert_eq!(input.occurrence, Some(2));
    assert!(input.dry_run);
}

#[test]
fn files_admission_requires_the_paths_each_op_needs() {
    assert_eq!(
        message(files_input(&object(json!({"op": "rm"}))).unwrap_err()),
        "files: op must be one of move, copy, create, delete, mkdir"
    );
    assert_eq!(
        message(files_input(&object(json!({"op": "delete"}))).unwrap_err()),
        "files: delete requires a non-empty from"
    );
    assert_eq!(
        message(files_input(&object(json!({"op": "create"}))).unwrap_err()),
        "files: create requires a non-empty to"
    );
    let input = files_input(&object(json!({"op": "move", "from": "a", "to": "b"}))).expect("valid");
    assert_eq!(input.op, FilesOp::Move);
}

#[test]
fn write_admission_requires_a_path_and_string_content_and_authorizes_nothing() {
    assert_eq!(
        message(write_input(&object(json!({"content": "x"}))).unwrap_err()),
        "write: path must be a non-empty string"
    );
    assert_eq!(
        message(write_input(&object(json!({"path": "a.txt"}))).unwrap_err()),
        "write: content must be a string"
    );
    assert_eq!(
        message(
            write_input(&object(
                json!({"path": "a.txt", "content": "", "confirm": 7})
            ))
            .unwrap_err()
        ),
        "write: confirm must be a string token"
    );
    let input = write_input(&object(json!({
        "path": "a.txt", "content": "", "expectedHash": "h", "dryRun": true
    })))
    .expect("an empty file is a valid write");
    assert_eq!(input.path, "a.txt");
    assert_eq!(input.expected_hash.as_deref(), Some("h"));
    assert!(input.dry_run);
    assert!(!input.auto_apply, "a served write is not pre-authorized");
}

#[test]
fn run_and_script_admission_refuse_schema_level_misuse() {
    assert_eq!(
        message(run_input(&object(json!({}))).unwrap_err()),
        "run: command must be a non-empty string"
    );
    assert_eq!(
        message(run_input(&object(json!({"command": "test", "timeout": 0}))).unwrap_err()),
        "run: timeout must be an integer in [1, 3600] seconds"
    );
    assert_eq!(
        message(run_input(&object(json!({"command": "test", "args": [1]}))).unwrap_err()),
        "run: args must be an array of strings"
    );
    let run = run_input(&object(
        json!({"command": "test", "args": ["--x"], "timeout": 5}),
    ))
    .expect("valid");
    assert_eq!(
        (run.args.as_slice(), run.timeout_secs, run.budget),
        (&["--x".to_owned()][..], 5, 4000)
    );
    assert_eq!(
        message(script_input(&object(json!({"source": "return 1", "fuel": 0}))).unwrap_err()),
        "script: fuel must be a positive integer"
    );
    let script = script_input(&object(json!({"source": "return 1"}))).expect("valid");
    assert!(script.dry_run);
}

#[test]
fn undo_splits_rewind_from_list_with_their_own_defaults() {
    assert_eq!(
        message(undo_request(&object(json!({"action": "peek"}))).unwrap_err()),
        "undo: action must be \"rewind\" or \"list\""
    );
    assert_eq!(
        message(undo_request(&object(json!({"steps": 101}))).unwrap_err()),
        "undo: steps must be an integer in [1, 100]"
    );
    assert_eq!(
        message(undo_request(&object(json!({"ops": []}))).unwrap_err()),
        "undo: ops must be an array of 1 to 100 journal seqs"
    );
    assert_eq!(
        message(undo_request(&object(json!({"limit": 0}))).unwrap_err()),
        "undo: limit must be an integer in [1, 200]"
    );
    assert_eq!(
        message(undo_request(&object(json!({"sinceSeq": -1}))).unwrap_err()),
        "undo: sinceSeq must be a non-negative integer journal seq"
    );
    assert_eq!(
        message(undo_request(&object(json!({"scope": "global"}))).unwrap_err()),
        "undo: scope must be \"session\" or \"workspace\""
    );
    assert_eq!(
        message(undo_request(&object(json!({"onConflict": "merge"}))).unwrap_err()),
        "undo: onConflict must be \"surface\", \"force\" or \"keepCurrent\""
    );
    match undo_request(&object(json!({}))).expect("rewind") {
        UndoRequest::Rewind(input) => {
            assert_eq!(
                (input.steps, input.scope, input.budget),
                (1, UndoScope::Session, 4000)
            );
        }
        UndoRequest::List(_) => panic!("rewind is the default action"),
    }
    match undo_request(&object(
        json!({"action": "list", "limit": 5, "diff": false}),
    ))
    .expect("list")
    {
        UndoRequest::List(input) => {
            assert_eq!(
                (input.scope, input.limit, input.diff, input.budget),
                (UndoScope::Workspace, 5, false, 8000)
            );
        }
        UndoRequest::Rewind(_) => panic!("list was asked for"),
    }
}
