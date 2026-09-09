//! Seeding `workbench.toml` — the allowlist workbench `run` executes from,
//! and the confirm mode that keeps a confirm-gated write off a human's desk.

use std::collections::HashMap;
use std::path::Path;
use std::process::Command;

const HEADER: &str = "# Seeded by `magistr init` from .magistr/project.json (and `workbench-mcp init` \
manifest detection).\n# Amend by hand: this table is workbench `run`'s allowlist and only a human may change it.\n";

/// The `[edit]` block `magistr init` declares when the project has none.
///
/// workbench's default confirm mode is `elicit`: a confirm-gated write —
/// more than one file, or more than 200 changed lines, which is an ordinary
/// refactor — asks the *client* to put a dialog in front of a human. Under
/// `magistr next-loop` there is nobody there to answer it, so a routine edit
/// stops an autonomous run for a question no one hears; that is exactly the
/// unintended human intervention this seed exists to prevent.
///
/// `token` routes the same gate through workbench's two-call
/// `CONFIRM_REQUIRED` + `confirmToken` flow: the **agent** confirms and the
/// aggregated diff still rides the envelope, so the review is not lost — it
/// is simply not asked of a person. It is deliberately *not* a
/// security-lattice key in workbench (org policy must not be able to force
/// dialogs onto a delegated session), which is what makes a workspace
/// entitled to set it. `edit.force_confirm`, the tighten-only knob that puts
/// a gate back, is untouched.
const CONFIRM_MODE: &str = "\n# magistr runs unattended (`magistr next-loop`), so a confirm-gated write\n# takes workbench's token flow — the agent confirms against the diff in the\n# envelope — rather than an elicitation dialog nobody is there to answer.\n[edit]\nconfirm_mode = \"token\"\n";

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

    /// Seed `<root>/workbench.toml`. A file that already declares
    /// `[commands]` is never modified (returns `Ok(false)`). Otherwise, when
    /// `run_init` is set, `workbench-mcp init <root>` runs first; then the
    /// gate keys are folded in. Returns whether the file was written.
    pub fn apply(&self, root: &Path, run_init: bool) -> std::io::Result<bool> {
        let path = root.join("workbench.toml");
        let before = std::fs::read_to_string(&path).unwrap_or_default();
        let mut wrote = false;
        // The commands table is a human's and is seeded once; the confirm
        // mode below is a *separate* question and is asked of every project,
        // because the common case is exactly a file that already declares
        // `[commands]` and has never heard of `edit.confirm_mode`.
        if !declares_commands(&before) {
            if run_init {
                // Best effort: a missing or failing binary just means no
                // manifest-detected rows, which the gate keys below cover.
                let _ = Command::new("workbench-mcp")
                    .arg("init")
                    .arg(root)
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .status();
            }
            let current = std::fs::read_to_string(&path).unwrap_or_default();
            if let Some(body) = self.merge_into(&current) {
                std::fs::write(&path, body)?;
                wrote = true;
            }
        }
        // Appended last, so a later `[commands]` merge is never landed in
        // this table — `merge_into` refuses to append when the file's last
        // table is not `[commands]`, which is the same protection.
        Ok(Self::ensure_confirm_mode(root)? || wrote)
    }

    /// Declare the unattended confirm mode on `root`'s `workbench.toml`,
    /// answering whether anything was written.
    ///
    /// Split out of [`WorkbenchSeed::apply`] because it is the half that
    /// wants asking on *every* session rather than once at init: a project
    /// scaffolded before this landed has a perfectly good `[commands]` table
    /// and no idea that a confirm-gated write is about to ask a human who is
    /// not there. `scaffold::repair` is the caller that closes that.
    pub fn ensure_confirm_mode(root: &Path) -> std::io::Result<bool> {
        let path = root.join("workbench.toml");
        let current = std::fs::read_to_string(&path).unwrap_or_default();
        match with_confirm_mode(&current) {
            Some(body) => {
                std::fs::write(&path, body)?;
                Ok(true)
            }
            None => Ok(false),
        }
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
