//! Markdown-based persistence for project memory.
//!
//! Stores `ProjectMemory` as a human-readable `.magistr/memory.md` file
//! with H2 sections per category and bullet-point entries.
//!
//! Format:
//! ```markdown
//! # Project Memory — my-project
//!
//! ## Patterns
//!
//! - Uses builder pattern for config structs
//! - Prefers iterators over manual loops
//!
//! ## Observations
//!
//! - [2026-03-11] Discovered TDD workflow is enforced by hooks
//! ```

use std::path::Path;

use super::memory_category::MemoryCategory;
use super::memory_entry::MemoryEntry;
use super::project_memory::ProjectMemory;
use super::storage_error::MemoryStorageError;

/// Resolve the memory file path for a project directory.
///
/// Returns `<project_dir>/.magistr/memory.md`.
pub(crate) fn memory_path(project_dir: &Path) -> std::path::PathBuf {
    crate::paths::memory(project_dir)
}

/// Render a `ProjectMemory` as a markdown string.
#[must_use]
pub(crate) fn render(memory: &ProjectMemory) -> String {
    let mut out = String::new();

    out.push_str(&format!("# Project Memory — {}\n", memory.project_name));

    for &category in MemoryCategory::ALL {
        let entries: Vec<&MemoryEntry> = memory
            .entries
            .iter()
            .filter(|e| e.category == category)
            .collect();

        if entries.is_empty() {
            continue;
        }

        out.push_str(&format!("\n## {}\n\n", category.heading()));

        for entry in entries {
            if let Some(ts) = entry.added_at {
                let date = ts.format("%Y-%m-%d");
                out.push_str(&format!("- [{}] {}\n", date, entry.text));
            } else {
                out.push_str(&format!("- {}\n", entry.text));
            }
        }
    }

    out
}

/// Parse a markdown string back into a `ProjectMemory`.
///
/// Tolerant of malformed input — skips lines it doesn't understand.
pub(crate) fn parse(content: &str) -> ProjectMemory {
    let mut memory = ProjectMemory::default();

    // Extract project name from title
    for line in content.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("# Project Memory") {
            let name = rest.trim_start_matches([' ', '\u{2014}', '-']).trim();
            if !name.is_empty() {
                memory.project_name = name.to_string();
            }
            break;
        }
    }

    // Parse sections
    let mut current_category: Option<MemoryCategory> = None;

    for line in content.lines() {
        let trimmed = line.trim();

        // Detect H2 headings → category
        if let Some(heading) = trimmed.strip_prefix("## ") {
            current_category = MemoryCategory::from_heading(heading.trim());
            continue;
        }

        // Skip H1 and empty lines
        if trimmed.starts_with('#') || trimmed.is_empty() {
            continue;
        }

        // Parse bullet entries
        if let Some(bullet_text) = trimmed.strip_prefix("- ")
            && let Some(category) = current_category
        {
            let entry = parse_entry(category, bullet_text);
            memory.entries.push(entry);
        }
    }

    memory
}

/// Parse a single bullet-point entry, extracting optional `[date]` prefix.
fn parse_entry(category: MemoryCategory, text: &str) -> MemoryEntry {
    // Try to parse `[YYYY-MM-DD] rest of text`
    if text.starts_with('[')
        && let Some(close) = text.find(']')
    {
        let date_str = &text[1..close];
        let rest = text[close + 1..].trim();

        if let Ok(date) = chrono::NaiveDate::parse_from_str(date_str, "%Y-%m-%d") {
            let dt = date
                .and_hms_opt(0, 0, 0)
                .expect("midnight is always valid")
                .and_utc();
            return MemoryEntry {
                category,
                text: rest.to_string(),
                added_at: Some(dt),
            };
        }
    }

    MemoryEntry::new(category, text)
}

/// Load project memory from a file.
///
/// Returns empty memory if the file doesn't exist.
/// Returns an error only if the file exists but can't be read.
pub(crate) fn load(path: &Path) -> Result<ProjectMemory, MemoryStorageError> {
    if !path.is_file() {
        return Ok(ProjectMemory::default());
    }

    let content =
        std::fs::read_to_string(path).map_err(|e| MemoryStorageError::Io(path.to_owned(), e))?;
    Ok(parse(&content))
}

/// Save project memory to a file.
///
/// Creates parent directories if needed.
pub(crate) fn save(memory: &ProjectMemory, path: &Path) -> Result<(), MemoryStorageError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| MemoryStorageError::Io(parent.to_owned(), e))?;
    }

    let content = render(memory);
    std::fs::write(path, content).map_err(|e| MemoryStorageError::Io(path.to_owned(), e))?;
    Ok(())
}

#[cfg(test)]
#[path = "storage.tests.rs"]
mod tests;
