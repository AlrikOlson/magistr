//! Centralized environment configuration for magistr.
//!
//! `MagistrEnv` captures all `MAGISTR_*` / `REDUCE_MOTION` / `NO_ANIMATIONS` environment variables
//! once at startup.  Every subsystem receives `&MagistrEnv` instead of calling
//! `std::env::var()` directly — eliminating scattered reads, making tests
//! deterministic (construct `MagistrEnv` directly, zero `set_var`), and
//! providing a single source of truth for runtime configuration.
//!
//! **Flow:**
//! ```text
//! CLI flags → main.rs propagate_env_flags()   (sets env for subprocesses)
//!           → MagistrEnv::from_process_env()       (snapshot into struct)
//!           → &MagistrEnv threaded through app
//! ```

/// Snapshot of all `MAGISTR_*` environment variables.
///
/// Constructed once via [`MagistrEnv::from_process_env()`], then passed by
/// reference through the application.  Tests use [`MagistrEnv::default()`]
/// and override individual fields.
#[derive(Debug, Clone)]
pub struct MagistrEnv {
    // ── Agent / Session ──────────────────────────────────────────────
    pub agent_backend: Option<String>,
    pub model: Option<String>,
    pub continue_session: bool,

    // ── Theme ────────────────────────────────────────────────────────
    pub theme: Option<String>,
    pub theme_bg: Option<String>,
    pub high_contrast: bool,
    pub cvd: Option<String>,
    pub nerd_fonts: Option<String>,
    pub icons: Option<String>,

    // ── Layout ───────────────────────────────────────────────────────
    pub layout: Option<String>,
    pub density: Option<String>,

    // ── Visual ────────────────────────────────────────────────────────
    pub visual_intensity: Option<String>,
    pub perf: bool,

    // ── Accessibility ────────────────────────────────────────────────
    pub reduce_motion: bool,
    pub no_animations: bool,
    pub no_mouse: bool,
    pub accessible: bool,
    pub typewriter: bool,
    pub keys: Option<String>,
    pub scroll: Option<String>,
    pub scroll_accel: Option<String>,
    pub no_tips: bool,
    pub stderr_status: bool,

    // ── Sound ────────────────────────────────────────────────────────
    pub sound: Option<String>,
    pub bell: Option<String>,
    pub bell_throttle: Option<String>,

    // ── Budget ───────────────────────────────────────────────────────
    pub budget: Option<f64>,
    pub budget_warn: Option<f64>,

    // ── Workflow ──────────────────────────────────────────────────────
    pub no_autoformat: bool,
    pub max_iterations: Option<u32>,
    pub export_summary: bool,
    pub export_dir: Option<String>,

    // ── Terminal ──────────────────────────────────────────────────────
    pub mux_mode: Option<String>,
    pub toasts: Option<String>,

    // ── Logging ──────────────────────────────────────────────────────
    pub log_level: Option<String>,

    // ── External environment variables ─────────────────────
    pub term_program: Option<String>,
}

impl MagistrEnv {
    /// Read all `MAGISTR_*` environment variables from the current process.
    ///
    /// Call this **once** in `main()`, after `propagate_env_flags()` has
    /// applied CLI overrides to the environment.
    pub fn from_process_env() -> Self {
        Self {
            agent_backend: var_opt("MAGISTR_AGENT_BACKEND"),
            model: var_opt("MAGISTR_MODEL"),
            continue_session: var_bool("MAGISTR_CONTINUE"),

            theme: var_opt("MAGISTR_THEME"),
            theme_bg: var_opt("MAGISTR_THEME_BG"),
            high_contrast: var_bool("MAGISTR_HIGH_CONTRAST"),
            cvd: var_opt("MAGISTR_CVD"),
            nerd_fonts: var_opt("MAGISTR_NERD_FONTS"),
            icons: var_opt("MAGISTR_ICONS"),

            layout: var_opt("MAGISTR_LAYOUT"),
            density: var_opt("MAGISTR_DENSITY"),

            visual_intensity: var_opt("MAGISTR_VISUAL_INTENSITY"),
            perf: var_bool("MAGISTR_PERF"),

            reduce_motion: var_bool("REDUCE_MOTION"),
            no_animations: var_bool("NO_ANIMATIONS"),
            no_mouse: var_bool("MAGISTR_NO_MOUSE"),
            accessible: var_bool("MAGISTR_ACCESSIBLE"),
            typewriter: var_bool("MAGISTR_TYPEWRITER"),
            keys: var_opt("MAGISTR_KEYS"),
            scroll: var_opt("MAGISTR_SCROLL"),
            scroll_accel: var_opt("MAGISTR_SCROLL_ACCEL"),
            no_tips: var_bool("MAGISTR_NO_TIPS"),
            stderr_status: var_bool("MAGISTR_STDERR_STATUS"),

            sound: var_opt("MAGISTR_SOUND"),
            bell: var_opt("MAGISTR_BELL"),
            bell_throttle: var_opt("MAGISTR_BELL_THROTTLE"),

            budget: var_opt("MAGISTR_BUDGET").and_then(|v| v.parse().ok()),
            budget_warn: var_opt("MAGISTR_BUDGET_WARN").and_then(|v| v.parse().ok()),

            no_autoformat: var_bool("MAGISTR_NO_AUTOFORMAT"),
            max_iterations: var_opt("MAGISTR_MAX_ITERATIONS").and_then(|v| v.parse().ok()),
            export_summary: var_bool("MAGISTR_EXPORT_SUMMARY"),
            export_dir: var_opt("MAGISTR_EXPORT_DIR"),

            mux_mode: var_opt("MAGISTR_MUX_MODE"),
            toasts: var_opt("MAGISTR_TOASTS"),

            log_level: var_opt("MAGISTR_LOG"),

            term_program: var_opt("TERM_PROGRAM"),
        }
    }

    /// Whether instant-scroll mode is active (MAGISTR_SCROLL=instant, case-insensitive).
    pub fn instant_scroll(&self) -> bool {
        self.scroll
            .as_deref()
            .is_some_and(|v| v.eq_ignore_ascii_case("instant"))
    }
}

impl Default for MagistrEnv {
    /// Sensible defaults for testing — everything off / unset.
    fn default() -> Self {
        Self {
            agent_backend: None,
            model: None,
            continue_session: false,

            theme: None,
            theme_bg: None,
            high_contrast: false,
            cvd: None,
            nerd_fonts: None,
            icons: None,

            layout: None,
            density: None,

            visual_intensity: None,
            perf: false,

            reduce_motion: false,
            no_animations: false,
            no_mouse: false,
            accessible: false,
            typewriter: false,
            keys: None,
            scroll: None,
            scroll_accel: None,
            no_tips: false,
            stderr_status: false,

            sound: None,
            bell: None,
            bell_throttle: None,

            budget: None,
            budget_warn: None,

            no_autoformat: false,
            max_iterations: None,
            export_summary: false,
            export_dir: None,

            mux_mode: None,
            toasts: None,

            log_level: None,

            term_program: None,
        }
    }
}

/// Read an env var, returning `Some(value)` if set and non-empty.
fn var_opt(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|v| !v.is_empty())
}

/// Read a boolean env var (`"1"` = true, anything else = false).
fn var_bool(name: &str) -> bool {
    std::env::var(name).map(|v| v == "1").unwrap_or(false)
}
