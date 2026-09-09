//! When a session started, as the header's session stop spells it.

/// Read the session start time once at the entry point for the header.
/// The cockpit receives the timestamp as data and never reads a clock.
#[must_use]
pub fn session_started_label() -> String {
    chrono::Local::now().format("%H:%M").to_string()
}

#[cfg(test)]
#[path = "session_started.tests.rs"]
mod tests;
