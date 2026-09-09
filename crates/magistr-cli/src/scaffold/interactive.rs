//! Interactive file-by-file scaffold approval.
//!
//! When `magistr init --dry-run` runs on a TTY, this module provides an
//! interactive loop that walks through each file in the scaffold preview
//! and lets the user decide: apply, skip, view, apply-all, or quit.

use std::io::{BufRead, Write};
use std::path::Path;

use super::{FileAction, FilePreview, ScaffoldPreview};
use crate::error::Result;
use crate::utils::format as fmt;

/// User action for a single file in the approval loop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ApprovalAction {
    /// Apply this file.
    Apply,
    /// Skip this file (don't write it).
    Skip,
    /// View file details (existing, new, diff), then re-prompt.
    View,
    /// Apply this file and all remaining files.
    ApplyAll,
    /// Quit — skip all remaining files.
    Quit,
}

/// Result of the interactive approval process.
#[derive(Debug, Clone)]
pub(crate) struct ApprovalResult {
    /// Indices into the preview's files vec that were approved.
    pub approved_indices: Vec<usize>,
    /// Whether the user quit early (for callers that need to distinguish
    /// "user reviewed everything" from "user bailed out").
    pub quit_early: bool,
}

impl ApprovalResult {
    /// Get references to the approved file previews.
    pub fn approved_files<'a>(&self, preview: &'a ScaffoldPreview) -> Vec<&'a FilePreview> {
        self.approved_indices
            .iter()
            .map(|&i| &preview.files[i])
            .collect()
    }
}

/// Parse a single character into an approval action.
///
/// Accepts: `a` (apply), `s` (skip), `v` (view), `A` (apply all), `q` (quit).
/// Returns `None` for unrecognized input.
pub(crate) fn parse_action(input: &str) -> Option<ApprovalAction> {
    match input.trim() {
        "a" => Some(ApprovalAction::Apply),
        "s" => Some(ApprovalAction::Skip),
        "v" => Some(ApprovalAction::View),
        "A" => Some(ApprovalAction::ApplyAll),
        "q" => Some(ApprovalAction::Quit),
        // Also accept full words
        "apply" => Some(ApprovalAction::Apply),
        "skip" => Some(ApprovalAction::Skip),
        "view" => Some(ApprovalAction::View),
        "quit" => Some(ApprovalAction::Quit),
        _ => None,
    }
}

/// Format the action badge for a file (colored).
pub(crate) fn format_action_badge(action: &FileAction) -> String {
    match action {
        FileAction::Create => fmt::green("[CREATE]"),
        FileAction::Overwrite => fmt::yellow("[OVERWRITE]"),
        FileAction::Unchanged => fmt::dim("[UNCHANGED]"),
        FileAction::Preserved => fmt::cyan("[PRESERVED]"),
    }
}

/// Format the file header line for the approval prompt.
pub(crate) fn format_file_header(index: usize, total: usize, file: &FilePreview) -> String {
    format!(
        "{} {} {}",
        fmt::bold(&format!("[{}/{}]", index + 1, total)),
        format_action_badge(&file.action),
        file.path,
    )
}

/// Print detailed view of a file (existing content, new content, diff).
pub(crate) fn print_file_detail<W: Write>(w: &mut W, file: &FilePreview) -> std::io::Result<()> {
    // Show existing content if present
    if let Some(ref existing) = file.existing_content {
        writeln!(w, "\n{}", fmt::bold("── Existing content ──"))?;
        let lines: Vec<&str> = existing.lines().collect();
        let preview_lines = if lines.len() > 30 {
            writeln!(
                w,
                "{}",
                fmt::dim(&format!("(showing first 30 of {} lines)", lines.len()))
            )?;
            &lines[..30]
        } else {
            &lines
        };
        for line in preview_lines {
            writeln!(w, "{}", fmt::dim(&format!("  {line}")))?;
        }
    }

    // Show new content
    writeln!(w, "\n{}", fmt::bold("── New content ──"))?;
    let lines: Vec<&str> = file.new_content.lines().collect();
    let preview_lines = if lines.len() > 30 {
        writeln!(
            w,
            "{}",
            fmt::dim(&format!("(showing first 30 of {} lines)", lines.len()))
        )?;
        &lines[..30]
    } else {
        &lines
    };
    for line in preview_lines {
        writeln!(w, "  {line}")?;
    }

    // Show diff if available
    if let Some(ref diff) = file.diff {
        writeln!(w, "\n{}", fmt::bold("── Diff ──"))?;
        for line in diff.lines() {
            if line.starts_with('+') && !line.starts_with("+++") {
                writeln!(w, "{}", fmt::green(line))?;
            } else if line.starts_with('-') && !line.starts_with("---") {
                writeln!(w, "{}", fmt::red(line))?;
            } else if line.starts_with("@@") {
                writeln!(w, "{}", fmt::cyan(line))?;
            } else {
                writeln!(w, "{line}")?;
            }
        }
    }

    writeln!(w)?;
    Ok(())
}

/// Run the interactive approval loop.
///
/// Reads from `reader` and writes to `writer` to support testing.
/// In production, these are stdin and stderr.
///
/// Only actionable files (Create, Overwrite) are prompted. Unchanged
/// and Preserved files are shown as informational and auto-skipped.
pub(crate) fn interactive_approve<R: BufRead, W: Write>(
    preview: &ScaffoldPreview,
    reader: &mut R,
    writer: &mut W,
) -> Result<ApprovalResult> {
    let total = preview.files.len();
    let mut approved = Vec::new();
    let mut quit_early = false;

    writeln!(
        writer,
        "{} ({total} files)\n",
        fmt::bold("📋 Interactive scaffold approval")
    )
    .ok();
    writeln!(
        writer,
        "For each file: {}pply  {}kip  {}iew  {}pply-all  {}uit\n",
        fmt::bold("[a]"),
        fmt::bold("[s]"),
        fmt::bold("[v]"),
        fmt::bold("[A]"),
        fmt::bold("[q]"),
    )
    .ok();

    for (i, file) in preview.files.iter().enumerate() {
        // Auto-skip non-actionable files with a status line
        match file.action {
            FileAction::Unchanged => {
                writeln!(writer, "{}", format_file_header(i, total, file)).ok();
                writeln!(writer, "  {}", fmt::dim("→ skipped (no changes)")).ok();
                continue;
            }
            FileAction::Preserved => {
                writeln!(writer, "{}", format_file_header(i, total, file)).ok();
                writeln!(writer, "  {}", fmt::dim("→ skipped (user file preserved)")).ok();
                continue;
            }
            FileAction::Create | FileAction::Overwrite => {}
        }

        // Prompt loop for actionable files
        loop {
            writeln!(writer, "{}", format_file_header(i, total, file)).ok();
            write!(writer, "  Action [a/s/v/A/q]: ").ok();
            writer.flush().ok();

            let mut input = String::new();
            if reader.read_line(&mut input).is_err() || input.is_empty() {
                quit_early = true;
                break;
            }

            match parse_action(&input) {
                Some(ApprovalAction::Apply) => {
                    approved.push(i);
                    writeln!(writer, "  {}", fmt::green("✓ approved")).ok();
                    break;
                }
                Some(ApprovalAction::Skip) => {
                    writeln!(writer, "  {}", fmt::yellow("✗ skipped")).ok();
                    break;
                }
                Some(ApprovalAction::View) => {
                    print_file_detail(writer, file).ok();
                    // Loop back to re-prompt
                }
                Some(ApprovalAction::ApplyAll) => {
                    approved.push(i);
                    writeln!(writer, "  {}", fmt::green("✓ approved (and all remaining)")).ok();
                    // Approve all remaining actionable files
                    for j in (i + 1)..total {
                        let remaining = &preview.files[j];
                        if remaining.action == FileAction::Create
                            || remaining.action == FileAction::Overwrite
                        {
                            approved.push(j);
                        }
                    }
                    return Ok(ApprovalResult {
                        approved_indices: approved,
                        quit_early: false,
                    });
                }
                Some(ApprovalAction::Quit) => {
                    writeln!(
                        writer,
                        "  {}",
                        fmt::red("✗ quit — skipping remaining files")
                    )
                    .ok();
                    quit_early = true;
                    break;
                }
                None => {
                    writeln!(
                        writer,
                        "  {}",
                        fmt::red("Unknown action. Use: a (apply), s (skip), v (view), A (apply all), q (quit)")
                    )
                    .ok();
                    // Loop back to re-prompt
                }
            }
        }

        if quit_early {
            break;
        }
    }

    Ok(ApprovalResult {
        approved_indices: approved,
        quit_early,
    })
}

/// Run the full interactive approval flow: prompt user, then apply approved files.
///
/// This is the main entry point called from `cmd_init_dry_run` when on a TTY.
pub(crate) fn run_interactive(preview: &ScaffoldPreview, dir: &Path) -> Result<Vec<String>> {
    let stdin = std::io::stdin();
    let mut reader = stdin.lock();
    let mut writer = std::io::stderr();

    let result = interactive_approve(preview, &mut reader, &mut writer)?;

    if result.approved_indices.is_empty() {
        eprintln!("\nNo files approved. Nothing written.");
        return Ok(Vec::new());
    }

    let approved_files = result.approved_files(preview);
    let count = approved_files.len();

    eprintln!(
        "\n{}",
        fmt::bold(&format!(
            "Applying {count} file{}...",
            if count == 1 { "" } else { "s" }
        ))
    );

    let written = super::apply::apply_approved(dir, &approved_files)?;

    eprintln!(
        "{} Applied {} file{}.",
        fmt::green("✓"),
        written.len(),
        if written.len() == 1 { "" } else { "s" }
    );

    Ok(written)
}

#[cfg(test)]
#[path = "interactive.tests.rs"]
mod tests;
