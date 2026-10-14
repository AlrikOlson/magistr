//! Seeding `workbench.toml` — the allowlist workbench `run` executes from,
//! and the confirm mode that keeps a confirm-gated write off a human's desk.

use std::collections::HashMap;
use std::path::Path;

const HEADER: &str = "# Seeded by `magistr init` from .magistr/project.json (and `workbench-mcp init` \
manifest detection).\n# Amend by hand: this table is workbench `run`'s allowlist and only a human may change it.\n";

/// The `[edit]` block `magistr init` declares when the project has none.
/// Token mode applies verified writes without asking the agent to confirm
/// its own request. The same policy covers text edits, moves, overwriting
/// copies and script batches. Parse verification, staleness checks and the
/// journal remain active; `edit.force_confirm` restores confirmation gates.
const CONFIRM_MODE: &str = "\n# Unattended work applies verified writes without a second confirmation.\n# Set edit.force_confirm to require confirmation for gated operations.\n[edit]\nconfirm_mode = \"token\"\n";

/// The gate commands `magistr init` seeds into `workbench.toml [commands]`.
///
/// The table is a human's: it is written once, when the project has none,
/// and never touched again by the scaffold. When `workbench-mcp` is on PATH
/// its own `init` runs first so manifest-detected commands (`cargo-test`,
/// npm scripts, …) land beside the gate keys; the gate keys are then
/// appended only if that seeding left them out.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WorkbenchSeed {
    /// `(key, command line, provenance)` in emission order.
    pub commands: Vec<(String, String, String)>,
}

impl WorkbenchSeed {
    /// Build from the scaffold template vars (`check_cmd`, `test_cmd`,
    /// `lint_cmd`, `format_cmd`). Blank commands are skipped.
    #[must_use]
    pub fn from_template_vars(vars: &HashMap<String, String>) -> Self {
        let pick = |key: &str, var: &str, provenance: &str| {
            vars.get(var)
                .map(|v| v.trim())
                .filter(|v| !v.is_empty())
                .map(|v| (key.to_owned(), v.to_owned(), provenance.to_owned()))
        };
        let commands = [
            pick("check", "check_cmd", "project.json quality_gate.check"),
            pick("test", "test_cmd", "project.json quality_gate.test"),
            pick("lint", "lint_cmd", "project.json quality_gate.lint"),
            pick("format", "format_cmd", "project.json format.command"),
        ]
        .into_iter()
        .flatten()
        .collect();
        Self { commands }
    }

    /// Build from a loaded project config: the three quality gates, every
    /// custom gate (keyed by its name) and the formatter.
    #[must_use]
    pub fn from_project_config(config: &magistr_core::types::ProjectConfig) -> Self {
        let mut commands = Vec::new();
        let mut push = |key: &str, line: &str, provenance: String| {
            let line = line.trim();
            if !line.is_empty() {
                commands.push((key.to_owned(), line.to_owned(), provenance));
            }
        };
        let gate = &config.quality_gate;
        push(
            "check",
            &gate.check,
            "project.json quality_gate.check".into(),
        );
        push("test", &gate.test, "project.json quality_gate.test".into());
        push("lint", &gate.lint, "project.json quality_gate.lint".into());
        for custom in &config.custom_gates {
            push(
                &custom.name,
                &custom.command,
                format!("project.json custom_gates.{}", custom.name),
            );
        }
        push(
            "format",
            &config.format.command,
            "project.json format.command".into(),
        );
        Self { commands }
    }

    /// Whether there is anything to seed.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.commands.is_empty()
    }

    /// How `body`'s `[commands]` table disagrees with this seed: a gate key
    /// the table does not declare, or one it runs with a different command
    /// line. One human sentence per finding; empty means no drift.
    #[must_use]
    pub fn drift(&self, body: &str) -> Vec<String> {
        let declared = declared_commands(body);
        self.commands
            .iter()
            .filter_map(|(key, line, provenance)| {
                match declared.iter().find(|(k, _)| k == key) {
                    None => Some(format!(
                        "workbench.toml [commands] has no `{key}` — {provenance} runs `{line}`"
                    )),
                    Some((_, actual)) if actual != line => Some(format!(
                        "`{key}` drifted: workbench.toml runs `{actual}`, {provenance} runs `{line}`"
                    )),
                    Some(_) => None,
                }
            })
            .collect()
    }

    /// One `"key" = "line" # provenance` row.
    fn row(key: &str, line: &str, provenance: &str) -> String {
        format!(
            "\"{}\" = \"{}\" # {provenance}\n",
            toml_escape(key),
            toml_escape(line)
        )
    }

    /// A complete `workbench.toml` with only the seeded table.
    #[must_use]
    pub fn render(&self) -> String {
        let mut out = String::from(HEADER);
        out.push_str("[commands]\n");
        for (key, line, provenance) in &self.commands {
            out.push_str(&Self::row(key, line, provenance));
        }
        out
    }

    /// Fold the seed into an existing file body: rows whose key is already
    /// declared are skipped; the rest are appended to the `[commands]`
    /// table when it is the file's last table, or a fresh table is added
    /// when the file has none. `None` means nothing needed writing.
    #[must_use]
    pub fn merge_into(&self, existing: &str) -> Option<String> {
        let declared = declared_keys(existing);
        let missing: Vec<_> = self
            .commands
            .iter()
            .filter(|(key, _, _)| !declared.contains(&key.as_str()))
            .collect();
        if missing.is_empty() {
            return None;
        }
        let has_table = existing.lines().any(|l| l.trim() == "[commands]");
        let last_table_is_commands = existing
            .lines()
            .map(str::trim)
            .rfind(|l| l.starts_with('['))
            .is_some_and(|l| l == "[commands]");
        if has_table && !last_table_is_commands {
            // Appending here would land the rows in another table; leave the
            // human's file alone rather than corrupt it.
            return None;
        }
        let mut out = existing.to_owned();
        if !out.is_empty() && !out.ends_with('\n') {
            out.push('\n');
        }
        if !has_table {
            if out.is_empty() {
                out.push_str(HEADER);
            } else {
                out.push('\n');
            }
            out.push_str("[commands]\n");
        }
        for (key, line, provenance) in missing {
            out.push_str(&Self::row(key, line, provenance));
        }
        Some(out)
    }

    /// The `<root>/workbench.toml` body seeding would write, or `None` when
    /// the file needs nothing: the current content read while planning and
    /// the body to write. A file that already declares `[commands]` keeps
    /// its table; otherwise the native lockfile/manifest detection
    /// (`magistr workspace init`) seeds the table first and the gate keys
    /// are folded in. The confirm mode is a separate question asked of every
    /// project, appended last so a later `[commands]` merge never lands in
    /// the `[edit]` table.
    #[must_use]
    pub fn plan(&self, root: &Path) -> (Option<String>, Option<String>) {
        let path = root.join("workbench.toml");
        let before = std::fs::read_to_string(&path).ok();
        let mut body = before.clone().unwrap_or_default();
        // The commands table is a human's and is seeded once.
        if !declares_commands(&body) {
            let detected = magistr_workspace::config::DetectedCommand::detect(root);
            if !detected.is_empty() {
                let block = magistr_workspace::config::DetectedCommand::render_table(&detected);
                body = if body.is_empty() {
                    block
                } else {
                    format!("{}\n\n{block}", body.trim_end_matches('\n'))
                };
            }
            if let Some(merged) = self.merge_into(&body) {
                body = merged;
            }
        }
        if let Some(with_mode) = with_confirm_mode(&body) {
            body = with_mode;
        }
        // A project with nothing to seed and no file gets no file.
        let planned = (before.as_deref().unwrap_or_default() != body.as_str()).then_some(body);
        (before, planned)
    }

    /// The confirm-mode declaration as a planned change: the current
    /// `workbench.toml` (none when there is no file) and the body declaring
    /// the mode when the file does not already.
    ///
    /// Asked on *every* session rather than once at init, by
    /// `scaffold::repair`: a project scaffolded before this landed has a
    /// perfectly good `[commands]` table and no idea that a confirm-gated
    /// write is about to ask a human who is not there.
    #[must_use]
    pub fn plan_confirm_mode(root: &Path) -> (Option<String>, Option<String>) {
        let current = std::fs::read_to_string(root.join("workbench.toml")).ok();
        let planned = with_confirm_mode(current.as_deref().unwrap_or_default());
        (current, planned)
    }
}

/// `body` with the `[edit]` block appended, or `None` when it already says
/// what the confirm mode is — a human who chose `elicit` chose it, and a
/// file magistr has already seeded must not grow a second block on every
/// `magistr init`.
///
/// An empty file is left empty: a project with no `workbench.toml` at all
/// has no workbench workspace to configure, and writing one here would
/// create a file the commands seed decided not to.
fn with_confirm_mode(body: &str) -> Option<String> {
    if body.trim().is_empty() || declares_confirm_mode(body) {
        return None;
    }
    let mut out = body.to_owned();
    if !out.ends_with('\n') {
        out.push('\n');
    }
    out.push_str(CONFIRM_MODE);
    Some(out)
}

/// Whether `body` already declares `edit.confirm_mode`, written either as a
/// key under `[edit]` or as a top-level dotted key.
fn declares_confirm_mode(body: &str) -> bool {
    let mut in_edit = false;
    for line in body.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_edit = trimmed == "[edit]";
            continue;
        }
        let key = trimmed
            .split('=')
            .next()
            .map(str::trim)
            .map(|k| k.trim_matches('"'))
            .unwrap_or_default();
        if key == "edit.confirm_mode" || (in_edit && key == "confirm_mode") {
            return true;
        }
    }
    false
}

fn declares_commands(body: &str) -> bool {
    body.lines().any(|l| l.trim() == "[commands]")
}

/// Keys declared in the `[commands]` table of `body` (bare or quoted).
fn declared_keys(body: &str) -> Vec<&str> {
    let mut in_commands = false;
    let mut keys = Vec::new();
    for line in body.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_commands = trimmed == "[commands]";
            continue;
        }
        if !in_commands || trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if let Some((key, _)) = trimmed.split_once('=') {
            keys.push(key.trim().trim_matches('"'));
        }
    }
    keys
}

/// `(key, command line)` pairs declared in the `[commands]` table of `body`,
/// with quotes and trailing comments removed.
fn declared_commands(body: &str) -> Vec<(String, String)> {
    let mut in_commands = false;
    let mut rows = Vec::new();
    for line in body.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_commands = trimmed == "[commands]";
            continue;
        }
        if !in_commands || trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if let Some((key, value)) = trimmed.split_once('=') {
            rows.push((
                key.trim().trim_matches('"').to_owned(),
                toml_value(value.trim()),
            ));
        }
    }
    rows
}

/// The string a TOML value spells: a quoted string is unescaped, a bare
/// value is taken up to the first `#`.
fn toml_value(raw: &str) -> String {
    let Some(rest) = raw.strip_prefix('"') else {
        return raw.split('#').next().unwrap_or_default().trim().to_owned();
    };
    let mut out = String::new();
    let mut chars = rest.chars();
    while let Some(c) = chars.next() {
        match c {
            '\\' => {
                if let Some(escaped) = chars.next() {
                    out.push(escaped);
                }
            }
            '"' => break,
            other => out.push(other),
        }
    }
    out
}

fn toml_escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(test)]
#[path = "workbench_toml.tests.rs"]
mod tests;
