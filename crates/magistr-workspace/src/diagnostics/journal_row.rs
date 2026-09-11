use serde::Serialize;

use crate::{
    file_content::FileContent,
    journal::{JournalFileKind, JournalLogEntry},
};

use super::JournalViewOptions;

/// One journaled file change, as a reader sees it — the retained journal
/// view: one row per file an op touched, every row carrying its op's `seq`
/// (what `undo ops:[…]` names); an op that changed no file still gets a thin
/// row, because history that silently omits an entry is worse.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JournalRow {
    pub kind: &'static str,
    pub seq: i64,
    /// Milliseconds since the Unix epoch, journal-assigned.
    pub at: i64,
    /// The tool that originated the op: edit, files, script, write, undo.
    pub tool: String,
    /// The session that owns the op; "" is the anonymous identity.
    pub session_id: String,
    /// Whether `undo ops:[seq]` would still accept this op.
    pub undoable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub undone_by: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub undo_of: Option<Vec<i64>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub change: Option<&'static str>,
    pub added: usize,
    pub removed: usize,
    /// Text pipeline admission — absent for opaque binary content. A
    /// journaled op passed the parse-verify choke point before it landed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parse_ok: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub binary: Option<bool>,
    /// The journal blob fell to the size cap: history, not a rewind.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub evicted: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diff: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diff_truncated: Option<bool>,
}

impl JournalRow {
    /// Render entries (newest first, as the reader returns them) as rows.
    /// Pure: same entries and options → same rows.
    #[must_use]
    pub fn render(entries: &[JournalLogEntry], options: &JournalViewOptions) -> Vec<Self> {
        let mut rows = Vec::new();
        for entry in entries {
            let op = &entry.op;
            let common = Self {
                kind: "op",
                seq: op.seq,
                at: op.at,
                tool: op.tool.clone(),
                session_id: op.session_id.clone(),
                undoable: entry.undoable,
                undone_by: entry.undone_by,
                undo_of: op.undo_of.clone(),
                path: None,
                change: None,
                added: 0,
                removed: 0,
                parse_ok: None,
                binary: None,
                evicted: None,
                diff: None,
                diff_truncated: None,
            };
            if op.files.is_empty() {
                rows.push(Self {
                    parse_ok: Some(true),
                    ..common
                });
                continue;
            }
            for file in &op.files {
                let change = Some(kind_name(file.kind));
                // Empty bytes are a real stored value (a creation's before, a
                // deletion's after); `None` means the blob was evicted.
                let evicted = (file.kind != JournalFileKind::Delete && file.after.is_none())
                    || (file.kind != JournalFileKind::Create && file.before.is_none());
                if evicted {
                    rows.push(Self {
                        path: Some(file.path.clone()),
                        change,
                        evicted: Some(true),
                        ..common.clone()
                    });
                    continue;
                }
                let before = (file.kind != JournalFileKind::Create)
                    .then_some(file.before.as_ref())
                    .flatten();
                let after = (file.kind != JournalFileKind::Delete)
                    .then_some(file.after.as_ref())
                    .flatten();
                let body = crate::commands::unified_diff(&file.path, before, after);
                let (added, removed) = changed_lines(&body);
                let binary = matches!(file.before, Some(FileContent::Binary(_)))
                    || matches!(file.after, Some(FileContent::Binary(_)));
                let cut = body.encode_utf16().count() > options.diff_char_cap;
                rows.push(Self {
                    path: Some(file.path.clone()),
                    change,
                    added,
                    removed,
                    parse_ok: (!binary).then_some(true),
                    binary: binary.then_some(true),
                    diff: options.diff.then(|| {
                        if cut {
                            take_utf16(&body, options.diff_char_cap)
                        } else {
                            body.clone()
                        }
                    }),
                    diff_truncated: (options.diff && cut).then_some(true),
                    ..common.clone()
                });
            }
        }
        rows
    }

    /// The retained human rendering of the `journal` verb: one header line
    /// per op, one line per file, diffs last; an empty log says so.
    #[must_use]
    pub fn render_text(rows: &[Self]) -> String {
        let mut out: Vec<String> = Vec::new();
        let mut current_seq = None;
        for row in rows {
            if current_seq != Some(row.seq) {
                current_seq = Some(row.seq);
                out.push(header_line(row));
            }
            let Some(path) = &row.path else {
                out.push("    (no file changed)".into());
                continue;
            };
            let mark = match row.change {
                Some("create") => "A",
                Some("delete") => "D",
                _ => "M",
            };
            let evicted = if row.evicted == Some(true) {
                "  [blob evicted — history only]"
            } else {
                ""
            };
            out.push(format!(
                "    {mark} {path}  +{} -{}{evicted}",
                row.added, row.removed
            ));
            if let Some(diff) = row.diff.as_deref().filter(|diff| !diff.is_empty()) {
                out.extend(diff.split('\n').map(|line| format!("      {line}")));
                if row.diff_truncated == Some(true) {
                    out.push("      … diff cut at the per-row cap".into());
                }
            }
        }
        if out.is_empty() {
            "(the journal is empty)".into()
        } else {
            out.join("\n")
        }
    }
}

/// `#<seq>  <ISO stamp>  <tool>  [session=<id>]  undoable|undone[ by #N]  [(undo of …)]`.
fn header_line(row: &JournalRow) -> String {
    let session = if row.session_id.is_empty() {
        String::new()
    } else {
        format!("  session={}", row.session_id)
    };
    let state = if row.undoable {
        "undoable".to_owned()
    } else {
        match row.undone_by {
            Some(by) => format!("undone by #{by}"),
            None => "undone".to_owned(),
        }
    };
    let undo_of = row
        .undo_of
        .as_ref()
        .map(|ops| {
            let ops: Vec<String> = ops.iter().map(ToString::to_string).collect();
            format!("  (undo of {})", ops.join(", "))
        })
        .unwrap_or_default();
    format!(
        "#{}  {}  {}{session}  {state}{undo_of}",
        row.seq,
        super::utc_timestamp(u64::try_from(row.at).unwrap_or(0)),
        row.tool
    )
}

fn kind_name(kind: JournalFileKind) -> &'static str {
    match kind {
        JournalFileKind::Create => "create",
        JournalFileKind::Write => "write",
        JournalFileKind::Delete => "delete",
    }
}

/// Lines a diff would apply: its `+`/`-` rows (file headers excluded).
fn changed_lines(diff: &str) -> (usize, usize) {
    let mut added = 0;
    let mut removed = 0;
    for line in diff.split('\n') {
        if line.starts_with("+++") || line.starts_with("---") {
            continue;
        }
        if line.starts_with('+') {
            added += 1;
        } else if line.starts_with('-') {
            removed += 1;
        }
    }
    (added, removed)
}

/// The retained cut counts JavaScript string units; keep whole characters.
fn take_utf16(text: &str, units: usize) -> String {
    let mut used = 0;
    text.chars()
        .take_while(|character| {
            used += character.len_utf16();
            used <= units
        })
        .collect()
}

#[cfg(test)]
#[path = "journal_row.tests.rs"]
mod tests;
