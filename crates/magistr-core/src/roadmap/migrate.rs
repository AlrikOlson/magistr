//! Markdown → YAML migration tool.
//!
//! Parses an existing `ROADMAP.md` into a [`Roadmap`] struct that can be
//! saved as `.magistr/roadmap.yaml` via [`YamlStore::from_roadmap`].
//!
//! # Supported Format
//!
//! ```markdown
//! # Title
//!
//! Preamble paragraphs...
//!
//! **Vision:** vision text...
//!
//! **Principles:**
//! - Principle one
//! - Principle two
//!
//! ---
//!
//! ## Phase E0: Name ✦ "Tagline"
//!
//! **Problem:** problem text...
//!
//! **Solution:** solution text...
//!
//! ### Tasks
//!
//! - [x] Task text
//!   - [x] Subtask text
//! - [ ] Another task
//!
//! ---
//!
//! Epilogue text...
//! ```

use super::phase::Phase;
use super::phase_status::PhaseStatus;
use super::schema::Roadmap;
use super::task::Task;
use crate::error::Result;

/// Parse markdown content into a [`Roadmap`] struct.
pub fn parse_markdown(content: &str) -> Result<Roadmap> {
    let lines: Vec<&str> = content.lines().collect();
    let mut i = 0;

    // ── Title (# heading) ───────────────────────────────────────────────
    let title = skip_blank(&lines, &mut i)
        .and_then(|line| line.strip_prefix("# "))
        .map(|t| t.trim().to_string())
        .unwrap_or_default();
    if !title.is_empty() {
        i += 1;
    }

    // ── Preamble, Vision, Principles (before first ## phase) ────────────
    let mut preamble_lines: Vec<&str> = Vec::new();
    let mut vision = String::new();
    let mut principles: Vec<String> = Vec::new();

    while i < lines.len() {
        let line = lines[i];

        // Stop at first phase heading
        if line.starts_with("## ") {
            break;
        }

        // Skip horizontal rules
        if is_horizontal_rule(line) {
            i += 1;
            continue;
        }

        // Vision
        if let Some(rest) = line.strip_prefix("**Vision:**") {
            let mut vis = rest.trim().to_string();
            i += 1;
            // Collect continuation lines
            while i < lines.len()
                && !lines[i].is_empty()
                && !lines[i].starts_with("**")
                && !lines[i].starts_with("## ")
                && !lines[i].starts_with("- ")
                && !is_horizontal_rule(lines[i])
            {
                vis.push('\n');
                vis.push_str(lines[i].trim());
                i += 1;
            }
            vision = vis;
            continue;
        }

        // Principles
        if line.starts_with("**Principles:**") {
            i += 1;
            while i < lines.len() && lines[i].starts_with("- ") {
                principles.push(lines[i][2..].trim().to_string());
                i += 1;
            }
            continue;
        }

        preamble_lines.push(line);
        i += 1;
    }

    let preamble = collapse_blank_lines(&preamble_lines);

    // ── Phases ──────────────────────────────────────────────────────────
    let mut phases: Vec<Phase> = Vec::new();

    while i < lines.len() {
        // Skip blank lines and horizontal rules between phases
        if lines[i].trim().is_empty() || is_horizontal_rule(lines[i]) {
            i += 1;
            continue;
        }

        let line = lines[i];

        if let Some(header) = line.strip_prefix("## ") {
            let phase = parse_phase_header(header);
            i += 1;

            // Collect phase body until next ##, --- separator, or end
            let mut problem = String::new();
            let mut solution = String::new();
            let mut tasks: Vec<Task> = Vec::new();
            let mut in_tasks = false;

            while i < lines.len() && !lines[i].starts_with("## ") && !is_horizontal_rule(lines[i]) {
                let pline = lines[i];

                if pline.starts_with("### Tasks") {
                    in_tasks = true;
                    i += 1;
                    continue;
                }

                if pline.starts_with("### ") {
                    // Other subsections — skip header, continue
                    in_tasks = false;
                    i += 1;
                    continue;
                }

                if in_tasks && let Some(task) = try_parse_task_line(pline, 0) {
                    // Collect subtasks
                    i += 1;
                    let subtasks = collect_subtasks(&lines, &mut i, 2);
                    tasks.push(Task { subtasks, ..task });
                    continue;
                }

                if !in_tasks {
                    if let Some(rest) = pline.strip_prefix("**Problem:**") {
                        problem = collect_prose(&lines, &mut i, rest.trim());
                        continue;
                    }
                    if let Some(rest) = pline.strip_prefix("**Solution:**") {
                        solution = collect_prose(&lines, &mut i, rest.trim());
                        continue;
                    }
                }

                i += 1;
            }

            let mut phase = phase;
            phase.problem = problem;
            phase.solution = solution;
            phase.tasks = tasks;
            phase.recompute_status();
            phases.push(phase);
        } else {
            // We've passed all phases — rest is epilogue
            break;
        }
    }

    // ── Epilogue ────────────────────────────────────────────────────────
    let mut epilogue_lines: Vec<&str> = Vec::new();
    while i < lines.len() {
        let line = lines[i];
        if !is_horizontal_rule(line) {
            epilogue_lines.push(line);
        }
        i += 1;
    }
    let epilogue = collapse_blank_lines(&epilogue_lines);

    let mut roadmap = Roadmap {
        title,
        preamble,
        vision,
        principles,
        phases,
        epilogue,
    };
    roadmap.recompute_statuses();

    Ok(roadmap)
}

// ── Helpers ─────────────────────────────────────────────────────────────────

/// Skip blank lines, returning the first non-blank line (if any).
fn skip_blank<'a>(lines: &[&'a str], i: &mut usize) -> Option<&'a str> {
    while *i < lines.len() && lines[*i].trim().is_empty() {
        *i += 1;
    }
    lines.get(*i).copied()
}

fn is_horizontal_rule(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed == "---" || trimmed == "***" || trimmed == "___"
}

/// Parse a phase heading like `Phase E0: Name ✦ "Tagline"`
fn parse_phase_header(header: &str) -> Phase {
    // Strip optional "Phase " prefix
    let header = header.strip_prefix("Phase ").unwrap_or(header).trim();

    // Try to extract tagline from ✦ "..." or — "..."
    let (main, tagline) = if let Some(pos) = header.find('✦') {
        let before = header[..pos].trim();
        let after = header[pos + '✦'.len_utf8()..].trim();
        let tagline = after
            .trim_start_matches('"')
            .trim_start_matches('\u{201c}') // left double quote
            .trim_end_matches('"')
            .trim_end_matches('\u{201d}') // right double quote
            .trim()
            .to_string();
        (
            before,
            if tagline.is_empty() {
                None
            } else {
                Some(tagline)
            },
        )
    } else {
        (header, None)
    };

    // Split ID: Name
    let (id, name) = if let Some(colon_pos) = main.find(':') {
        let id = main[..colon_pos].trim().to_string();
        let name = main[colon_pos + 1..].trim().to_string();
        (id, name)
    } else {
        // No colon — auto-generate an ID from the name.
        // Take uppercase initials of each word, e.g. "Migration Path" → "MP".
        let auto_id: String = main
            .split_whitespace()
            .filter_map(|w| w.chars().next())
            .map(|c| c.to_uppercase().next().unwrap_or(c))
            .collect();
        let id = if auto_id.is_empty() {
            "X0".to_string()
        } else {
            auto_id
        };
        (id, main.to_string())
    };

    Phase {
        id,
        name,
        tagline,
        status: PhaseStatus::default(),
        problem: String::new(),
        solution: String::new(),
        depends_on: vec![],
        tasks: vec![],
    }
}

/// Try to parse a task line at the given indent level.
///
/// A task line looks like `- [x] Text` or `- [ ] Text`, with optional
/// leading whitespace for nesting.
fn try_parse_task_line(line: &str, indent_spaces: usize) -> Option<Task> {
    let prefix = " ".repeat(indent_spaces);
    let stripped = if indent_spaces == 0 {
        line.trim_start()
    } else {
        line.strip_prefix(&prefix)?
    };

    let (checked, text) = if let Some(rest) = stripped
        .strip_prefix("- [x] ")
        .or_else(|| stripped.strip_prefix("- [X] "))
    {
        (true, rest.trim().to_string())
    } else if let Some(rest) = stripped.strip_prefix("- [ ] ") {
        (false, rest.trim().to_string())
    } else {
        return None;
    };

    Some(Task {
        text,
        checked,
        detail: String::new(),
        subtasks: vec![],
    })
}

/// Collect subtasks at the given indent level.
fn collect_subtasks(lines: &[&str], i: &mut usize, indent: usize) -> Vec<Task> {
    let mut tasks = Vec::new();
    while *i < lines.len() {
        if let Some(task) = try_parse_task_line(lines[*i], indent) {
            *i += 1;
            let subtasks = collect_subtasks(lines, i, indent + 2);
            tasks.push(Task { subtasks, ..task });
        } else {
            break;
        }
    }
    tasks
}

/// Collect prose continuation lines after a `**Label:**` prefix.
///
/// Advances `i` past the collected lines.
fn collect_prose(lines: &[&str], i: &mut usize, first_line: &str) -> String {
    let mut text = first_line.to_string();
    *i += 1;
    while *i < lines.len() {
        let line = lines[*i];
        if line.is_empty()
            || line.starts_with("**")
            || line.starts_with("## ")
            || line.starts_with("### ")
            || line.starts_with("- [")
            || is_horizontal_rule(line)
        {
            break;
        }
        if !text.is_empty() {
            text.push('\n');
        }
        text.push_str(line);
        *i += 1;
    }
    text
}

/// Collapse leading/trailing blank lines and trim.
fn collapse_blank_lines(lines: &[&str]) -> String {
    let joined = lines.join("\n");
    joined.trim().to_string()
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
#[path = "migrate.tests.rs"]
mod tests;
