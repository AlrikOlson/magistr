//! Post-session auto-formatting via the orchestration layer.
//!
//! `Formatter` reads the format command and extensions from `FormatConfig`,
//! runs the formatter on all changed files after an agent session completes.
//!
//! This replaces the `auto-format.sh` PostToolUse hook with a single
//! post-session formatting pass — more efficient and backend-agnostic.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::types::FormatConfig;
use crate::workflow::format_result::FormatResult;

/// Post-session file formatter.
///
/// Reads format command and extensions from `FormatConfig`, runs the formatter
/// on changed files detected via `git diff --name-only`.
pub struct Formatter {
    config: FormatConfig,
    project_dir: PathBuf,
    /// When true, formatting is skipped (`MAGISTR_NO_AUTOFORMAT=1`).
    no_autoformat: bool,
}

impl Formatter {
    /// Create a new formatter for the given project.
    pub fn new(config: FormatConfig, project_dir: impl Into<PathBuf>) -> Self {
        Self {
            config,
            project_dir: project_dir.into(),
            no_autoformat: false,
        }
    }

    /// Create a new formatter with `MagistrEnv`-driven config.
    pub fn with_env(
        config: FormatConfig,
        project_dir: impl Into<PathBuf>,
        env: &crate::env::MagistrEnv,
    ) -> Self {
        Self {
            config,
            project_dir: project_dir.into(),
            no_autoformat: env.no_autoformat,
        }
    }

    /// Run the formatter on all changed files matching the configured extensions.
    ///
    /// Returns a `FormatResult` with details about what was formatted.
    /// Respects `no_autoformat` flag to skip formatting entirely.
    pub fn run(&self) -> FormatResult {
        if self.no_autoformat {
            return FormatResult::skipped("MAGISTR_NO_AUTOFORMAT=1");
        }

        // Check if format command is configured
        if self.config.command.is_empty() || self.config.command == "none" {
            return FormatResult::skipped("no format command configured");
        }

        // Get changed files
        let changed = self.changed_files();
        if changed.is_empty() {
            return FormatResult::skipped("no changed files");
        }

        // Filter by extension
        let matching: Vec<PathBuf> = changed
            .into_iter()
            .filter(|f| self.matches_extension(f))
            .collect();

        if matching.is_empty() {
            return FormatResult::skipped("no changed files match configured extensions");
        }

        // Run formatter on each file
        let mut result = FormatResult {
            files_formatted: Vec::new(),
            files_failed: Vec::new(),
            skipped: false,
            skip_reason: None,
        };

        for file in matching {
            match self.format_file(&file) {
                Ok(()) => result.files_formatted.push(file),
                Err(msg) => result.files_failed.push((file, msg)),
            }
        }

        result
    }

    // ── Private helpers ─────────────────────────────────────────────────

    /// Get changed files from git (both staged and unstaged).
    fn changed_files(&self) -> Vec<PathBuf> {
        // Unstaged changes
        let mut files = self.git_diff_names(&[]);
        // Staged changes
        let staged = self.git_diff_names(&["--cached"]);
        for f in staged {
            if !files.contains(&f) {
                files.push(f);
            }
        }
        // Untracked files
        let untracked = self.untracked_files();
        for f in untracked {
            if !files.contains(&f) {
                files.push(f);
            }
        }
        files
    }

    fn git_diff_names(&self, extra_args: &[&str]) -> Vec<PathBuf> {
        let mut args = vec!["diff", "--name-only"];
        args.extend(extra_args);

        let output = Command::new("git")
            .args(&args)
            .current_dir(&self.project_dir)
            .output();

        match output {
            Ok(out) if out.status.success() => String::from_utf8_lossy(&out.stdout)
                .lines()
                .filter(|l| !l.is_empty())
                .map(PathBuf::from)
                .collect(),
            _ => Vec::new(),
        }
    }

    fn untracked_files(&self) -> Vec<PathBuf> {
        let output = Command::new("git")
            .args(["ls-files", "--others", "--exclude-standard"])
            .current_dir(&self.project_dir)
            .output();

        match output {
            Ok(out) if out.status.success() => String::from_utf8_lossy(&out.stdout)
                .lines()
                .filter(|l| !l.is_empty())
                .map(PathBuf::from)
                .collect(),
            _ => Vec::new(),
        }
    }

    fn matches_extension(&self, path: &Path) -> bool {
        if self.config.extensions.is_empty() {
            return true; // No filter = format everything
        }

        path.extension()
            .and_then(|e| e.to_str())
            .is_some_and(|ext| self.config.extensions.iter().any(|e| e == ext))
    }

    fn format_file(&self, file: &Path) -> Result<(), String> {
        let abs_path = self.project_dir.join(file);
        if !abs_path.is_file() {
            return Err("file does not exist".into());
        }

        // Replace `{file}` placeholder in command, or append file path
        // The path is one shell word: quoted unless the template already
        // quotes the placeholder, so a space, a `$` or a Windows backslash
        // in it reaches the formatter intact.
        let file = abs_path.to_string_lossy();
        let template = &self.config.command;
        let cmd_str = if template.contains("'{file}'") || template.contains("\"{file}\"") {
            template.replace("{file}", &file)
        } else if template.contains("{file}") {
            template.replace("{file}", &shell_word(&file))
        } else {
            format!("{template} {}", shell_word(&file))
        };

        let output = Command::new("sh")
            .args(["-c", &cmd_str])
            .current_dir(&self.project_dir)
            .output()
            .map_err(|e| format!("failed to execute formatter: {e}"))?;

        if output.status.success() {
            Ok(())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(format!(
                "formatter exited with {}: {}",
                output.status,
                stderr.trim()
            ))
        }
    }
}

/// `word` as one single-quoted shell word.
fn shell_word(word: &str) -> String {
    format!("'{}'", word.replace('\'', "'\\''"))
}

#[cfg(test)]
#[path = "formatter.tests.rs"]
mod tests;
