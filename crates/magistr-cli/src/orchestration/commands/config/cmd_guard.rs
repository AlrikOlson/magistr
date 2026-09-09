//! `magistr guard <stop|commit|write> [DIR]` — the Claude Code hook guard
//! behind the scaffolded `.claude/hooks/magistr-guard.sh`.
//!
//! Reads the hook's JSON on stdin (only when stdin is not a terminal), the
//! ship/roadmap state through the in-process think-and-ship engines, the
//! working tree and the project's frozen-tree declaration
//! (`.magistr/legacy-gui-ratchet.json`), then exits 2 with the reason on
//! stderr to block the event. A project without the native roadmap backend
//! is never blocked.

use std::io::{IsTerminal, Read};
use std::path::{Component, Path, PathBuf};

use color_eyre::eyre;

use super::guard_decision::{CheckSummary, FrozenTree, GuardInput, GuardVerdict, evaluate};
use crate::cli::GuardEvent;

/// Exit code Claude Code reads as "block this event; stderr carries the reason".
const BLOCK_EXIT_CODE: i32 = 2;

/// The frozen-tree declaration, shared with `scripts/legacy-gui-ratchet.sh`
/// (the `legacy-gui` quality gate). The name travels with whichever tree is
/// currently frozen — it was `legacy-tui-ratchet.json` while `src/tui` was —
/// because a project freezes one tree at a time and the file's name is how
/// the next reader learns which.
const RATCHET_FILE: &str = ".magistr/legacy-gui-ratchet.json";

pub fn cmd_guard(event: GuardEvent, dir: &Path) -> eyre::Result<()> {
    let given_dir = dir.to_path_buf();
    let dir = dir.canonicalize().unwrap_or_else(|_| dir.to_path_buf());
    let hook = read_hook_input();
    let mut input = hook_fields(&hook);
    input.write_path = input
        .write_path
        .as_deref()
        .and_then(|raw| relative_write_path(&[&dir, &given_dir], raw));
    input.frozen = read_frozen(&dir);
    input.git_dirty = git_dirty(&dir);

    match crate::roadmap::open_native_engines(&dir) {
        Some(engines) => {
            apply_engine_state(
                &mut input,
                &engines.ship_status(),
                &engines.roadmap_status(),
            );
        }
        None => {
            eprintln!(
                "magistr guard: {} does not use the think-and-ship roadmap backend; allowing",
                dir.display()
            );
            return Ok(());
        }
    }

    match evaluate(event, &input) {
        GuardVerdict::Allow(note) => {
            if let Some(note) = note {
                eprintln!("magistr guard: {note}");
            }
            Ok(())
        }
        GuardVerdict::Block(reason) => {
            eprintln!("magistr guard ({}): {reason}", event.hook_name());
            std::process::exit(BLOCK_EXIT_CODE)
        }
    }
}

/// The hook JSON Claude Code pipes in, or `Null` when stdin is a terminal or
/// not JSON (a manual invocation).
fn read_hook_input() -> serde_json::Value {
    let mut stdin = std::io::stdin();
    if stdin.is_terminal() {
        return serde_json::Value::Null;
    }
    let mut raw = String::new();
    if stdin.read_to_string(&mut raw).is_err() {
        return serde_json::Value::Null;
    }
    serde_json::from_str(&raw).unwrap_or(serde_json::Value::Null)
}

/// The fields the guard reads from the hook JSON: `stop_hook_active` (Stop),
/// `tool_input.command` (PreToolUse on Bash) and `tool_input.file_path` /
/// `notebook_path` (PreToolUse on the file-editing tools, still as given —
/// `cmd_guard` makes it project-relative). Missing fields are inert.
pub(crate) fn hook_fields(hook: &serde_json::Value) -> GuardInput {
    let tool_input = &hook["tool_input"];
    GuardInput {
        stop_hook_active: hook["stop_hook_active"].as_bool().unwrap_or(false),
        command: tool_input["command"].as_str().map(str::to_owned),
        write_path: tool_input["file_path"]
            .as_str()
            .or_else(|| tool_input["notebook_path"].as_str())
            .map(str::to_owned),
        ..GuardInput::default()
    }
}

/// `raw` (absolute, or relative to the project) as a path relative to one of
/// `roots` — the canonical project dir and the dir as the hook named it, so
/// a symlinked checkout (macOS `/tmp` → `/private/tmp`) resolves either way.
/// `None` when the file lies outside the project.
pub(crate) fn relative_write_path(roots: &[&Path], raw: &str) -> Option<String> {
    let given = Path::new(raw);
    let mut candidates: Vec<PathBuf> = Vec::new();
    for root in roots {
        let abs = normalize(&if given.is_absolute() {
            given.to_path_buf()
        } else {
            root.join(given)
        });
        // The file itself may not exist yet (Write); its parent usually does.
        if let (Some(parent), Some(name)) = (abs.parent(), abs.file_name())
            && let Ok(parent) = parent.canonicalize()
        {
            candidates.push(parent.join(name));
        }
        candidates.push(abs);
    }
    candidates.iter().find_map(|abs| {
        roots
            .iter()
            .find_map(|root| abs.strip_prefix(root).ok())
            // Spelled with `/` whatever the host separator: the frozen
            // tree's prefix and the hook's paths are written that way.
            .map(|rel| {
                rel.components()
                    .map(|c| c.as_os_str().to_string_lossy())
                    .collect::<Vec<_>>()
                    .join("/")
            })
    })
}

/// Lexical `.`/`..` resolution, no filesystem access.
fn normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other),
        }
    }
    out
}

/// The project's frozen tree, from `.magistr/legacy-gui-ratchet.json`
/// (`{"path": "crates/magistr-gui", "files": N, "loc": M, "unlock": [...],
/// "replacement": "…"}`). No file, or an unreadable one, means nothing is
/// frozen — the guard never blocks on its own inability to read.
///
/// A declaration that lists no `unlock` words falls back to `delete` alone:
/// the one verb every freeze admits, whatever the tree is called.
pub(crate) fn read_frozen(dir: &Path) -> Option<FrozenTree> {
    let raw = std::fs::read_to_string(dir.join(RATCHET_FILE)).ok()?;
    let json: serde_json::Value = serde_json::from_str(&raw).ok()?;
    let path = json["path"].as_str()?.trim_matches('/').to_owned();
    let unlock = json["unlock"]
        .as_array()
        .map(|words| {
            words
                .iter()
                .filter_map(serde_json::Value::as_str)
                .map(str::to_owned)
                .collect::<Vec<_>>()
        })
        .filter(|words| !words.is_empty())
        .unwrap_or_else(|| vec!["delete".to_owned()]);
    let replacement = json["replacement"]
        .as_str()
        .map(str::trim)
        .filter(|what| !what.is_empty())
        .map(str::to_owned);
    Some(FrozenTree {
        path,
        unlock,
        replacement,
    })
}

/// Fold the engines' `ship_status` and `roadmap_status` documents — the same
/// JSON the agent sees over MCP — into the guard input.
pub(crate) fn apply_engine_state(
    input: &mut GuardInput,
    ship: &serde_json::Value,
    roadmap: &serde_json::Value,
) {
    input.objective_status = ship["objective"]["status"]
        .as_str()
        .map(|s| s.to_ascii_lowercase());
    input.objective_scope = ship["objective"]["scope"].as_str().map(str::to_owned);
    input.tasks_total = ship["tasks"]["total"]
        .as_u64()
        .and_then(|n| usize::try_from(n).ok())
        .unwrap_or(0);
    input.checks = ship["checks"]
        .as_array()
        .map(|checks| {
            checks
                .iter()
                .map(|c| CheckSummary {
                    name: c["name"].as_str().unwrap_or_default().to_owned(),
                    required: c["required"].as_bool().unwrap_or(true),
                    passed: c["passed"].as_bool().unwrap_or(false),
                    verified: c["verified"].as_bool().unwrap_or(false),
                })
                .collect()
        })
        .unwrap_or_default();
    input.chunk_in_progress = roadmap["chunks"]
        .as_array()
        .is_some_and(|chunks| chunks.iter().any(|c| c["status"] == "in_progress"));
}

/// `git status --porcelain` printed anything. A missing git or a directory
/// outside a repository counts as clean — the guard never blocks on its own
/// inability to look.
fn git_dirty(dir: &Path) -> bool {
    std::process::Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(dir)
        .output()
        .ok()
        .filter(|out| out.status.success())
        .is_some_and(|out| !out.stdout.iter().all(u8::is_ascii_whitespace))
}

#[cfg(test)]
#[path = "cmd_guard.tests.rs"]
mod tests;
