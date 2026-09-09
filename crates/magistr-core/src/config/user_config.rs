//! User-level configuration stored in `~/.config/magistr/config.toml`.
//!
//! This module handles loading and saving user preferences like default theme.
//! Follows XDG conventions: uses `$XDG_CONFIG_HOME/magistr/config.toml` if set,
//! otherwise `~/.config/magistr/config.toml`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// User-level configuration persisted across sessions.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct UserConfig {
    /// Theme preferences.
    #[serde(default)]
    pub theme: ThemeConfig,

    /// Default budget settings for sessions.
    #[serde(default)]
    pub budget: BudgetDefaults,

    /// Layout preferences for wide terminal mode.
    #[serde(default)]
    pub layout: LayoutConfig,

    /// Appearance preferences (easing, border style).
    #[serde(default)]
    pub appearance: AppearanceConfig,

    /// Input controls (keybinding preset, sound).
    #[serde(default)]
    pub controls: ControlsConfig,

    /// Accessibility preferences.
    #[serde(default)]
    pub accessibility: AccessibilityDefaults,

    /// Default model preferences for agent sessions.
    #[serde(default)]
    pub models: ModelDefaults,

    /// Session defaults (backend selection).
    #[serde(default)]
    pub session: SessionDefaults,

    /// General preferences (project name, loop defaults, visual intensity).
    #[serde(default)]
    pub general: GeneralConfig,
}

/// Theme-related user preferences.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct ThemeConfig {
    /// Default theme name (e.g. "catppuccin-mocha", "dracula").
    pub default: Option<String>,
}

/// Default budget settings persisted across sessions.
///
/// These values are used when `--budget` / `--budget-warn` CLI flags are not provided.
/// CLI flags always take precedence over config file defaults.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct BudgetDefaults {
    /// Default hard spending limit in USD (e.g., 5.0 for $5.00).
    pub limit: Option<f64>,
    /// Default warning threshold in USD (e.g., 3.0 for $3.00).
    pub warn_threshold: Option<f64>,
}

/// Layout preferences for wide terminal mode (200+ cols).
///
/// Controls the multi-panel grid arrangement when the terminal is wide enough
/// for the Wide layout mode. The `grid` string defines column arrangement.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct LayoutConfig {
    /// Grid layout specification string.
    ///
    /// Columns separated by `|`, vertical stacks in `[brackets]`.
    /// Example: `"sidebar | stream | [stats, commits]"`
    ///
    /// When `None`, uses the default three-column layout: `"sidebar | stream | detail"`.
    pub grid: Option<String>,
}

/// Appearance preferences beyond theme selection.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct AppearanceConfig {
    /// Animation easing profile: `"default"`, `"spring"`, or `"smooth"`.
    ///
    /// Controls the interpolation curves used by tachyonfx effects.
    /// When `None`, uses `"default"`.
    pub easing: Option<String>,

    /// Border style preset: `"rounded"`, `"sharp"`, `"double"`, `"thick"`.
    ///
    /// Controls box-drawing character set for all bordered elements.
    /// When `None`, uses `"rounded"`.
    pub border_style: Option<String>,

    /// Color depth override: `"auto"`, `"truecolor"`, `"256"`, `"16"`.
    ///
    /// Overrides auto-detected terminal color capabilities.
    /// When `None`, uses `"auto"` (detect from environment).
    pub color_depth: Option<String>,

    /// Animation speed multiplier: `"0.5x"`, `"1x"`, `"1.5x"`, `"2x"`.
    ///
    /// Scales all animation durations. Lower = faster animations.
    /// When `None`, uses `"1x"`.
    pub animation_speed: Option<String>,

    /// Information density preset: `"compact"`, `"default"`, `"spacious"`.
    ///
    /// Controls chrome height, sidebar spacing, and card margins.
    /// When `None`, uses `"default"`.
    pub density: Option<String>,
}

impl AppearanceConfig {
    /// Get the configured border style, or `"rounded"`.
    #[must_use]
    pub fn border_style_or_default(&self) -> &str {
        self.border_style.as_deref().unwrap_or("rounded")
    }

    /// Get the configured color depth, or `"auto"`.
    #[must_use]
    pub fn color_depth_or_default(&self) -> &str {
        self.color_depth.as_deref().unwrap_or("auto")
    }

    /// Get the configured animation speed, or `"1x"`.
    #[must_use]
    pub fn animation_speed_or_default(&self) -> &str {
        self.animation_speed.as_deref().unwrap_or("1x")
    }

    /// Get the configured density preset, or `"default"`.
    #[must_use]
    pub fn density_or_default(&self) -> &str {
        self.density.as_deref().unwrap_or("default")
    }
}

/// Input control preferences.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct ControlsConfig {
    /// Keybinding preset: `"default"`, `"vim"`, or `"emacs"`.
    ///
    /// When `None`, uses `"default"`.
    pub keybinding_preset: Option<String>,

    /// Whether sound effects (terminal bell) are enabled.
    ///
    /// When `None`, sound is disabled (opt-in via `--sound` flag).
    pub sound: Option<bool>,

    /// Per-action keybinding overrides on top of the active preset.
    ///
    /// Keys are action names (e.g. `"Scroll Up"`), values are key strings
    /// (e.g. `"Ctrl+K"`). When `None` or empty, all bindings come from the preset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub binding_overrides: Option<BTreeMap<String, String>>,
}

/// Accessibility preferences persisted across sessions.
///
/// CLI flags always take precedence over config file defaults.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct AccessibilityDefaults {
    /// Disable animations (`reduce_motion`).
    pub reduce_motion: Option<bool>,

    /// Force high-contrast theme adaptation.
    pub high_contrast: Option<bool>,

    /// Color Vision Deficiency mode: `"deuteranopia"`, `"protanopia"`, `"tritanopia"`.
    ///
    /// When `None`, no CVD adaptation is applied.
    pub cvd_mode: Option<String>,

    /// Skip scroll animations (instant jump).
    pub instant_scroll: Option<bool>,

    /// Enable screen reader mode (live region announcements via stderr).
    ///
    /// When `None`, screen reader mode is disabled.
    pub screen_reader: Option<bool>,

    /// Focus auto-return timeout in seconds (0 = disabled).
    ///
    /// After this many seconds of inactivity, focus returns to the main
    /// content panel. When `None`, defaults to 0 (disabled).
    pub focus_timeout: Option<u32>,
}

/// Default model preferences for agent sessions.
///
/// Per-provider model defaults — each agent backend can have its own preferred
/// model. CLI flags always take precedence over config file defaults.
///
/// # Example TOML
///
/// ```toml
/// [models]
/// claude = "claude-sonnet-4"
/// copilot = "gpt-4o"
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct ModelDefaults {
    /// Default model for Claude Code (the in-binary `claude` provider, API key or CLI login).
    ///
    /// When `None`, the Claude backend chooses its own default.
    pub claude: Option<String>,

    /// Default model for GitHub Copilot (`copilot --acp`).
    ///
    /// When `None`, the Copilot backend chooses its own default.
    pub copilot: Option<String>,
}

impl ModelDefaults {
    /// Look up the default model for a given provider ID.
    ///
    /// Provider IDs match [`AgentRuntime::provider_id()`]: `"claude"` or `"copilot"`.
    /// Returns `None` when no override is set or the value is `"default"` (meaning
    /// "let the provider choose").
    #[must_use]
    pub fn for_provider(&self, provider_id: &str) -> Option<&str> {
        let value = match provider_id {
            "claude" => self.claude.as_deref(),
            "copilot" => self.copilot.as_deref(),
            _ => None,
        };
        // "default" is a sentinel meaning "no override"
        value.filter(|v| !v.eq_ignore_ascii_case("default"))
    }

    /// Set the default model for a given provider ID.
    ///
    /// Passing `None` clears the default (provider picks its own).
    pub fn set_for_provider(&mut self, provider_id: &str, model: Option<String>) {
        match provider_id {
            "claude" => self.claude = model,
            "copilot" => self.copilot = model,
            _ => {}
        }
    }
}

/// Session defaults persisted across sessions.
///
/// Controls which agent provider is preferred when multiple are available.
/// Both Claude Code and GitHub Copilot run over ACP (Agent Client Protocol).
/// CLI flags and `MAGISTR_AGENT_BACKEND` env var always take precedence.
///
/// # Example TOML
///
/// ```toml
/// [session]
/// default_provider = "claude"
/// isolation_mode = "sandbox"
/// permission_mode = "plan"
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct SessionDefaults {
    /// Preferred agent provider: `"auto"`, `"claude"`, `"copilot"`, `"codex"`, or one
    /// of the chat-only API providers magistr's own agent serves —
    /// `"openai"`, `"gemini"`, `"ollama"`.
    ///
    /// When `None` or `"auto"`, magistr auto-detects the best available provider
    /// (Claude Code with API key > Claude Code with CLI login > Copilot). Codex and
    /// API providers are never auto-detected: name one here to use it.
    pub default_provider: Option<String>,

    /// Override for agent binary path (e.g. `/usr/local/bin/copilot`).
    ///
    /// When `None`, magistr auto-detects the binary via `which`.
    pub binary_path_override: Option<String>,

    /// Agent isolation mode: `"sandbox"`, `"augmented"`, or `"inherit"`.
    ///
    /// - `"sandbox"` (default) — agent CWD is an isolated sandbox directory.
    /// - `"augmented"` — sandbox + project CLAUDE.md merged into sandbox.
    /// - `"inherit"` — no sandbox, agent runs in project directory directly.
    pub isolation_mode: Option<String>,

    /// Permission mode for the agent: `"plan"`, `"auto-edit"`, etc.
    ///
    /// Passed as `--permission-mode` to the ACP agent.
    /// When `None`, no permission mode flag is set.
    pub permission_mode: Option<String>,

    /// Text appended to the agent's system prompt in the sandbox CLAUDE.md.
    ///
    /// When `None`, only the default sandbox instructions are used.
    pub system_prompt_append: Option<String>,

    /// Additional `--add-dir` paths for agent access beyond the project directory.
    ///
    /// Each entry is an absolute path that the agent can read/write.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extra_add_dirs: Option<Vec<String>>,

    /// Extra MCP servers to include in the agent's sandbox configuration.
    ///
    /// Each entry defines a server name, command, arguments, and environment.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extra_mcp_servers: Option<Vec<McpServerEntry>>,

    /// Auto-approve file read operations without prompting.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auto_approve_read: Option<bool>,

    /// Auto-approve file edit/write operations without prompting.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auto_approve_edit: Option<bool>,

    /// Auto-approve shell command execution without prompting.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auto_approve_bash: Option<bool>,
}

/// A persisted MCP server entry for agent session configuration.
///
/// Serialized to TOML in `[session]` and to JSON for the Tauri GUI settings.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct McpServerEntry {
    /// Server name (used as the key in `mcpServers` config).
    pub name: String,
    /// Command to start the server.
    pub command: String,
    /// Command-line arguments.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub args: Vec<String>,
    /// Environment variables for the server process.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub env: BTreeMap<String, String>,
}

impl SessionDefaults {
    /// Get the configured default provider, if any.
    ///
    /// Returns `None` when unset or `"auto"` (meaning auto-detect).
    #[must_use]
    pub fn provider(&self) -> Option<&str> {
        self.default_provider
            .as_deref()
            .filter(|v| !v.eq_ignore_ascii_case("auto") && !v.is_empty())
    }

    /// Get the configured isolation mode, or `"sandbox"`.
    #[must_use]
    pub fn isolation_mode_or_default(&self) -> &str {
        self.isolation_mode.as_deref().unwrap_or("sandbox")
    }

    /// Get the configured permission mode, if any.
    #[must_use]
    pub fn permission_mode(&self) -> Option<&str> {
        self.permission_mode.as_deref().filter(|v| !v.is_empty())
    }
}

/// General preferences for the Settings > General section.
///
/// These settings control project display, agent prompt defaults,
/// loop behavior, and visual intensity.
///
/// # Example TOML
///
/// ```toml
/// [general]
/// project_name = "my-project"
/// max_iterations = 15
/// auto_pause_on_error = true
/// visual_intensity = "balanced"
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct GeneralConfig {
    /// Override project display name (defaults to directory name if `None`).
    pub project_name: Option<String>,

    /// Default agent prompt template prepended to user messages.
    pub prompt_template: Option<String>,

    /// Maximum loop iterations before auto-stop (default: 10).
    pub max_iterations: Option<u32>,

    /// Whether to auto-pause the loop on agent errors (default: true).
    pub auto_pause_on_error: Option<bool>,

    /// Visual intensity level: `"minimal"`, `"balanced"`, or `"full"`.
    ///
    /// Controls how many ambient visual effects (particles, glows, etc.) are active.
    /// When `None`, defaults to `"balanced"`.
    pub visual_intensity: Option<String>,

    /// Delay in milliseconds before auto-advancing past the inter-iteration
    /// transition summary card (default: 2000). Set to 0 to skip the card entirely.
    pub transition_delay_ms: Option<u64>,

    /// Whether sound effects are enabled (default: false, opt-in).
    pub sound_effects: Option<bool>,

    /// Sound effect volume (0.0–1.0, default: 0.5).
    pub sound_volume: Option<f64>,

    /// Send native notification on session/workflow completion (default: true).
    pub notifications_completions: Option<bool>,

    /// Send native notification on gate failure (default: true).
    pub notifications_failures: Option<bool>,

    /// Send native notification on permission request (default: true).
    pub notifications_permissions: Option<bool>,
}

impl GeneralConfig {
    /// Default max iterations when not configured.
    pub const DEFAULT_MAX_ITERATIONS: u32 = 10;

    /// Default auto-pause-on-error when not configured.
    pub const DEFAULT_AUTO_PAUSE: bool = true;

    /// Get the configured max iterations, or the default.
    #[must_use]
    pub fn max_iterations_or_default(&self) -> u32 {
        self.max_iterations.unwrap_or(Self::DEFAULT_MAX_ITERATIONS)
    }

    /// Get the configured auto-pause setting, or the default.
    #[must_use]
    pub fn auto_pause_or_default(&self) -> bool {
        self.auto_pause_on_error.unwrap_or(Self::DEFAULT_AUTO_PAUSE)
    }

    /// Get the configured visual intensity, or `"balanced"`.
    #[must_use]
    pub fn visual_intensity_or_default(&self) -> &str {
        self.visual_intensity.as_deref().unwrap_or("balanced")
    }

    /// Default transition delay in milliseconds.
    pub const DEFAULT_TRANSITION_DELAY_MS: u64 = 2000;

    /// Get the configured transition delay, or the default (2000ms).
    #[must_use]
    pub fn transition_delay_or_default(&self) -> u64 {
        self.transition_delay_ms
            .unwrap_or(Self::DEFAULT_TRANSITION_DELAY_MS)
    }

    /// Whether sound effects are enabled (default: false).
    #[must_use]
    pub fn sound_effects_enabled(&self) -> bool {
        self.sound_effects.unwrap_or(false)
    }

    /// Sound volume 0.0–1.0 (default: 0.5).
    #[must_use]
    pub fn sound_volume_or_default(&self) -> f64 {
        self.sound_volume.unwrap_or(0.5)
    }

    /// Whether completion notifications are enabled (default: true).
    #[must_use]
    pub fn notify_completions(&self) -> bool {
        self.notifications_completions.unwrap_or(true)
    }

    /// Whether failure notifications are enabled (default: true).
    #[must_use]
    pub fn notify_failures(&self) -> bool {
        self.notifications_failures.unwrap_or(true)
    }

    /// Whether permission request notifications are enabled (default: true).
    #[must_use]
    pub fn notify_permissions(&self) -> bool {
        self.notifications_permissions.unwrap_or(true)
    }
}

/// Resolve the user config file path.
///
/// Uses `$XDG_CONFIG_HOME/magistr/config.toml` if set, otherwise `~/.config/magistr/config.toml`.
pub fn config_path() -> Option<PathBuf> {
    config_dir().map(|dir| dir.join("config.toml"))
}

/// Resolve the user config directory.
fn config_dir() -> Option<PathBuf> {
    if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
        return Some(PathBuf::from(xdg).join("magistr"));
    }
    std::env::var("HOME")
        .ok()
        .map(|home| PathBuf::from(home).join(".config/magistr"))
}

/// Load user configuration from the default path.
///
/// Returns `UserConfig::default()` if the file does not exist.
/// Returns an error only if the file exists but is invalid TOML.
pub fn load() -> Result<UserConfig, UserConfigError> {
    let path = match config_path() {
        Some(p) => p,
        None => return Ok(UserConfig::default()),
    };
    load_from(&path)
}

/// Load user configuration from a specific path (for testing).
pub fn load_from(path: &Path) -> Result<UserConfig, UserConfigError> {
    if !path.is_file() {
        return Ok(UserConfig::default());
    }

    let content =
        std::fs::read_to_string(path).map_err(|e| UserConfigError::Io(path.to_owned(), e))?;
    let config: UserConfig = toml::from_str(&content)
        .map_err(|e| UserConfigError::Parse(path.to_owned(), e.to_string()))?;
    Ok(config)
}

/// Save user configuration to the default path.
///
/// Creates the parent directory if it does not exist.
pub fn save(config: &UserConfig) -> Result<(), UserConfigError> {
    let path = config_path()
        .ok_or_else(|| UserConfigError::NoHome("cannot determine config directory".into()))?;
    save_to(config, &path)
}

/// Save user configuration to a specific path (for testing).
pub fn save_to(config: &UserConfig, path: &Path) -> Result<(), UserConfigError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| UserConfigError::Io(parent.to_owned(), e))?;
    }

    let content =
        toml::to_string_pretty(config).map_err(|e| UserConfigError::Serialize(e.to_string()))?;
    std::fs::write(path, content).map_err(|e| UserConfigError::Io(path.to_owned(), e))?;
    Ok(())
}

/// Errors that can occur when loading or saving user configuration.
#[derive(Debug)]
pub enum UserConfigError {
    /// File I/O error.
    Io(PathBuf, std::io::Error),
    /// TOML parse error.
    Parse(PathBuf, String),
    /// TOML serialization error.
    Serialize(String),
    /// Home directory not found.
    NoHome(String),
}

impl std::fmt::Display for UserConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(path, e) => write!(f, "config I/O error at '{}': {}", path.display(), e),
            Self::Parse(path, e) => {
                write!(f, "failed to parse config '{}': {}", path.display(), e)
            }
            Self::Serialize(e) => write!(f, "failed to serialize config: {e}"),
            Self::NoHome(msg) => write!(f, "{msg}"),
        }
    }
}

impl std::error::Error for UserConfigError {}

#[cfg(test)]
#[path = "user_config.tests.rs"]
mod tests;
