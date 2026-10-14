//! Runtime configuration, mirroring the original TypeScript `loadConfig` semantics.

use std::env;
use std::path::PathBuf;

use crate::infra::PersistenceConfig;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Console,
    Json,
    Markdown,
}

impl OutputFormat {
    pub fn as_str(self) -> &'static str {
        match self {
            OutputFormat::Console => "console",
            OutputFormat::Json => "json",
            OutputFormat::Markdown => "markdown",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value.to_ascii_lowercase().as_str() {
            "console" => Some(OutputFormat::Console),
            "json" => Some(OutputFormat::Json),
            "markdown" => Some(OutputFormat::Markdown),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ValidationConfig {
    pub require_thought_prefix: bool,
    pub require_rationale_prefix: bool,
    pub allow_custom_purpose: bool,
    pub strict_mode: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FeaturesConfig {
    pub enable_revisions: bool,
    pub enable_branching: bool,
    pub enable_confidence: bool,
    pub enable_structured_actions: bool,
    pub enable_sessions: bool,
    /// When set, the server fills in this `session_id` on any incoming
    /// step that doesn't carry one. Combined with the on-disk session
    /// file naming, this lets each MCP-server-process spawn show up as
    /// its own session in the viewer without the agent having to
    /// remember to pass a `session_id` on every call.
    ///
    /// Resolved in this order at startup:
    ///   1. `THINK_AND_SHIP_DEFAULT_SESSION_ID` env var (explicit override)
    ///   2. Enabled sessions (`THINK_AND_SHIP_ENABLE_SESSIONS=true` or the
    ///      alternate `THINK_AND_SHIP_AUTO_SESSION=true`) use the admitted
    ///      project identity, stable across server spawns.
    ///   3. None (caller must pass `session_id` explicitly to use a session).
    ///
    /// A valid explicit session ID also enables sessions. Pure configuration
    /// resolution leaves the automatic ID unset until project admission.
    pub default_session_id: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DisplayConfig {
    pub color_output: bool,
    pub output_format: OutputFormat,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SystemConfig {
    pub max_history_size: usize,
    pub max_branch_depth: u32,
    /// Session inactivity timeout, in minutes.
    pub session_timeout: u64,
    /// How many UNPINNED prior steps to include in each step response's
    /// `recent_steps` rollup. Higher values keep more orientation in-band at
    /// the cost of tokens. This has never capped pinned steps — see
    /// [`Self::rollup_max_items`], which caps the whole array.
    pub recent_steps_limit: usize,
    /// Hard CEILING on the total number of entries in a `recent_steps` rollup,
    /// pinned and unpinned together.
    ///
    /// Before this existed, `recent_steps_limit` capped only the unpinned tail
    /// while every pinned step was folded in unconditionally — so a trace with
    /// 62 pinned steps returned ~61 entries on a configured budget of 3, on the
    /// most-called tool in the server, growing monotonically.
    ///
    /// Clamped UP to [`ROLLUP_PINNED_FLOOR`] at use: this knob can make the
    /// rollup bigger, never small enough to lose the load-bearing conclusions
    /// the pinned mechanism exists to keep visible.
    pub rollup_max_items: usize,
}

/// The FLOOR of the two-sided rollup budget: the number of pinned steps that
/// survive no matter how [`SystemConfig::rollup_max_items`] is configured.
///
/// Deliberately a source constant rather than a second env var. The ceiling is
/// the cheap thing to satisfy by deleting the asset it was meant to bound — the
/// same trade f275f10 guarded against on the tools/list side — and a floor that
/// configuration can lower is not a floor.
///
/// Five: a trace tends to accumulate roughly two pinned steps per roadmap
/// chunk (a decision and a close), so five is the smallest window in which a
/// conclusion and the decision it rests on both still appear.
pub const ROLLUP_PINNED_FLOOR: usize = 5;

/// How far OUTSIDE the pinned budget a pin must fall before the maintenance
/// report will propose releasing it, counted in pinned steps.
///
/// A pin one slot past the budget is not stale — it is one flex of
/// [`SystemConfig::recent_steps_limit`] away from being carried again. The lag
/// is the margin that stops yesterday's candidate from being today's mistake.
///
/// A source constant beside [`ROLLUP_PINNED_FLOOR`] for the same reason a floor
/// is: a lag that configuration could set to zero is not a lag. Three is chosen
/// to be small and non-zero; unlike the floor it has no measurement behind it,
/// which is exactly why it is revised with a reason rather than tuned away.
pub const PIN_DECAY_LAG: usize = 3;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BroadcastConfig {
    /// When set, the server binds a Unix domain socket here and emits a
    /// newline-delimited JSON frame for every trace mutation. Passive
    /// observers (the desktop viewer, log scrapers) subscribe by
    /// connecting. Unset = no broadcast, no socket, no overhead.
    pub path: Option<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct ThinkConfig {
    pub validation: ValidationConfig,
    pub features: FeaturesConfig,
    pub display: DisplayConfig,
    pub system: SystemConfig,
    pub persistence: PersistenceConfig,
    pub broadcast: BroadcastConfig,
}

impl PartialEq for ThinkConfig {
    fn eq(&self, other: &Self) -> bool {
        self.validation == other.validation
            && self.features == other.features
            && self.display == other.display
            && self.system == other.system
            && self.persistence.enabled == other.persistence.enabled
            && self.persistence.data_dir == other.persistence.data_dir
            && self.broadcast == other.broadcast
    }
}

impl Eq for ThinkConfig {}

impl Default for ThinkConfig {
    fn default() -> Self {
        Self::with_persistence(PersistenceConfig::from_env().enabled(false))
    }
}

impl ThinkConfig {
    /// Defaults with explicit persistence; does not inspect the host environment.
    #[must_use]
    pub fn with_persistence(persistence: PersistenceConfig) -> Self {
        Self {
            validation: ValidationConfig {
                require_thought_prefix: false,
                require_rationale_prefix: false,
                allow_custom_purpose: true,
                strict_mode: false,
            },
            features: FeaturesConfig {
                enable_revisions: true,
                enable_branching: true,
                enable_confidence: true,
                enable_structured_actions: true,
                enable_sessions: false,
                default_session_id: None,
            },
            display: DisplayConfig {
                color_output: true,
                output_format: OutputFormat::Console,
            },
            system: SystemConfig {
                max_history_size: 100,
                max_branch_depth: 5,
                session_timeout: 60,
                recent_steps_limit: 3,
                rollup_max_items: 12,
            },
            persistence,
            broadcast: BroadcastConfig { path: None },
        }
    }
}

/// Compatibility host entry point. Native application owners use the same
/// parser with captured inputs and provide their admitted project identity.
pub fn load_config() -> ThinkConfig {
    let mut config = super::ThinkConfigResolution::from_lookup(
        |key| env::var(key).ok(),
        PersistenceConfig::from_env(),
    )
    .config;
    if config.features.enable_sessions && config.features.default_session_id.is_none() {
        config.features.default_session_id = Some(generate_auto_session_id());
    }
    config
}

/// Generate the default session_id at server startup. Format is just
/// the resolved project identifier — `<basename>-<6hex>` from the cwd
/// path, or a sanitized `THINK_AND_SHIP_PROJECT_NAME` override. NO timestamp,
/// NO random suffix.
///
/// The original design appended a timestamp+random suffix so each
/// server spawn got its own session. That fragmented every project
/// into N sessions over time and broke persistence from the agent's
/// point of view — a fresh conversation couldn't see its own past
/// reasoning. The corrected design uses one stable id per project,
/// loaded on every spawn, so the trace accumulates across all
/// conversations a user ever has in that project.
fn generate_auto_session_id() -> String {
    resolve_project_id()
}

/// Separator between the project id and a caller-supplied custom name.
/// Two underscores so it can't be confused with a single `_` inside an
/// otherwise-bare custom name (e.g. `my_session`).
pub const PROJECT_SEP: &str = "__";

/// Canonical project identifier for the current process. Delegates to
/// the shared engine so all tool families produce the same identity for
/// the same working directory.
pub fn resolve_project_id() -> String {
    crate::infra::resolve_project_id(None)
}

/// Rewrite a caller-supplied session id so it lands inside the current
/// project's namespace on disk. Idempotent: passing an already-prefixed
/// id (`<project>__<rest>`) returns the input unchanged. Persistence validates
/// the final identifier before selecting a file.
pub fn namespace_session_id(project_id: &str, raw: &str) -> String {
    let raw = raw.trim();
    // Empty / equal-to-project ⇒ the bare project session.
    if raw.is_empty() || raw == project_id {
        return project_id.to_string();
    }
    // Already namespaced under this project (idempotent).
    let sep_prefix = format!("{project_id}{PROJECT_SEP}");
    if raw.starts_with(&sep_prefix) {
        return raw.to_string();
    }
    // Otherwise prefix and re-sanitize. The combined id is clamped to
    // the safe-id length (128 chars) by truncating the suffix.
    let mut combined = sep_prefix;
    combined.push_str(raw);
    if combined.len() > 128 {
        combined.truncate(128);
        // Don't leave a dangling separator after truncation.
        while combined.ends_with('-') || combined.ends_with('.') {
            combined.pop();
        }
    }
    combined
}
