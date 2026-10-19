//! Real pinned SQLite databases cross the native storage and undo boundaries.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use magistr_workspace::file_content::FileContent;
use magistr_workspace::index::cache::IndexCache;
use magistr_workspace::journal::{
    JournalDb, JournalDbOptions, JournalFileChange, JournalFileKind, JournalLogEntry,
    JournalLogQuery, JournalMode, JournalOpInit, JournalReader, UndoCandidateQuery,
};
use magistr_workspace::undo::{UndoConflictPolicy, UndoPlan};
use rusqlite::{Connection, OpenFlags, types::ValueRef};
use serde_json::{Value, json};

fn corpus() -> Value {
    serde_json::from_str(include_str!(
        "../../../subsystems/workspace/conformance/journal.json"
    ))
    .expect("reference fixture manifest")
}

fn decode_base64(encoded: &str) -> Vec<u8> {
    let mut decoded = Vec::new();
    let mut accumulator = 0_u32;
    let mut bits = 0_u32;
    for byte in encoded.bytes().take_while(|byte| *byte != b'=') {
        let value = match byte {
            b'A'..=b'Z' => byte - b'A',
            b'a'..=b'z' => byte - b'a' + 26,
            b'0'..=b'9' => byte - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            _ => panic!("invalid fixture base64 byte"),
        };
        accumulator = (accumulator << 6) | u32::from(value);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            decoded.push((accumulator >> bits) as u8);
            accumulator &= (1 << bits) - 1;
        }
    }
    decoded
}

fn materialize(directory: &Path, fixture: &Value) -> PathBuf {
    let compressed = decode_base64(fixture["database_zstd_base64"].as_str().expect("database"));
    let bytes = zstd::decode_all(compressed.as_slice()).expect("reference zstd database");
    assert!(bytes.starts_with(b"SQLite format 3\0"));
    let path = directory.join("journal.db");
    std::fs::write(&path, bytes).expect("materialize reference SQLite file");
    path
}

fn snapshot(entries: &[JournalLogEntry]) -> Value {
    Value::Array(
        entries
            .iter()
            .map(|entry| {
                let op = &entry.op;
                json!({
                    "op": {
                        "seq": op.seq, "at": op.at, "tool": op.tool,
                        "args_hash": op.args_hash, "session_id": op.session_id,
                        "undo_of": op.undo_of,
                        "files": op.files.iter().map(|file| json!({
                            "kind": match file.kind {
                                JournalFileKind::Create => "create",
                                JournalFileKind::Write => "write",
                                JournalFileKind::Delete => "delete",
                            },
                            "path": file.path,
                            "before_hash": file.before_hash,
                            "after_hash": file.after_hash,
                            "before": file.before.as_ref().map(FileContent::as_bytes),
                            "after": file.after.as_ref().map(FileContent::as_bytes),
                        })).collect::<Vec<_>>()
                    },
                    "undoable": entry.undoable, "undone_by": entry.undone_by,
                })
            })
            .collect(),
    )
}

fn full_query() -> JournalLogQuery {
    JournalLogQuery {
        limit: 100,
        ..JournalLogQuery::default()
    }
}

fn assert_raw_tables(path: &Path, expected: &Value) {
    let connection = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .expect("inspect reference SQLite tables");
    for (table, order) in [
        ("ops", "seq"),
        ("op_files", "op_seq, ord"),
        ("undone", "op_seq"),
        ("blobs", "hash"),
    ] {
        let sql = format!("SELECT * FROM {table} ORDER BY {order}");
        let mut statement = connection.prepare(&sql).expect("table query");
        let columns: Vec<String> = statement
            .column_names()
            .into_iter()
            .map(str::to_owned)
            .collect();
        let rows = statement
            .query_map([], |row| {
                let mut object = serde_json::Map::new();
                for (column, name) in columns.iter().enumerate() {
                    let value = match row.get_ref(column)? {
                        ValueRef::Null => Value::Null,
                        ValueRef::Integer(value) => json!(value),
                        ValueRef::Real(value) => json!(value),
                        ValueRef::Text(value) => {
                            json!(std::str::from_utf8(value).expect("table text"))
                        }
                        ValueRef::Blob(value) => json!(value),
                    };
                    object.insert(name.clone(), value);
                }
                Ok(Value::Object(object))
            })
            .expect("read table")
            .collect::<Result<Vec<_>, _>>()
            .expect("table rows");
        let mut expected_rows = expected[table].as_array().expect("expected rows").clone();
        if table == "blobs" {
            for row in &mut expected_rows {
                row["zstd"] = json!(decode_base64(row["zstd"].as_str().expect("zstd blob")));
            }
        }
        assert_eq!(rows, expected_rows, "{table} preserved byte-for-byte");
    }
}

#[test]
fn pinned_v1_and_v2_sqlite_history_migrates_without_rewriting_blobs_or_records() {
    let corpus = corpus();
    for fixture in corpus["fixtures"].as_array().expect("fixtures") {
        let root = tempfile::tempdir().expect("fixture root");
        let path = materialize(root.path(), fixture);
        let original = std::fs::read(&path).expect("original database");
        let version_probe = Connection::open_with_flags(&path, OpenFlags::SQLITE_OPEN_READ_ONLY)
            .expect("fixture version probe");
        let fixture_version: i64 = version_probe
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .expect("fixture version");
        assert_eq!(
            fixture_version,
            fixture["schema_version"].as_i64().expect("version")
        );
        drop(version_probe);
        let reader = JournalReader::open(&path).expect("read pinned database");
        assert_eq!(
            snapshot(&reader.recent(&full_query()).expect("recent")),
            fixture["recent"]
        );
        drop(reader);
        assert_eq!(std::fs::read(&path).expect("database after read"), original);
        assert_raw_tables(&path, &fixture["tables"]);

        let mut options = JournalDbOptions::new(&path);
        options.journal_mode = JournalMode::Truncate;
        let journal = JournalDb::open(options).expect("migrate pinned database");
        assert_eq!(
            snapshot(&journal.recent(&full_query()).expect("recent")),
            fixture["recent"]
        );
        let ops = journal.list(0).expect("append order");
        assert_eq!(
            ops.len(),
            fixture["recent"].as_array().expect("recent fixture").len()
        );
        assert!(ops.windows(2).all(|pair| pair[0].seq < pair[1].seq));
        assert_eq!(ops[0].files[0].path, "zeta.txt");
        assert_eq!(ops[0].files[1].path, "alpha.txt");
        assert_eq!(
            ops[0].files[1]
                .after
                .as_ref()
                .expect("BOM content")
                .as_bytes(),
            b"\xef\xbb\xbfother\n"
        );
        assert!(journal.get(999).expect("missing op").is_none());
        drop(journal);

        assert_raw_tables(&path, &fixture["tables"]);
        let db = Connection::open_with_flags(&path, OpenFlags::SQLITE_OPEN_READ_ONLY)
            .expect("version probe");
        let version: i64 = db
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .expect("version");
        assert_eq!(version, 2);
    }
}

#[test]
fn pinned_undo_relationships_and_session_scopes_survive_native_open() {
    let corpus = corpus();
    let root = tempfile::tempdir().expect("fixture root");
    let path = materialize(root.path(), &corpus["fixtures"][1]);
    let journal = JournalDb::open(JournalDbOptions::new(path)).expect("native journal");
    let query = UndoCandidateQuery {
        session_id: Some("alpha".to_owned()),
        limit: 100,
        seqs: Some(vec![8, 7, 6, 4, 3, 2, 1]),
    };
    let candidates = journal.undo_candidates(&query).expect("named candidates");
    assert_eq!(
        candidates.iter().map(|op| op.seq).collect::<Vec<_>>(),
        [8, 7, 6, 1]
    );
    let recent = journal
        .recent(&JournalLogQuery {
            session_id: Some("alpha".to_owned()),
            limit: 2,
            since_seq: Some(4),
            before_seq: Some(8),
        })
        .expect("bounded recent");
    assert_eq!(
        recent.iter().map(|entry| entry.op.seq).collect::<Vec<_>>(),
        [7, 6]
    );
    assert_eq!(
        journal.get(4).expect("undo op").expect("op 4").undo_of,
        Some(vec![3])
    );
    assert_eq!(
        journal
            .get(9)
            .expect("bookkeeping undo")
            .expect("op 9")
            .undo_of,
        Some(vec![2])
    );
}

#[test]
fn rebuilding_a_corrupt_native_cache_preserves_the_real_durable_journal() {
    let corpus = corpus();
    let fixture = &corpus["fixtures"][1];
    let root = tempfile::tempdir().expect("workspace root");
    let state_dir = root.path().join(".magistr/workspace");
    std::fs::create_dir_all(&state_dir).expect("state directory");
    let path = materialize(&state_dir, fixture);
    let original = std::fs::read(&path).expect("journal before rebuild");
    std::fs::write(
        state_dir.join("native-index.db"),
        b"deliberately corrupt cache",
    )
    .expect("corrupt expendable cache");
    let cache = IndexCache::open(&state_dir, "TRUNCATE").expect("rebuild native index");
    assert!(
        cache
            .rebuild_reason
            .as_deref()
            .is_some_and(|reason| reason.contains("corrupt"))
    );
    drop(cache);
    assert_eq!(
        std::fs::read(&path).expect("journal after rebuild"),
        original
    );
    let reader = JournalReader::open(&path).expect("durable reader after rebuild");
    assert_eq!(
        snapshot(&reader.recent(&full_query()).expect("retained history")),
        fixture["recent"]
    );
    drop(reader);
    assert_raw_tables(&path, &fixture["tables"]);
}

#[test]
fn restored_binary_bytes_come_from_the_pinned_database() {
    let corpus = corpus();
    let root = tempfile::tempdir().expect("fixture root");
    let path = materialize(root.path(), &corpus["fixtures"][1]);
    let journal = JournalDb::open(JournalDbOptions::new(path)).expect("native journal");
    let candidates = journal
        .undo_candidates(&UndoCandidateQuery {
            session_id: Some("alpha".to_owned()),
            limit: 1,
            seqs: Some(vec![8]),
        })
        .expect("binary restoration candidate");
    let actual = BTreeMap::from([(
        "zeta.txt".to_owned(),
        Some(FileContent::Text("final\n".to_owned())),
    )]);
    let plan = UndoPlan::build(&candidates, &actual, UndoConflictPolicy::Surface)
        .expect("restoration plan");
    assert!(plan.conflicts.is_empty());
    assert_eq!(plan.undo_of, [8]);
    assert_eq!(plan.writes.len(), 1);
    assert_eq!(plan.writes[0].path, "zeta.txt");
    let restored = plan.writes[0]
        .content
        .as_ref()
        .expect("binary restored content");
    assert!(matches!(restored, FileContent::Binary(_)));
    assert_eq!(restored.as_bytes(), [255, 128, 10]);
    assert_eq!(
        plan.writes[0].expected_hash,
        actual["zeta.txt"].as_ref().expect("actual").content_hash()
    );
    assert!(plan.writes[0].expected_exists);

    let candidates = journal
        .undo_candidates(&UndoCandidateQuery {
            session_id: Some("alpha".to_owned()),
            limit: 2,
            seqs: Some(vec![8, 7]),
        })
        .expect("two-step restoration");
    let plan = UndoPlan::build(&candidates, &actual, UndoConflictPolicy::Surface)
        .expect("thread binary history");
    assert_eq!(plan.undo_of, [8, 7]);
    assert_eq!(
        plan.writes[0]
            .content
            .as_ref()
            .expect("original text")
            .as_bytes(),
        b"second\n"
    );
}

#[test]
#[ignore = "requires Bun; executed by the workspace-reference gate"]
fn native_append_is_decoded_by_the_pinned_reference_reader() {
    let corpus = corpus();
    for fixture in corpus["fixtures"].as_array().expect("fixtures") {
        for mode in [JournalMode::Wal, JournalMode::Truncate] {
            let root = tempfile::tempdir().expect("interop root");
            let path = materialize(root.path(), fixture);
            let mut options = JournalDbOptions::new(&path);
            options.journal_mode = mode;
            let mut journal = JournalDb::open(options).expect("native journal");
            let expected_seq = fixture["recent"][0]["op"]["seq"]
                .as_i64()
                .expect("last seq")
                + 1;
            let appended = journal
                .append(&JournalOpInit {
                    tool: "native-conformance".to_owned(),
                    args_hash: "native-args".to_owned(),
                    session_id: "native-session".to_owned(),
                    undo_of: Some(vec![1]),
                    files: vec![JournalFileChange {
                        kind: JournalFileKind::Write,
                        path: "native.bin".to_owned(),
                        before: FileContent::Binary((0..=255).collect()),
                        after: FileContent::Binary(vec![0, 255, 128, 10]),
                    }],
                })
                .expect("append native compressed binary blobs");
            assert_eq!(appended.op.seq, expected_seq);
            let expected = snapshot(&journal.recent(&full_query()).expect("native snapshot"));
            // Exercise a reader beside a live WAL writer and after a TRUNCATE
            // writer closes. The pinned Bun reader on macOS cannot open a closed
            // WAL database after its sidecars have been removed.
            let live_writer = (mode == JournalMode::Wal).then_some(journal);
            let script = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../subsystems/workspace/conformance/journal-reference.ts");
            let result = Command::new("bun")
                .arg(script)
                .arg("--read")
                .arg(path.canonicalize().expect("canonical database path"))
                .output()
                .expect("run pinned reader");
            assert!(
                result.status.success(),
                "{}",
                String::from_utf8_lossy(&result.stderr)
            );
            let observed: Value =
                serde_json::from_slice(&result.stdout).expect("reference snapshot JSON");
            drop(live_writer);
            assert_eq!(
                observed, expected,
                "native zstd and relationships readable by reference"
            );
        }
    }
}
