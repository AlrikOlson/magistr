//! How the runtime is configured.

use std::time::Duration;

use tokio_util::sync::CancellationToken;

use crate::graphics::Env;

/// What the runtime needs to know before it takes the terminal over: the
/// environment the graphics mode is decided from, how long the startup
/// window waits for the terminal's replies, the external quit signal the
/// loop observes beside its input, and — for the harness only — a frame
/// on which the view is made to panic.
#[derive(Clone, Debug)]
pub struct Options {
    env: Env,
    probe_timeout: Duration,
    panic_on_frame: Option<u64>,
    quit: Option<CancellationToken>,
}

impl Options {
    /// How long a terminal gets to answer DA1 before the cockpit assumes
    /// it never will (every real terminal answers within milliseconds; a
    /// slow SSH link is the reason for the margin).
    pub const DEFAULT_PROBE_TIMEOUT: Duration = Duration::from_secs(2);

    /// Options for `env` with the default probe timeout and no external
    /// quit signal.
    #[must_use]
    pub const fn new(env: Env) -> Self {
        Self {
            env,
            probe_timeout: Self::DEFAULT_PROBE_TIMEOUT,
            panic_on_frame: None,
            quit: None,
        }
    }

    /// Options from the process environment.
    #[must_use]
    pub fn from_os() -> Self {
        Self::new(Env::from_os())
    }

    /// Wait at most `timeout` for the startup replies.
    #[must_use]
    pub const fn probe_timeout(mut self, timeout: Duration) -> Self {
        self.probe_timeout = timeout;
        self
    }

    /// Fault injection for the harness: panic in place of the view on
    /// frame `frame` (frames count from 1), so a test can prove what a
    /// crashed cockpit leaves behind.
    #[must_use]
    pub const fn panic_on_frame(mut self, frame: u64) -> Self {
        self.panic_on_frame = Some(frame);
        self
    }

    /// End the run when `quit` fires — the host's process-wide shutdown
    /// request, so an external interrupt closes the cockpit the way a
    /// closed input does: the loop stops, the core is never asked, and
    /// the session leaves through its own sequence. A token already fired
    /// when the run starts ends it on the loop's first turn.
    #[must_use]
    pub fn quit_on(mut self, quit: CancellationToken) -> Self {
        self.quit = Some(quit);
        self
    }

    /// The environment the graphics mode is decided from.
    #[must_use]
    pub const fn env(&self) -> &Env {
        &self.env
    }

    /// The frame the view is made to panic on, if any.
    #[must_use]
    pub const fn panic_frame(&self) -> Option<u64> {
        self.panic_on_frame
    }

    /// The startup window's length.
    #[must_use]
    pub const fn probe_timeout_duration(&self) -> Duration {
        self.probe_timeout
    }

    /// The external quit signal, if the host armed one.
    #[must_use]
    pub const fn quit_signal(&self) -> Option<&CancellationToken> {
        self.quit.as_ref()
    }
}
