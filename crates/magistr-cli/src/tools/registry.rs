//! Tool registry — typed tool definitions and extraction from ACP events.
//!
//! Each `MagistrTool` describes a tool that magistr's MCP server exposes.
//! The `ToolRegistry` provides:
//!
//! 1. **Extraction** — `try_extract(name, input)` deserializes raw JSON into
//!    a typed `ToolOutput` variant. No manual parsing needed.
//! 2. **MCP tool generation** — `build_mcp_tools()` returns `SdkMcpTool`
//!    instances ready to plug into an `SdkMcpServer`.
//! 3. **Schema introspection** — each tool carries its JSON Schema for
//!    validation and documentation.

use magistr_mcp::{SdkMcpTool, ToolResult};
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use std::collections::HashMap;

use crate::scaffold::research::Recommendations;

// ── Tool Output ────────────────────────────────────────────────────────

/// Typed payload extracted from an ACP tool call.
///
/// Each variant corresponds to a registered `MagistrTool`. Adding a new tool
/// means adding a variant here and a `MagistrTool` entry in `ToolRegistry::new()`.
#[derive(Debug, Clone)]
pub enum ToolOutput {
    /// Structured project recommendations from the AI interview.
    Recommend(Box<Recommendations>),
    /// The AI is asking the user a structured question.
    Ask(Question),
    /// Tool call matched a registered name but deserialization failed.
    /// Carries the tool name and raw input for diagnostics.
    Invalid {
        tool: String,
        input: Value,
        error: String,
    },
}

/// A structured question from the AI to the user.
///
/// Used by `magistr_ask` to signal the interview needs user input.
/// The wizard shows the question prominently and waits for a response.
///
/// Each question belongs to a project category (testing, CI/CD, etc.)
/// which determines how it's displayed in the decision panel.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct Question {
    /// The question text to display.
    pub question: String,
    /// Optional multiple-choice options. Supports both plain strings and
    /// rich objects with label + description for showing pros/cons.
    #[serde(default, deserialize_with = "deserialize_choices")]
    pub choices: Vec<Choice>,
    /// Whether to allow freeform text input in addition to choices.
    #[serde(default = "default_true")]
    pub allow_freeform: bool,
    /// Whether multiple choices can be selected (checkboxes vs radio buttons).
    /// When true, the user can toggle multiple selections before confirming.
    #[serde(default)]
    pub multi_select: bool,
    /// Category this question belongs to (e.g. "testing", "ci_cd", "mcp_servers").
    /// Used to group and display the question in the correct decision panel section.
    #[serde(default)]
    pub category: Option<String>,
    /// Icon for this category (e.g. "🧪", "🔄", "📡").
    /// Falls back to a default based on category if not provided.
    #[serde(default)]
    pub icon: Option<String>,
}

/// A single choice option in a question.
///
/// Supports both plain labels and rich labels with descriptions (for
/// presenting pros/cons of each option to the user).
#[derive(Debug, Clone, PartialEq, serde::Deserialize)]
#[serde(untagged)]
pub enum Choice {
    /// Rich choice with label and description (pros/cons).
    Rich {
        label: String,
        #[serde(default)]
        description: Option<String>,
    },
    /// Choice containing only its display text.
    Plain(String),
}

impl Choice {
    /// Get the display label for this choice.
    pub fn label(&self) -> &str {
        match self {
            Choice::Rich { label, .. } => label,
            Choice::Plain(s) => s,
        }
    }

    /// Get the optional description (pros/cons) for this choice.
    pub fn description(&self) -> Option<&str> {
        match self {
            Choice::Rich { description, .. } => description.as_deref(),
            Choice::Plain(_) => None,
        }
    }
}

impl Question {
    /// Get the display icon for this question's category.
    ///
    /// Returns the explicit icon if set, otherwise derives from category name.
    pub fn display_icon(&self) -> &str {
        if let Some(ref icon) = self.icon {
            return icon;
        }
        match self.category.as_deref() {
            Some("code_quality") => "🔍",
            Some("testing") => "🧪",
            Some("error_handling") => "⚠️",
            Some("ci_cd") => "🔄",
            Some("documentation") => "📖",
            Some("dependencies") => "📦",
            Some("architecture") => "🏗️",
            Some("logging") => "📊",
            Some("mcp_servers") => "📡",
            Some("agents") => "🤖",
            Some("security") => "🔒",
            _ => "❓",
        }
    }

    /// Get a human-readable title for this question's category.
    pub fn category_title(&self) -> &str {
        match self.category.as_deref() {
            Some("code_quality") => "Code Quality",
            Some("testing") => "Testing Strategy",
            Some("error_handling") => "Error Handling",
            Some("ci_cd") => "CI/CD Pipeline",
            Some("documentation") => "Documentation",
            Some("dependencies") => "Dependencies",
            Some("architecture") => "Architecture",
            Some("logging") => "Logging & Observability",
            Some("mcp_servers") => "MCP Servers",
            Some("agents") => "AI Agents",
            Some("security") => "Security",
            _ => "Question",
        }
    }
}

/// Custom deserializer that accepts both `["a", "b"]` and `[{label, description}, ...]`.
fn deserialize_choices<'de, D>(deserializer: D) -> Result<Vec<Choice>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de;

    struct ChoicesVisitor;

    impl<'de> de::Visitor<'de> for ChoicesVisitor {
        type Value = Vec<Choice>;

        fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            f.write_str("an array of strings or choice objects")
        }

        fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
        where
            A: de::SeqAccess<'de>,
        {
            let mut choices = Vec::new();
            while let Some(val) = seq.next_element::<serde_json::Value>()? {
                match val {
                    serde_json::Value::String(s) => choices.push(Choice::Plain(s)),
                    serde_json::Value::Object(map) => {
                        let label = map
                            .get("label")
                            .and_then(|v| v.as_str())
                            .unwrap_or("?")
                            .to_string();
                        let description = map
                            .get("description")
                            .and_then(|v| v.as_str())
                            .map(String::from);
                        choices.push(Choice::Rich { label, description });
                    }
                    _ => {}
                }
            }
            Ok(choices)
        }
    }

    deserializer.deserialize_seq(ChoicesVisitor)
}

fn default_true() -> bool {
    true
}

// ── Tool Definition ────────────────────────────────────────────────────

/// A tool definition: name, description, JSON schema, and extraction logic.
///
/// This is not a trait — it's a concrete struct that captures the extraction
/// closure at construction time. This keeps the registry simple and avoids
/// `dyn Trait` complexity.
pub struct MagistrTool {
    /// MCP tool name (e.g. `"magistr_recommend"`).
    pub name: &'static str,
    /// Human-readable description for the AI.
    pub description: &'static str,
    /// JSON Schema for the tool's input parameters.
    pub schema: Value,
    /// Extract typed output from raw JSON input.
    extractor: Box<dyn Fn(&Value) -> ToolOutput + Send + Sync>,
}

impl MagistrTool {
    /// Create a tool that deserializes input into `T` and wraps it in a `ToolOutput`.
    fn typed<T, F>(name: &'static str, description: &'static str, schema: Value, wrap: F) -> Self
    where
        T: DeserializeOwned + 'static,
        F: Fn(T) -> ToolOutput + Send + Sync + 'static,
    {
        Self {
            name,
            description,
            schema,
            extractor: Box::new(
                move |input| match serde_json::from_value::<T>(input.clone()) {
                    Ok(val) => wrap(val),
                    Err(e) => ToolOutput::Invalid {
                        tool: name.to_string(),
                        input: input.clone(),
                        error: e.to_string(),
                    },
                },
            ),
        }
    }

    /// Try to extract a typed output from raw JSON input.
    pub fn extract(&self, input: &Value) -> ToolOutput {
        (self.extractor)(input)
    }

    /// Build an `SdkMcpTool` for use in the magistr MCP server.
    ///
    /// The MCP handler validates input and returns success — actual processing
    /// happens on the magistr side by observing late-arriving `raw_input` in ACP
    /// `StreamEvent` updates.
    pub fn to_mcp_tool(&self) -> SdkMcpTool {
        let name = self.name;
        let is_ask = name == "magistr_ask";
        let schema = self.schema.clone();
        SdkMcpTool::new(
            self.name,
            self.description,
            self.schema.clone(),
            move |input| {
                let schema = schema.clone();
                Box::pin(async move {
                    // Basic schema validation — check required fields exist
                    if let Some(required) = schema.get("required").and_then(|r| r.as_array()) {
                        for field in required {
                            if let Some(field_name) = field.as_str()
                                && input.get(field_name).is_none()
                            {
                                return Ok(ToolResult::error(format!(
                                    "Missing required field: {field_name}"
                                )));
                            }
                        }
                    }
                    // The ACP interview host consumes this payload separately.
                    // A direct MCP call cannot confirm display or configuration writes.
                    if is_ask {
                        Ok(ToolResult::text(
                            "Question received for the Magistr interview host. This handler does not display UI. \
                             Outside a Magistr interview, ask through your host's user-input tool. \
                             Continue only after the user answers.",
                        ))
                    } else {
                        Ok(ToolResult::text(format!(
                            "{name}: recommendations received for the Magistr interview host. \
                             This handler does not apply configuration changes."
                        )))
                    }
                })
            },
        )
    }
}

impl std::fmt::Debug for MagistrTool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MagistrTool")
            .field("name", &self.name)
            .field("description", &self.description)
            .finish()
    }
}

// ── Tool Registry ──────────────────────────────────────────────────────

/// Registry of all magistr tools.
///
/// Constructed once, used to:
/// - Extract typed data from ACP `ToolCall` events
/// - Generate MCP tools for the server
/// - Look up tools by name (supports MCP naming: `mcp__jig__tool_name`)
pub struct ToolRegistry {
    tools: Vec<MagistrTool>,
    /// Name → index lookup (includes both bare and MCP-prefixed names).
    index: HashMap<String, usize>,
}

impl ToolRegistry {
    /// Create the default registry with all built-in magistr tools.
    pub fn new() -> Self {
        let tools = vec![Self::recommend_tool(), Self::ask_tool()];
        let mut index = HashMap::new();
        for (i, tool) in tools.iter().enumerate() {
            // Bare name: "magistr_recommend"
            index.insert(tool.name.to_string(), i);
            // MCP-prefixed: "mcp__jig__magistr_recommend"
            index.insert(format!("mcp__jig__{}", tool.name), i);
        }
        Self { tools, index }
    }

    /// Try to extract a typed `ToolOutput` from a tool call.
    ///
    /// Returns `None` if the tool name doesn't match any registered tool.
    /// Returns `Some(ToolOutput::Invalid { .. })` if the name matches but
    /// deserialization fails.
    pub fn try_extract(&self, name: &str, input: &Value) -> Option<ToolOutput> {
        // Normalize: strip MCP server prefix variations
        let normalized = normalize_tool_name(name);
        self.index
            .get(normalized.as_ref())
            .map(|&idx| self.tools[idx].extract(input))
    }

    /// Check if a tool name matches any registered tool.
    pub fn is_magistr_tool(&self, name: &str) -> bool {
        let normalized = normalize_tool_name(name);
        self.index.contains_key(normalized.as_ref())
    }

    /// Get all tool names (bare, without MCP prefix).
    pub fn tool_names(&self) -> Vec<&'static str> {
        self.tools.iter().map(|t| t.name).collect()
    }

    /// Build `SdkMcpTool` instances for all registered tools.
    ///
    /// Used to extend the magistr MCP server with these tools.
    pub fn build_mcp_tools(&self) -> Vec<SdkMcpTool> {
        self.tools.iter().map(|t| t.to_mcp_tool()).collect()
    }

    // ── Built-in tool definitions ──────────────────────────────────

    /// `magistr_recommend` — submit structured project recommendations.
    fn recommend_tool() -> MagistrTool {
        MagistrTool::typed::<Recommendations, _>(
            "magistr_recommend",
            "Submit structured project recommendations. Call this tool with your \
             findings instead of outputting JSON. Include ALL project configuration: \
             quality gates, testing, format, conventions, MCP servers, agents, and description.",
            json!({
                "type": "object",
                "properties": {
                    "mcp_servers": {
                        "type": "array",
                        "description": "MCP servers to recommend for this project",
                        "items": {
                            "type": "object",
                            "properties": {
                                "key": { "type": "string", "description": "Config key (e.g. 'context7')" },
                                "name": { "type": "string", "description": "Display name" },
                                "description": { "type": "string", "description": "What it does" },
                                "command": { "type": "string", "description": "Command to run (e.g. 'npx')" },
                                "args": { "type": "array", "items": { "type": "string" }, "description": "Command arguments" },
                                "category": { "type": "string", "description": "Category (docs, code, etc.)" }
                            },
                            "required": ["key", "name", "description"]
                        }
                    },
                    "agents": {
                        "type": "array",
                        "description": "Recommended AI agents for this stack",
                        "items": {
                            "type": "object",
                            "properties": {
                                "key": { "type": "string", "description": "Agent key (e.g. 'cursor')" },
                                "reason": { "type": "string", "description": "Why this agent is good" }
                            },
                            "required": ["key", "reason"]
                        }
                    },
                    "conventions": {
                        "type": "array",
                        "description": "Coding conventions and best practices for AI agents to follow",
                        "items": { "type": "string" }
                    },
                    "tips": {
                        "type": "array",
                        "description": "Practical tips for AI tool configuration",
                        "items": { "type": "string" }
                    },
                    "suggested_description": {
                        "type": "string",
                        "description": "One-line project description based on codebase analysis"
                    },
                    "suggested_project_type": {
                        "type": "string",
                        "description": "Project type: cli, web-app, api, library, monorepo, or other",
                        "enum": ["cli", "web-app", "api", "library", "monorepo", "other"]
                    },
                    "quality_gate": {
                        "type": "object",
                        "description": "Quality gate commands discovered from the codebase (CI, Makefile, scripts, etc.)",
                        "properties": {
                            "check": { "type": "string", "description": "Build/check command (e.g. 'cargo check --all-targets')" },
                            "test": { "type": "string", "description": "Test command (e.g. 'cargo test')" },
                            "lint": { "type": "string", "description": "Lint command (e.g. 'cargo clippy -- -D warnings')" }
                        }
                    },
                    "format": {
                        "type": "object",
                        "description": "Format configuration discovered from the codebase",
                        "properties": {
                            "extensions": { "type": "array", "items": { "type": "string" }, "description": "File extensions to format (e.g. ['rs'])" },
                            "command": { "type": "string", "description": "Format command (e.g. 'cargo fmt')" }
                        }
                    },
                    "testing": {
                        "type": "object",
                        "description": "Testing configuration discovered from the codebase",
                        "properties": {
                            "companion_pattern": { "type": "string", "description": "Test companion file pattern (e.g. 'tests/{name}.rs')" },
                            "test_dir": { "type": "string", "description": "Test directory (e.g. 'tests')" }
                        }
                    },
                    "reference_repos": {
                        "type": "array",
                        "description": "GitHub repos that are good references for this project's stack/patterns",
                        "items": { "type": "string" }
                    },
                    "generated_files": {
                        "type": "array",
                        "description": "AI-generated file contents that override static scaffold templates. Generate full, ready-to-write content for each file.",
                        "items": {
                            "type": "object",
                            "properties": {
                                "path": { "type": "string", "description": "Relative path from project root (e.g. 'CLAUDE.md', 'ROADMAP.md', '.claude/rules/conventions.md')" },
                                "content": { "type": "string", "description": "Complete file content" }
                            },
                            "required": ["path", "content"]
                        }
                    },
                    "lifecycle_hooks": {
                        "type": "array",
                        "description": "Lifecycle hooks to trigger at workflow events. Each hook is either an MCP tool invocation or a shell command.",
                        "items": {
                            "type": "object",
                            "properties": {
                                "event": { "type": "string", "description": "Lifecycle event (e.g. 'on_enter(verify)', 'on_gate_pass', 'on_iteration_start')" },
                                "tool": { "type": "string", "description": "MCP tool name (mutually exclusive with 'command')" },
                                "command": { "type": "string", "description": "Shell command to run (mutually exclusive with 'tool')" },
                                "timeout_secs": { "type": "integer", "description": "Timeout for shell commands (seconds)" },
                                "description": { "type": "string", "description": "Description shown in agent system prompt" },
                                "on_failure": { "type": "string", "enum": ["continue", "fail_fast", "warn"], "description": "Failure strategy (default: continue)" }
                            },
                            "required": ["event"]
                        }
                    },
                    "confidence": {
                        "type": "object",
                        "description": "Confidence report: how confident are you in these recommendations and which are backed by encyclopedia knowledge vs. web research.",
                        "properties": {
                            "overall": { "type": "string", "enum": ["high", "medium", "low"], "description": "Overall confidence level. High = grounded in encyclopedia + verified by research. Medium = mostly grounded. Low = research-only." },
                            "rationale": { "type": "string", "description": "Brief explanation of the confidence assessment" },
                            "encyclopedia_backed": { "type": "array", "items": { "type": "string" }, "description": "Which recommendations are backed by magistr's built-in encyclopedia (e.g. 'quality_gate', 'conventions', 'testing')" },
                            "research_backed": { "type": "array", "items": { "type": "string" }, "description": "Which recommendations were discovered through web research (e.g. 'mcp_servers', 'reference_repos')" }
                        }
                    }
                }
            }),
            |recs| ToolOutput::Recommend(Box::new(recs)),
        )
    }

    /// `magistr_ask` — ask the user a structured question.
    fn ask_tool() -> MagistrTool {
        MagistrTool::typed::<Question, _>(
            "magistr_ask",
            "Ask the user a question and wait for their response. Use this \
             to present options discovered during research so the user can choose. \
             Each choice should have a description explaining pros/cons. \
             ALWAYS set `category` to identify which project area this question covers. \
             Use rich choices (with label + description) for all multi-option questions.",
            json!({
                "type": "object",
                "properties": {
                    "question": {
                        "type": "string",
                        "description": "The question to ask the user"
                    },
                    "category": {
                        "type": "string",
                        "description": "Project category this question covers. One of: code_quality, testing, error_handling, ci_cd, documentation, dependencies, architecture, logging, mcp_servers, agents, security",
                        "enum": ["code_quality", "testing", "error_handling", "ci_cd", "documentation", "dependencies", "architecture", "logging", "mcp_servers", "agents", "security"]
                    },
                    "icon": {
                        "type": "string",
                        "description": "Optional emoji icon override for the category (e.g. 🧪, 🔄). Auto-derived from category if omitted."
                    },
                    "choices": {
                        "type": "array",
                        "description": "Options for the user to choose from. Use rich objects with label + description for presenting paradigm-level choices with pros/cons.",
                        "items": {
                            "oneOf": [
                                { "type": "string" },
                                {
                                    "type": "object",
                                    "properties": {
                                        "label": { "type": "string", "description": "Display name of the option" },
                                        "description": { "type": "string", "description": "Explanation, pros/cons, or details about this option" }
                                    },
                                    "required": ["label"]
                                }
                            ]
                        }
                    },
                    "allow_freeform": {
                        "type": "boolean",
                        "description": "Whether to allow freeform text input (default: true)",
                        "default": true
                    },
                    "multi_select": {
                        "type": "boolean",
                        "description": "Whether the user can select multiple choices (checkboxes). When true, user toggles selections and confirms. Default: false (single-select radio buttons).",
                        "default": false
                    }
                },
                "required": ["question", "category"]
            }),
            ToolOutput::Ask,
        )
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ── Helpers ────────────────────────────────────────────────────────────

/// Normalize a tool name by stripping MCP server prefixes.
///
/// Delegates to `crate::tools::normalize` — the single source of truth.
fn normalize_tool_name(name: &str) -> std::borrow::Cow<'_, str> {
    crate::tools::normalize::normalize_tool_name(name)
}

// ── Tests ──────────────────────────────────────────────────────────────

#[cfg(test)]
#[path = "registry.tests.rs"]
mod tests;
