use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;
use tracing::warn;

#[derive(Debug, Parser)]
#[command(
    name = "magistr",
    version,
    about = "Manage coding agents and project workflow"
)]
pub struct Cli {
    /// Output machine-readable JSON
    #[arg(long, global = true)]
    pub json: bool,

    /// Hard spending limit in USD — session stops when exceeded (e.g., --budget 5.00)
    #[arg(long, global = true, env = "MAGISTR_BUDGET")]
    pub budget: Option<f64>,

    /// Warning threshold in USD — warn when reached (e.g., --budget-warn 3.00)
    #[arg(long, global = true, env = "MAGISTR_BUDGET_WARN")]
    pub budget_warn: Option<f64>,

    /// Permission profile: cautious (ask everything), standard (auto-allow reads),
    /// trusted (auto-allow most), yolo (auto-allow all)
    #[arg(long, global = true, env = "MAGISTR_TRUST")]
    pub trust: Option<String>,

    /// Enable debug logging to stderr. Accepts: debug, info, warn, error.
    /// Can also be set via MAGISTR_LOG env var with full tracing filter syntax.
    #[arg(long, short = 'v', global = true)]
    pub verbose: Option<String>,

    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Scaffold project configuration for coding agents with an auto-detected stack
    Init {
        /// Project directory (defaults to current directory)
        #[arg(default_value = ".")]
        dir: PathBuf,

        /// Select one or more agents; repeat or separate with commas.
        #[arg(long = "agent", value_delimiter = ',', value_parser = ["codex", "claude", "portable", "cursor", "copilot", "cline", "windsurf", "aider"], conflicts_with_all = ["ci", "check"])]
        agents: Vec<String>,

        /// Accept every interview default without prompting (CI-safe, no tty needed)
        #[arg(long, short = 'y', conflicts_with_all = ["dry_run", "ci", "check"])]
        yes: bool,

        /// Reinitialize in place, preserving project config and state (CI mode overwrites its generated file)
        #[arg(long, short = 'f')]
        force: bool,

        /// Preview scaffold without writing files — show what would be created/changed
        #[arg(long, short = 'n', conflicts_with = "force")]
        dry_run: bool,

        /// Initialize from a preset file or URL (.magistr-preset.toml).
        /// Accepts a local path or http(s):// URL.
        #[arg(long, conflicts_with_all = ["dry_run"])]
        from: Option<String>,

        /// Generate CI workflow file (.github/workflows/quality-gates.yml) from detected stack
        #[arg(long, conflicts_with_all = ["dry_run", "from"])]
        ci: bool,

        /// Compare current filesystem state against existing config and report drift
        #[arg(long, conflicts_with_all = ["force", "dry_run", "from", "ci"])]
        check: bool,
    },

    /// Print detected stack as JSON (no file changes)
    Detect {
        /// Project directory (defaults to current directory)
        #[arg(default_value = ".")]
        dir: PathBuf,

        /// Show confidence scores per detection category
        #[arg(long, short = 's')]
        smart: bool,

        /// Detect all projects in a repo and show as a tree
        #[arg(long, short = 't')]
        tree: bool,
    },

    /// Review and repair selected agent setup, preserving customizations
    Repair {
        #[arg(default_value = ".")]
        dir: PathBuf,
        #[arg(long)]
        dry_run: bool,
        #[arg(long, short = 'y')]
        yes: bool,
    },

    /// Remove generated agent setup while keeping project settings and work
    Remove {
        #[arg(default_value = ".")]
        dir: PathBuf,
        #[arg(long)]
        dry_run: bool,
        #[arg(long, short = 'y')]
        yes: bool,
    },

    /// Remove Magistr project configuration and state after confirmation
    Reset {
        /// Project directory (defaults to current directory)
        #[arg(default_value = ".")]
        dir: PathBuf,
    },

    /// Run standard and required custom quality gates from .magistr/project.json
    Validate {
        /// Project directory (defaults to current directory)
        #[arg(default_value = ".")]
        dir: PathBuf,

        /// CI mode: compact output and proper exit codes for pipelines
        #[arg(long)]
        ci: bool,

        /// Emit GitHub Actions annotations (::error, ::warning) for gate failures
        #[arg(long)]
        annotations: bool,
    },

    /// Launch an agent session with project context
    Run {
        /// The prompt to send to the agent
        prompt: String,

        /// Resume a saved session ID from magistr sessions
        #[arg(long)]
        resume: Option<String>,

        /// Continue from the most recent conversation
        #[arg(long, short = 'c')]
        r#continue: bool,

        #[command(flatten)]
        agent: crate::workflow_agent_options::WorkflowAgentOptions,

        /// Agent backend to use
        #[arg(long, short = 'b', default_value = "auto")]
        backend: BackendFlag,

        /// Stream events to stdout without interactive prompts
        #[arg(long)]
        headless: bool,

        /// Output format for headless mode (plain or json)
        #[arg(long, default_value = "plain", requires = "headless")]
        format: HeadlessFormat,
    },

    /// Implement the next spec items (research-plan-execute-verify cycle)
    #[group(id = "headless_output", args = ["headless", "gate_only"], multiple = true)]
    Next {
        #[command(flatten)]
        agent: crate::workflow_agent_options::WorkflowAgentOptions,
        /// Project directory (defaults to current directory)
        #[arg(default_value = ".")]
        dir: PathBuf,

        /// Continue the selected unfinished objective and task plan
        #[arg(long, short = 'c', conflicts_with = "headless")]
        r#continue: bool,

        /// Agent backend to use
        #[arg(long, short = 'b', default_value = "auto")]
        backend: BackendFlag,

        /// Stream events to stdout without interactive prompts
        #[arg(long)]
        headless: bool,

        /// Output format for headless mode (plain or json)
        #[arg(long, default_value = "plain", requires = "headless_output")]
        format: HeadlessFormat,

        /// Run quality gates only, without an AI iteration
        #[arg(long, conflicts_with_all = ["continue", "agent", "model", "effort", "show_config"])]
        gate_only: bool,
    },

    /// Run ready roadmap work in fresh agent sessions
    NextLoop {
        /// Loop only on this roadmap track (exact name or unambiguous fragment)
        #[arg(long)]
        track: Option<String>,
        #[command(flatten)]
        agent: crate::workflow_agent_options::WorkflowAgentOptions,
        /// Project directory (defaults to current directory)
        #[arg(default_value = ".")]
        dir: PathBuf,

        /// Maximum number of iterations (default: 10)
        #[arg(long, default_value = "10")]
        max_iterations: u32,

        /// Agent backend to use
        #[arg(long, short = 'b', default_value = "auto")]
        backend: BackendFlag,

        /// Stream events to stdout without interactive prompts
        #[arg(long)]
        headless: bool,

        /// Output format for headless mode (plain or json)
        #[arg(long, default_value = "plain", requires = "headless")]
        format: HeadlessFormat,

        /// Maximum cost in USD for headless mode (e.g., --max-cost 5.00)
        #[arg(long, requires = "headless")]
        max_cost: Option<f64>,
    },

    /// Record what happened to an interrupted agent attempt
    ResolveDispatch {
        /// Project directory
        #[arg(default_value = ".")]
        dir: PathBuf,
        /// Saved run ID shown by workspace doctor
        #[arg(long)]
        run: String,
        /// Command ID shown by workspace doctor
        #[arg(long)]
        command: String,
        /// Attempt number shown by workspace doctor
        #[arg(long)]
        attempt: std::num::NonZeroU32,
        /// What happened: completed work is not retried; failed or cancelled work can be retried
        #[arg(long, value_parser = ["completed", "failed", "cancelled"])]
        outcome: String,
        /// Explain how you checked the work and its outcome
        #[arg(long)]
        reason: String,
        /// Confirm the loop and every agent it started have stopped
        #[arg(long, required = true)]
        confirm_stopped: bool,
    },

    /// Generate shell completions for bash, zsh, or fish
    Completions {
        /// Shell to generate completions for
        #[arg(value_enum)]
        shell: clap_complete::Shell,

        /// Auto-install completions to the appropriate shell config directory
        #[arg(long)]
        install: bool,
    },

    /// Generate man pages to a directory
    ManPages {
        /// Output directory for man pages
        #[arg(default_value = ".")]
        dir: PathBuf,
    },

    /// Manage configuration: validate, show effective config, migrate
    #[command(subcommand_required = false)]
    Config {
        /// Config action (validates configuration when omitted)
        #[command(subcommand)]
        action: Option<ConfigAction>,
    },

    /// Validate project config against known anti-patterns
    Lint {
        /// Project directory (defaults to current directory)
        #[arg(default_value = ".")]
        dir: PathBuf,

        /// Also show stack-based improvement suggestions
        #[arg(long, short = 's')]
        suggest: bool,
    },

    /// List configured archetypes in a polyglot project
    Archetypes {
        /// Project directory (defaults to current directory)
        #[arg(default_value = ".")]
        dir: PathBuf,

        /// Show full config dump for each archetype
        #[arg(long)]
        detail: bool,

        /// Output machine-readable JSON
        #[arg(long)]
        json: bool,
    },

    /// Diagnose why the agent might be ignoring your rules
    Doctor {
        /// Project directory (defaults to current directory)
        #[arg(default_value = ".")]
        dir: PathBuf,
    },

    /// Claude Code hook guard: reads the hook JSON on stdin and the ship/roadmap
    /// state, and exits 2 with the reason on stderr to block the event
    Guard {
        /// Which hook is asking: `stop` (end of turn) or `commit` (PreToolUse on Bash)
        event: GuardEvent,

        /// Project directory (defaults to current directory)
        #[arg(default_value = ".")]
        dir: PathBuf,
    },

    /// List current and retained work plans
    Plans {
        /// Project directory (defaults to current directory)
        #[arg(default_value = ".")]
        dir: PathBuf,

        /// Filter by status: active, completed, abandoned
        #[arg(long, short = 's')]
        status: Option<PlanStatusFilter>,

        /// Maximum number of plans to show
        #[arg(long, short = 'n', default_value = "20")]
        limit: usize,
    },

    /// Show, generate or export the project roadmap
    #[command(subcommand_required = false)]
    Roadmap {
        /// Roadmap action
        #[command(subcommand)]
        action: Option<RoadmapAction>,
    },

    /// Export current project config as a shareable preset (.magistr-preset.toml)
    Export {
        /// Project directory (defaults to current directory)
        #[arg(default_value = ".")]
        dir: PathBuf,

        /// Output file path (defaults to .magistr-preset.toml in project dir)
        #[arg(long, short = 'o')]
        output: Option<PathBuf>,
    },

    /// Show cost history across sessions
    Costs {
        /// What to report and how.
        #[command(flatten)]
        query: CostQuery,
    },

    /// Import a shareable preset and scaffold .claude/ from it.
    /// Accepts a local path or http(s):// URL.
    Import {
        /// Path or URL to a .magistr-preset.toml file
        source: String,

        /// Project directory (defaults to current directory)
        #[arg(default_value = ".")]
        dir: PathBuf,

        /// Reinitialize in place, preserving project config and state
        #[arg(long, short = 'f')]
        force: bool,
    },

    /// Browse and manage past agent sessions
    Sessions {
        /// Filter by project name (substring match)
        #[arg(long, short = 'p')]
        project: Option<String>,

        /// Show only sessions from the last N days
        #[arg(long)]
        since_days: Option<u32>,

        /// Maximum number of sessions to display
        #[arg(long, short = 'n', default_value = "20")]
        limit: usize,

        /// Output format: table (default), json
        #[arg(long, default_value = "table")]
        format: SessionFormat,

        /// Tag a session: --tag <session-id> <tag>
        #[arg(long, num_args = 2, value_names = ["SESSION_ID", "TAG"])]
        tag: Option<Vec<String>>,

        /// Purge sessions older than N days (default: 30)
        #[arg(long)]
        purge: Option<u32>,

        /// Filter by owner identity (substring match)
        #[arg(long)]
        owner: Option<String>,

        /// Hand a session to a new owner: --handoff <session-id> <new-owner>
        #[arg(long, num_args = 2, value_names = ["SESSION_ID", "NEW_OWNER"])]
        handoff: Option<Vec<String>>,
    },

    /// Sign in to the team's identity provider (OAuth device flow)
    Login {
        /// Provider kind: github or oidc (defaults to the roster's)
        #[arg(long, short = 'p')]
        provider: Option<String>,

        /// OIDC issuer URL — discovery appends /.well-known/openid-configuration
        #[arg(long)]
        issuer: Option<String>,

        /// The registered OAuth client id
        #[arg(long)]
        client_id: Option<String>,

        /// Scopes to request (defaults to the provider's minimum)
        #[arg(long)]
        scope: Option<String>,

        /// List the cached logins and their expiry instead of signing in
        #[arg(long)]
        status: bool,

        /// Project directory (defaults to current directory)
        #[arg(default_value = ".")]
        dir: PathBuf,
    },

    /// Clear cached verified identities
    Logout {
        /// Provider key to clear (defaults to every provider)
        #[arg(long, short = 'p')]
        provider: Option<String>,
    },

    /// Manage the team roster and roles (.magistr/team.json)
    #[command(subcommand_required = false)]
    Team {
        /// Team action (defaults to show)
        #[command(subcommand)]
        action: Option<TeamAction>,

        /// Project directory (defaults to current directory)
        #[arg(default_value = ".")]
        dir: PathBuf,
    },

    /// Inspect the org's workflow policy (.magistr/team.json `policy`)
    #[command(subcommand_required = false)]
    Policy {
        /// Policy action (defaults to show)
        #[command(subcommand)]
        action: Option<PolicyAction>,

        /// Project directory (defaults to current directory)
        #[arg(default_value = ".")]
        dir: PathBuf,
    },

    /// View permission audit log
    Permissions {
        /// Maximum number of entries to display
        #[arg(long, short = 'n', default_value = "50")]
        limit: usize,

        /// Filter by project name
        #[arg(long, short = 'p')]
        project: Option<String>,

        /// Filter by actor (substring match on the recorded identity)
        #[arg(long, short = 'a')]
        actor: Option<String>,

        /// Export every matching entry to stdout (the -n limit does not apply)
        #[arg(long, value_enum)]
        export: Option<ExportFormat>,
    },

    /// List configured AI models with availability status
    Models,

    /// Analyze project structure and build persistent memory
    Learn {
        /// Project directory (defaults to current directory)
        #[arg(default_value = ".")]
        dir: PathBuf,
    },

    /// Manage installed plugins (custom detection and scaffold templates)
    #[command(subcommand_required = false)]
    Plugins {
        /// Plugin action (defaults to list)
        #[command(subcommand)]
        action: Option<PluginAction>,
    },

    /// Manage user-authored preset fragments (create, list, test, validate)
    #[command(subcommand_required = false)]
    Preset {
        /// Preset action (defaults to list)
        #[command(subcommand)]
        action: Option<PresetAction>,

        /// Project directory (defaults to current directory)
        #[arg(default_value = ".")]
        dir: PathBuf,
    },

    /// Proxy an MCP stdio server, filtering non-JSON from stdout.
    /// Wraps MCP servers whose stdout contains log lines that corrupt
    /// the JSON-RPC transport.
    #[command(hide = true)]
    McpProxy {
        /// The command and arguments to run (everything after --)
        #[arg(trailing_var_arg = true, required = true)]
        cmd: Vec<String>,
    },

    /// Connect coding agents to Magistr's project and source tools over stdio.
    Mcp {
        /// Limit this connection to planning or inspection; native permissions still apply.
        #[arg(long, value_enum, default_value = "full")]
        access: crate::unified_mcp::Access,
        /// Project directory
        #[arg(long, default_value = ".")]
        dir: PathBuf,
        /// Workspace configuration for a newly admitted owner (repeatable key=value)
        #[arg(long = "config", value_name = "KEY=VALUE")]
        config: Vec<String>,
    },

    /// Launch Codex with this project's managed skill selection.
    CodexLaunch {
        /// Project directory containing the Codex setup
        #[arg(long, default_value = ".")]
        project_root: PathBuf,
        /// Report runtime and skill-discovery compatibility without starting a session
        #[arg(long)]
        diagnose: bool,
        /// Arguments forwarded literally to Codex after --
        #[arg(last = true)]
        args: Vec<std::ffi::OsString>,
    },

    /// Remove unchanged managed Codex setup while preserving project and workflow state.
    CodexRemove {
        /// Project directory containing the Codex setup
        #[arg(default_value = ".")]
        dir: PathBuf,

        /// Preview the removal plan without writing — every path with its
        /// action, ownership and reason (JSON with --json)
        #[arg(long, short = 'n')]
        dry_run: bool,
    },

    /// Serve magistr's own ACP agent on stdio for the provider named — the
    /// agent side of the bridge the cockpit speaks as a client. The runtime
    /// spawns this binary with it, so no agent lands on PATH.
    #[command(hide = true)]
    AcpAgent {
        /// The provider that drives the model (`claude`: the Claude Code CLI on
        /// PATH; `openai`, `gemini`, `ollama`: those HTTP APIs, chat only;
        /// `scripted`: no model, a smoke agent)
        #[arg(long)]
        provider: String,

        /// A tool the session may run without asking (repeatable; the same
        /// `--allow-tool` every agent runtime is launched with)
        #[arg(long = "allow-tool")]
        allow_tool: Vec<String>,
    },

    /// View and manage what magistr has learned about your project
    Memory {
        /// Project directory (defaults to current directory)
        #[arg(default_value = ".")]
        dir: PathBuf,

        /// Memory action
        #[command(subcommand)]
        action: Option<MemoryAction>,
    },

    /// The native workspace tools — search, map, read, edit, files, undo,
    /// script, run and workspace_status — served over MCP for one workspace
    Workspace {
        #[command(subcommand)]
        action: WorkspaceAction,
    },
}

/// Actions for the `magistr workspace` subcommand.
#[derive(Debug, Subcommand)]
pub enum WorkspaceAction {
    /// Start, inspect, reopen or archive separate task working copies
    WorkingCopy {
        /// Project directory
        #[arg(long, default_value = ".")]
        dir: PathBuf,
        #[command(subcommand)]
        action: crate::working_copy_action::WorkingCopyAction,
    },
    /// Read-only health report over the .workspace artifacts (environment,
    /// index, config provenance, plugins, quarantine, crashes), the live
    /// owner (one bounded probe: generation, build, attached connections)
    /// and the run checkpoint's operation settlement. No lock is taken, so
    /// it runs beside a live server. Exit 0 healthy, 1 findings.
    Doctor {
        #[command(subcommand)]
        mode: Option<WorkspaceDoctorMode>,

        /// Workspace root (defaults to the current directory)
        #[arg(default_value = ".")]
        dir: PathBuf,

        /// Session-layer configuration for the report (repeatable key=value)
        #[arg(long = "config", value_name = "KEY=VALUE")]
        config: Vec<String>,
    },

    /// The write journal — the rows `undo action:"list"` returns — read
    /// beside a live server without taking the workspace lock.
    Journal {
        /// Workspace root (defaults to the current directory)
        #[arg(default_value = ".")]
        dir: PathBuf,

        /// Ops to list, newest first (1..=200)
        #[arg(long, default_value_t = 20, value_name = "N")]
        limit: usize,

        /// Only ops with seq >= SEQ
        #[arg(long, value_name = "SEQ")]
        since: Option<u64>,

        /// Only ops with seq < SEQ (the pagination cursor)
        #[arg(long, value_name = "SEQ")]
        before: Option<u64>,

        /// Only ops journaled by this session
        #[arg(long, value_name = "ID")]
        session: Option<String>,

        /// Keep the counts, drop the diff bodies
        #[arg(long)]
        no_diff: bool,

        /// Print the envelope `undo action:"list"` returns instead of text
        #[arg(long)]
        json: bool,
    },

    /// Per-workspace MRTR confirm-token key management.
    Key {
        #[command(subcommand)]
        action: WorkspaceKeyAction,
    },

    /// Seed run's `[commands]` allowlist into .magistr/workspace.toml from
    /// lockfile/manifest detection; an existing table is never touched.
    Init {
        /// Workspace root (defaults to the current directory)
        #[arg(default_value = ".")]
        dir: PathBuf,
    },

    /// The source certification observation — canonical root, durable owner
    /// generation, selection policy and content manifest — as the serialized
    /// SourceSnapshot, read beside a live server without taking any lock.
    Snapshot {
        /// Workspace root (defaults to the current directory)
        #[arg(default_value = ".")]
        dir: PathBuf,

        /// Explicit artifact input to hash even when ignored (repeatable,
        /// workspace-relative)
        #[arg(long = "artifact", value_name = "PATH")]
        artifact: Vec<String>,
    },
}

/// The two non-report forms of `magistr workspace doctor`.
#[derive(Debug, Subcommand)]
pub enum WorkspaceDoctorMode {
    /// Pack the same probes into a redacted tar.gz — structure and
    /// symptoms, never repo file contents. Exit 0 healthy, 1 findings.
    Bundle {
        /// Workspace root (defaults to the current directory)
        #[arg(default_value = ".")]
        dir: PathBuf,

        /// Archive path (default: workspace-doctor-bundle-<timestamp>.tar.gz
        /// in the current directory); an existing file is never overwritten
        #[arg(long, value_name = "PATH")]
        out: Option<PathBuf>,

        /// Session-layer configuration for the report (repeatable key=value)
        #[arg(long = "config", value_name = "KEY=VALUE")]
        config: Vec<String>,
    },

    /// Print the five-layer resolved configuration with per-key provenance.
    Config {
        /// Workspace root (defaults to the current directory)
        #[arg(default_value = ".")]
        dir: PathBuf,

        /// Session-layer configuration to resolve (repeatable key=value)
        #[arg(long = "config", value_name = "KEY=VALUE")]
        config: Vec<String>,
    },
}

/// Actions for `magistr workspace key`.
#[derive(Debug, Subcommand)]
pub enum WorkspaceKeyAction {
    /// Replace the workspace key; every outstanding confirm token is voided.
    Rotate {
        /// Workspace root (defaults to the current directory)
        #[arg(default_value = ".")]
        dir: PathBuf,
    },
}

/// Actions for the `magistr memory` subcommand.
#[derive(Debug, Subcommand)]
pub enum MemoryAction {
    /// Show current project memory (default if no action specified)
    Show,

    /// Add an observation to project memory
    Add {
        /// The observation text to add
        text: String,

        /// Category: patterns, architecture, conventions, mistakes, observations
        #[arg(long, short = 'c', default_value = "observations")]
        category: String,
    },

    /// Reset project memory
    Reset,
}

/// Audit export formats for `magistr permissions --export`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum ExportFormat {
    /// A JSON array of audit entries.
    Json,
    /// RFC 4180 CSV with a header row.
    Csv,
}

/// Actions for the `magistr team` subcommand.
#[derive(Debug, Subcommand)]
pub enum TeamAction {
    /// Show your identity, resolved role, and the roster (default)
    Show,

    /// Create .magistr/team.json with you as the sole Owner
    Init,

    /// Add a member to the roster
    Add {
        /// Member id — an email or username
        id: String,

        /// Role: owner, admin, developer, viewer, ci
        #[arg(long, short = 'r', default_value = "developer")]
        role: String,

        /// Display name
        #[arg(long, short = 'n')]
        name: Option<String>,
    },

    /// Remove a member from the roster
    Remove {
        /// Member id — an email or username
        id: String,
    },

    /// Change a member's role
    SetRole {
        /// Member id — an email or username
        id: String,

        /// Role: owner, admin, developer, viewer, ci
        role: String,
    },
}

/// Actions for the `magistr policy` subcommand.
#[derive(Subcommand, Debug, Clone)]
pub enum PolicyAction {
    /// Show the guardrails this project runs under (default)
    Show,

    /// Ask the policy what it would do about one act
    Check {
        /// Tool name, e.g. Bash or mcp__magistr__gate
        #[arg(long, short = 't')]
        tool: Option<String>,

        /// Shell command the tool would run
        #[arg(long, short = 'c')]
        command: Option<String>,

        /// File path the act would touch (repeatable)
        #[arg(long, short = 'f')]
        path: Vec<String>,

        /// Model the session would run on
        #[arg(long, short = 'm')]
        model: Option<String>,
    },

    /// Compliance report: what the guardrails actually caught
    Report {
        /// Only count entries from this actor (substring match)
        #[arg(long, short = 'a')]
        actor: Option<String>,
    },
}

/// Actions for the `magistr roadmap` subcommand.
#[derive(Debug, Subcommand)]
pub enum RoadmapAction {
    /// Show current roadmap progress (default if no action specified)
    Show {
        /// Project directory (defaults to current directory)
        #[arg(default_value = ".")]
        dir: PathBuf,
    },

    /// Generate a new roadmap from a high-level goal
    Generate {
        /// What you want to build (e.g. "add REST API with auth")
        goal: String,

        /// Project directory (defaults to current directory)
        #[arg(default_value = ".")]
        dir: PathBuf,

        /// Agent backend to use
        #[arg(long, short = 'b', default_value = "auto")]
        backend: BackendFlag,
    },

    /// Show the next ready chunk (magistr roadmap backend)
    Next {
        /// Project directory (defaults to current directory)
        #[arg(default_value = ".")]
        dir: PathBuf,
    },

    /// Roadmap status snapshot — counts, regions, next ready chunk (magistr backend; `--json` for the raw roadmap_status)
    Status {
        /// Project directory (defaults to current directory)
        #[arg(default_value = ".")]
        dir: PathBuf,
    },

    /// Regenerate ROADMAP.md from the roadmap store (magistr export, or the YAML source)
    Export {
        /// Project directory (defaults to current directory)
        #[arg(default_value = ".")]
        dir: PathBuf,
    },
}

/// Actions for the `magistr plugins` subcommand.
#[derive(Debug, Subcommand)]
pub enum PluginAction {
    /// List installed plugins (default if no action specified)
    List,

    /// Install a plugin from a local directory
    Add {
        /// Path to the plugin directory (must contain magistr-plugin.toml)
        path: PathBuf,
    },

    /// Remove an installed plugin by name
    Remove {
        /// Plugin name to remove
        name: String,
    },

    /// Install a plugin from the registry by name
    Install {
        /// Plugin name to install (looked up in the registry index)
        name: String,

        /// Custom registry index URL (defaults to official index)
        #[arg(long)]
        index: Option<String>,

        /// Ed25519 public key (base64) for signature verification
        #[arg(long)]
        public_key: Option<String>,
    },

    /// Search the plugin registry for available plugins
    Search {
        /// Search query (matches against plugin names and descriptions)
        query: Option<String>,

        /// Custom registry index URL (defaults to official index)
        #[arg(long)]
        index: Option<String>,
    },

    /// Validate a plugin directory (check manifest, detect.toml, templates)
    Validate {
        /// Path to the plugin directory
        path: PathBuf,
    },
}

/// Actions for the `magistr preset` subcommand.
#[derive(Debug, Subcommand)]
pub enum PresetAction {
    /// List all available presets (built-in + project-local + user-global)
    List,

    /// Create a new preset fragment interactively
    Create {
        /// Output file path (defaults to .magistr/presets/<name>.toml)
        #[arg(long, short = 'o')]
        output: Option<PathBuf>,
    },

    /// Validate a preset by checking schema and dry-running gate commands
    Test {
        /// Path to the preset file to test
        file: PathBuf,
    },
}

/// Actions for the `magistr config` subcommand.
#[derive(Debug, Subcommand)]
pub enum ConfigAction {
    /// Save an agent, model, or effort preference
    Set {
        /// Setting to save: agent, model, or effort
        key: String,
        /// Value to save; default clears a model or effort override
        value: String,
        /// Save for this project instead of your personal settings
        #[arg(long)]
        project: bool,
        /// Agent whose model or effort to save (otherwise uses your selected agent)
        #[arg(long)]
        agent: Option<String>,
        /// Project directory
        #[arg(long, default_value = ".")]
        dir: PathBuf,
    },
    /// Validate all configuration files (project.json, user config, theme TOML)
    Check {
        /// Project directory (defaults to current directory)
        #[arg(default_value = ".")]
        dir: PathBuf,
    },

    /// Print effective configuration (all sources merged, defaults applied)
    Show {
        /// Project directory (defaults to current directory)
        #[arg(default_value = ".")]
        dir: PathBuf,
    },
}

/// CLI value for the `--layout` flag.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum LayoutFlag {
    /// Stream panel on top, sidebar as bottom drawer
    Vertical,
    /// Sidebar beside stream panel (default for wide terminals)
    Horizontal,
    /// Auto-detect based on terminal aspect ratio
    Auto,
}

/// Output format for `magistr run --headless`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Default)]
pub enum HeadlessFormat {
    /// Human-readable plain text (default)
    #[default]
    Plain,
    /// NDJSON — one JSON envelope per event
    Json,
}

/// Output format for the `magistr costs` command.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum CostFormat {
    /// Human-readable table (default)
    Table,
    /// Comma-separated values
    Csv,
    /// Machine-readable JSON
    Json,
}

/// Everything `magistr costs` needs to decide what to report.
///
/// Flattened into the subcommand rather than listed inline so the
/// command handler takes one argument instead of nine.
#[derive(Debug, Clone, clap::Args)]
pub struct CostQuery {
    /// Filter by project name (substring match)
    #[arg(long, short = 'p')]
    pub project: Option<String>,

    /// Show only entries from the last N days
    #[arg(long)]
    pub since_days: Option<u32>,

    /// Maximum number of entries to display
    #[arg(long, short = 'n', default_value = "20")]
    pub limit: usize,

    /// Output format: table (default), csv, json
    #[arg(long, default_value = "table")]
    pub format: CostFormat,

    /// Show time-bucketed summary: daily, weekly, or monthly
    #[arg(long)]
    pub summary: Option<CostSummaryPeriod>,

    /// Meter spend by an attribution axis: project, user, model, or session
    #[arg(long, conflicts_with = "summary")]
    pub group_by: Option<CostGroupBy>,

    /// Chargeback report — spend per period per cost centre, with shares.
    /// Pair with --group-by to pick the cost centre (default: user) and
    /// --chargeback-period to pick the bucket (default: monthly).
    #[arg(long, conflicts_with = "summary")]
    pub chargeback: bool,

    /// Period bucket for --chargeback (default: monthly)
    #[arg(long, requires = "chargeback")]
    pub chargeback_period: Option<CostSummaryPeriod>,

    /// Show the budget policy hierarchy in force for a project directory
    ///
    /// Named `--policy` rather than `--budget` because the global
    /// `--budget <USD>` flag already owns that name.
    #[arg(long, num_args = 0..=1, default_missing_value = ".", value_name = "DIR")]
    pub policy: Option<PathBuf>,
}

/// Attribution axis for `magistr costs --group-by`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum CostGroupBy {
    /// One row per project
    Project,
    /// One row per team identity
    User,
    /// One row per model
    Model,
    /// One row per session
    Session,
}

/// Time period for cost summary bucketing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum CostSummaryPeriod {
    /// Group costs by day
    Daily,
    /// Group costs by week
    Weekly,
    /// Group costs by calendar month
    Monthly,
}

/// Output format for the `magistr sessions` command.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum SessionFormat {
    /// Human-readable table (default)
    Table,
    /// Machine-readable JSON
    Json,
}

/// Filter for plan status in `magistr plans`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum PlanStatusFilter {
    /// Show only active plans
    Active,
    /// Show only completed plans
    Completed,
    /// Show only abandoned plans
    Abandoned,
}

/// Agent backend selection for `magistr run`.
///
/// Can also be set via `MAGISTR_AGENT_BACKEND` environment variable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Default)]
pub enum BackendFlag {
    /// Auto-detect the best available backend (default)
    #[default]
    Auto,
    /// ACP (Agent Client Protocol) — formal lifecycle, graceful cancel
    #[value(alias = "claude-acp")]
    Acp,
}

impl BackendFlag {
    /// Resolve this flag to a concrete `Backend`.
    ///
    /// If `Auto`, the captured `MAGISTR_AGENT_BACKEND` request is honoured
    /// first, then `Backend::detect()` auto-detection. Explicit flags bypass
    /// the captured request.
    ///
    /// Returns an error if the requested backend is not available.
    pub fn resolve(
        self,
        inputs: &crate::config::inputs::ProjectInputs,
    ) -> Result<crate::agent::Backend, String> {
        match self {
            Self::Auto => {
                // Honour the captured MAGISTR_AGENT_BACKEND before auto-detection
                if let Some(backend) = inputs.agent_backend().and_then(Self::parse_backend_str) {
                    return backend.resolve(inputs);
                }
                crate::agent::detect_backend()
                    .ok_or_else(|| "No agent backend available. Install Claude Code (the `claude` command):\n  curl -fsSL https://claude.ai/install.sh | bash".to_string())
            }
            Self::Acp => Ok(crate::agent::Backend::Acp),
        }
    }

    /// Parse a backend name string into a `BackendFlag`.
    ///
    /// Trims whitespace, lowercases, and maps known values.
    /// Returns `None` for empty, `"auto"`, or unrecognized values.
    fn parse_backend_str(val: &str) -> Option<Self> {
        let val = val.trim().to_lowercase();
        if val.is_empty() {
            return None;
        }
        match val.as_str() {
            "acp" | "claude-acp" => Some(Self::Acp),
            "auto" => None, // Fall through to auto-detection
            _ => {
                warn!(
                    "Unrecognized MAGISTR_AGENT_BACKEND={val:?}, using auto-detection. \
                     Valid values: acp, claude-acp, auto"
                );
                None
            }
        }
    }
}

#[cfg(test)]
#[path = "cli.tests.rs"]
mod tests;

/// Which Claude Code hook `magistr guard` is answering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum GuardEvent {
    /// End of the assistant's turn (Stop hook): a shipped objective's own
    /// roadmap chunk must be closed; unrelated worktree changes are preserved.
    Stop,
    /// A Bash tool call (PreToolUse hook): `git commit` needs green, verified
    /// gates, and grep, rg, find or sed over the workspace is routed to
    /// magistr when the project declares it.
    Commit,
    /// A file-editing tool call (PreToolUse hook on Edit/Write/MultiEdit/
    /// NotebookEdit): a frozen tree (`.magistr/legacy-tui-ratchet.json`) is
    /// only writable under an unlocking ship objective.
    Write,
}

impl GuardEvent {
    /// The hook name as Claude Code spells it, for messages.
    #[must_use]
    pub fn hook_name(self) -> &'static str {
        match self {
            Self::Stop => "Stop",
            Self::Commit => "PreToolUse:Bash",
            Self::Write => "PreToolUse:Edit|Write",
        }
    }
}
