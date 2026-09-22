//! Guided initialization interview with stack-aware recommendations.
//!
//! Runs the `magistr init` interview as plain shell prompts:
//! 1. Reviews the auto-detected stack (archetype confirmation)
//! 2. Collects project metadata (type, description)
//! 3. Recommends AI agents and MCP servers based on the detected stack
//! 4. Chooses whether to install the language's lifecycle-hook preset
//! 5. Previews the quality-gate commands, with optional overrides
//! 6. Confirms selections before scaffolding
//!
//! Every prompt reads one line from an injectable `BufRead` and answers its
//! default on an empty line or EOF, so a scripted or closed stdin degrades to
//! the `--yes` answer set. Prompts render to stderr via `utils::format`.

use std::collections::HashMap;
use std::io::{self, BufRead, Write};

use magistr_core::types::ProjectTree;

use crate::error::Result;
use crate::types::{Preset, Stack};
use crate::utils::format as fmt;

use super::registry;
use super::research::RecommendedQualityGate;
use super::selections::ScaffoldSelections;

// ── Interview result ───────────────────────────────────────────────────

/// Selections collected from the guided interview.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct InterviewResult {
    /// Project type (cli, web-app, api, library, monorepo, other).
    pub project_type: String,
    /// One-line project description.
    pub description: String,
    /// Which AI agent configs to generate (e.g. "cursor", "copilot").
    pub selected_agents: Vec<String>,
    /// Additional MCP servers to include (beyond magistr + workbench-mcp).
    pub selected_mcp_servers: Vec<String>,
    /// Quality-gate command overrides from the gate preview (None = preset).
    pub quality_gate: Option<RecommendedQualityGate>,
    /// Whether to install the language's lifecycle-hook preset.
    pub include_hooks: bool,
}

impl Default for InterviewResult {
    fn default() -> Self {
        Self {
            project_type: PROJECT_TYPES[0].into(),
            description: String::new(),
            selected_agents: default_agents(),
            selected_mcp_servers: Vec::new(),
            quality_gate: None,
            include_hooks: true,
        }
    }
}

impl InterviewResult {
    /// Stack-aware defaults — the `--yes` answer set, identical to what the
    /// interview produces when every prompt is answered with Enter.
    pub(crate) fn defaults_for(stack: &Stack) -> Self {
        let selected_mcp_servers =
            registry::recommend(stack.language.as_str(), stack.framework.as_str())
                .into_iter()
                .filter(|e| e.default_enabled)
                .map(|e| e.key.to_string())
                .collect();
        Self {
            selected_mcp_servers,
            ..Self::default()
        }
    }

    /// Convert into the pipeline's selection carrier.
    pub(crate) fn to_selections(&self) -> ScaffoldSelections {
        ScaffoldSelections {
            project_type: self.project_type.clone(),
            description: self.description.clone(),
            selected_agents: self.selected_agents.clone(),
            selected_mcp_servers: self.selected_mcp_servers.clone(),
            quality_gate: self.quality_gate.clone(),
            ..ScaffoldSelections::default()
        }
    }
}

// ── Agent definitions ──────────────────────────────────────────────────

/// An AI agent that magistr can generate configuration for.
struct AgentDef {
    /// Internal ID (matches emitter names).
    id: &'static str,
    /// Display name for the interview.
    label: &'static str,
    /// One-line description of what gets generated.
    description: &'static str,
    /// Whether it's selected by default.
    default: bool,
}

/// All supported agents (Claude Code config is always generated).
const ALL_AGENTS: &[AgentDef] = &[
    AgentDef {
        id: "cursor",
        label: "Cursor",
        description: ".cursor/rules/ MDC files",
        default: true,
    },
    AgentDef {
        id: "copilot",
        label: "GitHub Copilot",
        description: ".github/copilot-instructions.md",
        default: false,
    },
    AgentDef {
        id: "cline",
        label: "Cline",
        description: ".clinerules conventions file",
        default: false,
    },
    AgentDef {
        id: "windsurf",
        label: "Windsurf",
        description: ".windsurfrules conventions file",
        default: false,
    },
    AgentDef {
        id: "aider",
        label: "Aider",
        description: ".aider.conf.yml configuration",
        default: false,
    },
    AgentDef {
        id: "codex",
        label: "Codex",
        description: ".codex/config.toml, AGENTS.md, and .agents/skills",
        default: false,
    },
];

/// The agent IDs selected by default.
fn default_agents() -> Vec<String> {
    ALL_AGENTS
        .iter()
        .filter(|a| a.default)
        .map(|a| a.id.to_string())
        .collect()
}

const PROJECT_TYPES: &[&str] = &["cli", "web-app", "api", "library", "monorepo", "other"];

// ── Main interview flow ────────────────────────────────────────────────

/// Run the full guided interview. Returns selections for scaffold generation.
///
/// Reads answers from `reader` (stdin in production) so tests can script the
/// whole interview. Returns `MagistrError::Cancelled` when the user declines
/// the detected stack or the final confirmation.
pub(crate) fn run_interview(
    reader: &mut impl BufRead,
    stack: &Stack,
    preset: &Preset,
) -> Result<InterviewResult> {
    print_header();
    print_detected_stack(stack);

    // Step 1: Archetype confirmation
    if !prompt_yes_no(reader, "\nUse this detected stack?", true)? {
        return Err(crate::error::MagistrError::Cancelled(
            "Stack not confirmed — inspect with `magistr detect --smart`, then re-run `magistr init`"
                .into(),
        ));
    }

    // Step 2: Project info
    eprintln!("\n{}  {}", fmt::bold("Step 1/5"), fmt::dim("Project Info"));
    eprintln!("{}", fmt::dim(&"─".repeat(40)));
    let project_type = prompt_select(reader, "Project type:", PROJECT_TYPES, 0)?;
    let description = prompt_text(reader, "Description (one line):")?;

    // Step 3: Agent selection
    eprintln!("\n{}  {}", fmt::bold("Step 2/5"), fmt::dim("AI Agents"));
    eprintln!("{}", fmt::dim(&"─".repeat(40)));
    eprintln!("{}", fmt::dim("Select which AI agent configs to generate."));
    let agent_labels: Vec<String> = ALL_AGENTS
        .iter()
        .map(|a| format!("{} — {}", a.label, a.description))
        .collect();
    let agent_label_refs: Vec<&str> = agent_labels.iter().map(|s| s.as_str()).collect();
    let agent_defaults: Vec<bool> = ALL_AGENTS.iter().map(|a| a.default).collect();
    let agent_picks = prompt_multi_select(reader, &agent_label_refs, &agent_defaults)?;
    let selected_agents: Vec<String> = agent_picks
        .iter()
        .map(|&i| ALL_AGENTS[i].id.to_string())
        .collect();

    // Step 4: MCP server recommendations
    eprintln!("\n{}  {}", fmt::bold("Step 3/5"), fmt::dim("MCP Servers"));
    eprintln!("{}", fmt::dim(&"─".repeat(40)));
    let lang = stack.language.as_str();
    let fw = stack.framework.as_str();
    let recommended = registry::recommend(lang, fw);
    let selected_mcp_servers = if recommended.is_empty() {
        eprintln!(
            "{}",
            fmt::dim("No additional MCP servers recommended for this stack.")
        );
        Vec::new()
    } else {
        eprintln!(
            "{}",
            fmt::dim(&format!(
                "Recommended for {lang} + {fw} (magistr + workbench-mcp always included):"
            ))
        );
        let labels: Vec<String> = recommended
            .iter()
            .map(|r| format!("{} — {}", r.name, r.description))
            .collect();
        let label_refs: Vec<&str> = labels.iter().map(|s| s.as_str()).collect();
        let defaults: Vec<bool> = recommended.iter().map(|r| r.default_enabled).collect();
        let picks = prompt_multi_select(reader, &label_refs, &defaults)?;
        picks
            .iter()
            .map(|&i| recommended[i].key.to_string())
            .collect()
    };

    // Step 5: Hooks + quality-gate preview
    eprintln!(
        "\n{}  {}",
        fmt::bold("Step 4/5"),
        fmt::dim("Hooks & Quality Gates")
    );
    eprintln!("{}", fmt::dim(&"─".repeat(40)));
    let include_hooks = prompt_yes_no(
        reader,
        &format!(
            "Install the {} lifecycle-hook preset (auto-format, gate-on-stop)?",
            stack.language
        ),
        true,
    )?;
    eprintln!("  Quality gate commands:");
    for (name, command) in [
        ("check", &preset.quality_gate.check),
        ("test", &preset.quality_gate.test),
        ("lint", &preset.quality_gate.lint),
    ] {
        let display = if command.trim().is_empty() {
            "not configured"
        } else {
            command
        };
        eprintln!("    {name}: {}", fmt::cyan(display));
    }
    let quality_gate = if prompt_yes_no(reader, "Customize gate commands?", false)? {
        let check = prompt_text_default(reader, "check:", &preset.quality_gate.check)?;
        let test = prompt_text_default(reader, "test:", &preset.quality_gate.test)?;
        let lint = prompt_text_default(reader, "lint:", &preset.quality_gate.lint)?;
        Some(RecommendedQualityGate {
            check: Some(check),
            test: Some(test),
            lint: Some(lint),
        })
    } else {
        None
    };

    // Step 6: Confirm
    eprintln!("\n{}  {}", fmt::bold("Step 5/5"), fmt::dim("Confirm"));
    eprintln!("{}", fmt::dim(&"─".repeat(40)));
    eprintln!("  Project:    {}", stack.project_name);
    eprintln!("  Stack:      {lang} + {fw}");
    eprintln!("  Type:       {}", PROJECT_TYPES[project_type]);
    eprintln!("  Agents:     {}", selected_agents.join(", "));
    let mcp_display: Vec<String> = ["magistr", "workbench-mcp"]
        .iter()
        .map(|s| s.to_string())
        .chain(selected_mcp_servers.iter().cloned())
        .collect();
    eprintln!("  MCP:        {}", mcp_display.join(", "));
    eprintln!(
        "  Hooks:      {}",
        if include_hooks { "preset" } else { "none" }
    );
    eprintln!(
        "  Gates:      {}",
        if quality_gate.is_some() {
            "customized"
        } else {
            "preset"
        }
    );

    if !prompt_yes_no(reader, "\nProceed with scaffolding?", true)? {
        return Err(crate::error::MagistrError::Cancelled(
            "Cancelled by user".into(),
        ));
    }

    Ok(InterviewResult {
        project_type: PROJECT_TYPES[project_type].to_string(),
        description,
        selected_agents,
        selected_mcp_servers,
        quality_gate,
        include_hooks,
    })
}

// ── Polyglot project-tree confirmation ─────────────────────────────────

/// Present the detected project tree for confirmation, letting the user
/// exclude false positives.
///
/// Returns the (possibly filtered) tree, or `None` when every project was
/// excluded. An empty line or EOF keeps the whole tree.
pub(crate) fn confirm_project_tree(
    reader: &mut impl BufRead,
    tree: &ProjectTree,
) -> Result<Option<ProjectTree>> {
    eprintln!("\n  {} Detected {} projects:", fmt::green("✓"), tree.len());
    for (i, project) in tree.projects.iter().enumerate() {
        let path = if project.path.as_os_str().is_empty() {
            ".".to_string()
        } else {
            project.path.display().to_string()
        };
        let f_label = project
            .framework
            .map(|f| format!(" + {}", f.as_str()))
            .unwrap_or_default();
        let name = project.metadata.name.as_deref().unwrap_or("");
        eprintln!(
            "    {}) {path} [{}{f_label}] {name}",
            i + 1,
            project.language.as_str()
        );
    }
    eprint!(
        "  {} ",
        fmt::dim("[comma-separated numbers to exclude, Enter to include all]:")
    );
    io::stderr().flush().ok();

    let mut line = String::new();
    reader.read_line(&mut line).ok();
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return Ok(Some(tree.clone()));
    }

    let mut excluded = vec![false; tree.len()];
    for part in trimmed.split(',') {
        if let Ok(n) = part.trim().parse::<usize>()
            && n >= 1
            && n <= tree.len()
        {
            excluded[n - 1] = true;
        }
    }

    let mut filtered = tree.clone();
    let mut keep = excluded.iter().map(|e| !e);
    filtered.projects.retain(|_| keep.next().unwrap_or(true));
    if filtered.projects.is_empty() {
        return Ok(None);
    }
    Ok(Some(filtered))
}

// ── Prompt helpers ─────────────────────────────────────────────────────

fn print_header() {
    eprintln!();
    eprintln!(
        "  {}",
        fmt::bold("magistr — Universal AI Agent Configuration")
    );
    eprintln!();
}

fn print_detected_stack(stack: &Stack) {
    eprintln!(
        "  {} Detected: {} + {} ({})",
        fmt::green("✓"),
        stack.language,
        stack.framework,
        stack.package_manager
    );
    eprintln!(
        "  {} Found: {}, {}, {}",
        fmt::green("✓"),
        stack.test_framework,
        stack.linter,
        stack.formatter
    );
}

/// Prompt for a single selection from a numbered list. Returns the index.
fn prompt_select(
    reader: &mut impl BufRead,
    question: &str,
    options: &[&str],
    default: usize,
) -> Result<usize> {
    eprintln!("  {question}");
    for (i, opt) in options.iter().enumerate() {
        let marker = if i == default {
            fmt::green("❯")
        } else {
            " ".into()
        };
        eprintln!("    {marker} {}) {opt}", i + 1);
    }
    eprint!(
        "  {} ",
        fmt::dim(&format!("[1-{}, default={}]:", options.len(), default + 1))
    );
    io::stderr().flush().ok();

    let mut line = String::new();
    reader.read_line(&mut line).ok();
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return Ok(default);
    }
    match trimmed.parse::<usize>() {
        Ok(n) if n >= 1 && n <= options.len() => Ok(n - 1),
        _ => Ok(default),
    }
}

/// Prompt for a text input. Returns the trimmed string.
fn prompt_text(reader: &mut impl BufRead, question: &str) -> Result<String> {
    eprint!("  {question} ");
    io::stderr().flush().ok();
    let mut line = String::new();
    reader.read_line(&mut line).ok();
    Ok(line.trim().to_string())
}

/// Prompt for a text input with a default. Enter keeps the default.
fn prompt_text_default(reader: &mut impl BufRead, question: &str, default: &str) -> Result<String> {
    eprint!("  {question} {} ", fmt::dim(&format!("[{default}]:")));
    io::stderr().flush().ok();
    let mut line = String::new();
    reader.read_line(&mut line).ok();
    let trimmed = line.trim();
    if trimmed.is_empty() {
        Ok(default.to_string())
    } else {
        Ok(trimmed.to_string())
    }
}

/// Prompt for yes/no. Returns bool.
fn prompt_yes_no(reader: &mut impl BufRead, question: &str, default: bool) -> Result<bool> {
    let hint = if default { "Y/n" } else { "y/N" };
    eprint!("  {question} {} ", fmt::dim(&format!("[{hint}]:")));
    io::stderr().flush().ok();

    let mut line = String::new();
    reader.read_line(&mut line).ok();
    let trimmed = line.trim().to_lowercase();
    Ok(match trimmed.as_str() {
        "" => default,
        "y" | "yes" => true,
        "n" | "no" => false,
        _ => default,
    })
}

/// Prompt for multi-select with toggleable defaults. Returns indices of selected items.
fn prompt_multi_select(
    reader: &mut impl BufRead,
    options: &[&str],
    defaults: &[bool],
) -> Result<Vec<usize>> {
    for (i, opt) in options.iter().enumerate() {
        let checked = if *defaults.get(i).unwrap_or(&false) {
            fmt::green("[x]")
        } else {
            fmt::dim("[ ]")
        };
        eprintln!("    {checked} {}) {opt}", i + 1);
    }
    eprint!(
        "  {} ",
        fmt::dim("[comma-separated numbers to toggle, Enter for defaults]:")
    );
    io::stderr().flush().ok();

    let mut line = String::new();
    reader.read_line(&mut line).ok();
    let trimmed = line.trim();

    if trimmed.is_empty() {
        // Use defaults
        return Ok(defaults
            .iter()
            .enumerate()
            .filter(|&(_, &d)| d)
            .map(|(i, _)| i)
            .collect());
    }

    // Parse comma-separated numbers and toggle
    let mut selected: Vec<bool> = defaults.to_vec();
    for part in trimmed.split(',') {
        if let Ok(n) = part.trim().parse::<usize>()
            && n >= 1
            && n <= options.len()
        {
            selected[n - 1] = !selected[n - 1];
        }
    }

    Ok(selected
        .iter()
        .enumerate()
        .filter(|&(_, &s)| s)
        .map(|(i, _)| i)
        .collect())
}

// ── Integration with .mcp.json generation ──────────────────────────────

/// Build the additional MCP server entries for merge into `.mcp.json`.
///
/// Returns a map of server name → JSON value, ready for merge.
pub(crate) fn build_extra_mcp_entries(
    server_names: &[String],
) -> HashMap<String, serde_json::Value> {
    let mut entries = HashMap::new();
    for name in server_names {
        if let Some(entry) = registry::find(name) {
            entries.insert(name.clone(), entry.to_mcp_value());
        }
    }
    entries
}

#[cfg(test)]
#[path = "interview.tests.rs"]
mod tests;
