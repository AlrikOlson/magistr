//! Structured ANSI output helpers.
//!
//! Replaces raw `\x1b[` escape sequences scattered across the codebase with
//! a consistent API. All helpers respect the `NO_COLOR` environment variable.
//!
//! Some helpers are not yet wired into callers but exist as the public API
//! for future command output.

use std::fmt;
use std::sync::OnceLock;

/// Whether color output is enabled (cached on first call).
static COLOR_ENABLED: OnceLock<bool> = OnceLock::new();

/// Returns `true` if color output is enabled.
///
/// Respects `NO_COLOR` (any non-empty value disables color) per <https://no-color.org/>.
#[must_use]
pub(crate) fn is_color_enabled() -> bool {
    *COLOR_ENABLED.get_or_init(|| {
        std::env::var("NO_COLOR")
            .map(|v| v.is_empty())
            .unwrap_or(true)
    })
}

// ── Colored message formatters ────────────────────────────────────

/// Format a success message (green ✓ prefix).
#[must_use]
pub(crate) fn success(msg: &str) -> String {
    if is_color_enabled() {
        format!("\x1b[32m✓\x1b[0m {msg}")
    } else {
        format!("OK {msg}")
    }
}

/// Format an error message (red ✗ prefix).
#[must_use]
pub(crate) fn error(msg: &str) -> String {
    if is_color_enabled() {
        format!("\x1b[31m✗\x1b[0m {msg}")
    } else {
        format!("ERROR {msg}")
    }
}

/// Format a warning message (yellow ⚠ prefix).
#[must_use]
pub(crate) fn warning(msg: &str) -> String {
    if is_color_enabled() {
        format!("\x1b[33m⚠\x1b[0m {msg}")
    } else {
        format!("WARN {msg}")
    }
}

/// Format an info message (cyan → prefix).
#[must_use]
pub(crate) fn info(msg: &str) -> String {
    if is_color_enabled() {
        format!("\x1b[36m→\x1b[0m {msg}")
    } else {
        format!("INFO {msg}")
    }
}

/// Format text as dim (muted, lower contrast).
#[must_use]
pub(crate) fn dim(msg: &str) -> String {
    if is_color_enabled() {
        format!("\x1b[2m{msg}\x1b[0m")
    } else {
        msg.to_string()
    }
}

/// Format text as bold.
#[must_use]
pub(crate) fn bold(msg: &str) -> String {
    if is_color_enabled() {
        format!("\x1b[1m{msg}\x1b[0m")
    } else {
        msg.to_string()
    }
}

// ── Raw color wrappers ────────────────────────────────────────────

/// Wrap text in green.
#[must_use]
pub(crate) fn green(msg: &str) -> String {
    if is_color_enabled() {
        format!("\x1b[32m{msg}\x1b[0m")
    } else {
        msg.to_string()
    }
}

/// Wrap text in red.
#[must_use]
pub(crate) fn red(msg: &str) -> String {
    if is_color_enabled() {
        format!("\x1b[31m{msg}\x1b[0m")
    } else {
        msg.to_string()
    }
}

/// Wrap text in yellow.
#[must_use]
pub(crate) fn yellow(msg: &str) -> String {
    if is_color_enabled() {
        format!("\x1b[33m{msg}\x1b[0m")
    } else {
        msg.to_string()
    }
}

/// Wrap text in cyan.
#[must_use]
pub(crate) fn cyan(msg: &str) -> String {
    if is_color_enabled() {
        format!("\x1b[36m{msg}\x1b[0m")
    } else {
        msg.to_string()
    }
}

// ── Status icons ──────────────────────────────────────────────────

/// Green checkmark icon.
#[must_use]
pub(crate) fn icon_pass() -> &'static str {
    if is_color_enabled() {
        "\x1b[32m✓\x1b[0m"
    } else {
        "OK"
    }
}

/// Red cross icon.
#[must_use]
pub(crate) fn icon_fail() -> &'static str {
    if is_color_enabled() {
        "\x1b[31m✗\x1b[0m"
    } else {
        "FAIL"
    }
}

/// Yellow warning icon.
#[must_use]
#[allow(dead_code)] // kept for future command output
pub(crate) fn icon_warn() -> &'static str {
    if is_color_enabled() {
        "\x1b[33m⚠\x1b[0m"
    } else {
        "WARN"
    }
}

// ── Structural formatting ─────────────────────────────────────────

/// Format a section header (bold + underline).
#[must_use]
#[allow(dead_code)] // kept for future command output
pub(crate) fn header(title: &str) -> String {
    if is_color_enabled() {
        format!("\x1b[1;4m{title}\x1b[0m")
    } else {
        format!("=== {title} ===")
    }
}

/// A horizontal separator line.
#[must_use]
#[allow(dead_code)] // kept for future command output
pub(crate) fn separator() -> &'static str {
    "────────────────────────────────────────"
}

/// Format a key-value pair for display.
#[must_use]
pub(crate) fn kv(key: &str, value: &impl fmt::Display) -> String {
    if is_color_enabled() {
        format!("\x1b[1m{key}:\x1b[0m {value}")
    } else {
        format!("{key}: {value}")
    }
}

#[cfg(test)]
#[path = "format.tests.rs"]
mod tests;
