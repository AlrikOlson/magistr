//! What the environment says about the terminal before any byte is sent.

use magistr_cockpit::capability::Mode;

use super::passthrough::Passthrough;

/// The environment variables the probe and the mode decision read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Env {
    term: String,
    tmux: bool,
    tui_mode: Option<String>,
    progress: bool,
    underlines: bool,
    screen_reader: bool,
}

impl Env {
    /// The `TERM_PROGRAM` values whose terminals honour ConEmu's OSC 9;4
    /// progress sequence, lowercased. Every other terminal *prints* the
    /// string, which is why this is an allowlist and not a denylist —
    /// mise learned that the loud way in Oct 2025.
    const PROGRESS_PROGRAMS: [&'static str; 3] = ["ghostty", "wezterm", "iterm.app"];

    /// The `TERM` prefixes whose terminals render `SGR 4:x` styled
    /// underlines and `SGR 58` underline colours: kitty (which defined
    /// them), Ghostty, WezTerm, foot, Alacritty (since 0.12) and the
    /// VTE-based terminals, which report `vte` or the `xterm-256color`
    /// they set for compatibility — so VTE is reached by `TERM_PROGRAM`
    /// instead, never by that shared `TERM`.
    const UNDERLINE_TERMS: [&'static str; 5] = [
        "xterm-kitty",
        "xterm-ghostty",
        "wezterm",
        "foot",
        "alacritty",
    ];

    /// The `TERM_PROGRAM` values that vouch for the same, lowercased.
    const UNDERLINE_PROGRAMS: [&'static str; 3] = ["ghostty", "wezterm", "iterm.app"];

    /// An environment described explicitly (tests, harnesses).
    ///
    /// Progress reporting is **off**: a harness that wants the OSC 9;4
    /// bytes in its capture opts in with [`Env::with_progress`], so no
    /// golden and no byte-exact test moves by default.
    #[must_use]
    pub fn new(term: impl Into<String>, tmux: bool, tui_mode: Option<&str>) -> Self {
        let term = term.into();
        let underlines = Self::honours_underline_styles(&term, "");
        Self {
            term,
            tmux,
            tui_mode: tui_mode.map(str::to_owned),
            progress: false,
            underlines,
            screen_reader: false,
        }
    }

    /// Read `TERM`, `TMUX`, `MAGISTR_TUI_MODE` and
    /// [`Mode::SCREEN_READER_ENV`] from the process environment, plus the
    /// three variables that name a terminal with a progress indicator
    /// (`TERM_PROGRAM`, `WT_SESSION`, `ConEmuANSI`).
    /// tmux is detected by a non-empty `TMUX` or a `TERM` starting with
    /// `screen` or `tmux`.
    #[must_use]
    pub fn from_os() -> Self {
        Self::from_lookup(|name| std::env::var(name).ok())
    }

    /// Resolve terminal capabilities from an explicit environment.
    fn from_lookup(var: impl Fn(&str) -> Option<String>) -> Self {
        let term = var("TERM").unwrap_or_default();
        let tmux = var("TMUX").is_some_and(|v| !v.is_empty())
            || term.starts_with("screen")
            || term.starts_with("tmux");
        let tui_mode = var("MAGISTR_TUI_MODE");
        let term_program = var("TERM_PROGRAM").unwrap_or_default();
        let progress = Self::honours_progress(
            &term_program,
            var("WT_SESSION").is_some_and(|v| !v.is_empty()),
            var("ConEmuANSI").is_some_and(|v| v.eq_ignore_ascii_case("on")),
        );
        let underlines = Self::honours_underline_styles(&term, &term_program);
        let screen_reader = var(Mode::SCREEN_READER_ENV).is_some_and(|v| Self::asks(&v));
        Self {
            term,
            tmux,
            tui_mode,
            progress,
            underlines,
            screen_reader,
        }
    }

    /// Whether an environment variable's value asks for the thing it
    /// names: anything but empty, `0`, `false`, `no` or `off`.
    fn asks(value: &str) -> bool {
        let value = value.trim();
        !value.is_empty()
            && !["0", "false", "no", "off"]
                .iter()
                .any(|deny| value.eq_ignore_ascii_case(deny))
    }

    /// Whether a terminal identified this way renders styled and
    /// coloured underlines: its `TERM` starts with one of
    /// [`Env::UNDERLINE_TERMS`], or its `TERM_PROGRAM` is one of
    /// [`Env::UNDERLINE_PROGRAMS`].
    ///
    /// An allowlist, like the progress one and for the same reason: a
    /// terminal that does not parse `4:3` may print the `:3`, and the
    /// cost of guessing wrong is visible garbage rather than a missing
    /// flourish.
    #[must_use]
    pub fn honours_underline_styles(term: &str, term_program: &str) -> bool {
        Self::UNDERLINE_TERMS
            .iter()
            .any(|known| term.starts_with(known))
            || Self::UNDERLINE_PROGRAMS
                .iter()
                .any(|known| term_program.eq_ignore_ascii_case(known))
    }

    /// Whether a terminal identified this way shows an OSC 9;4 progress
    /// bar: Ghostty, WezTerm or iTerm2 by `TERM_PROGRAM`, Windows
    /// Terminal by a non-empty `WT_SESSION`, ConEmu by `ConEmuANSI=ON`.
    #[must_use]
    pub fn honours_progress(term_program: &str, windows_terminal: bool, conemu: bool) -> bool {
        windows_terminal
            || conemu
            || Self::PROGRESS_PROGRAMS
                .iter()
                .any(|known| term_program.eq_ignore_ascii_case(known))
    }

    /// Report progress to this environment's terminal (harnesses; the
    /// process environment decides for a real session).
    #[must_use]
    pub const fn with_progress(mut self, progress: bool) -> Self {
        self.progress = progress;
        self
    }

    /// Render this session flat, for a screen reader (harnesses; the
    /// process environment decides for a real session).
    #[must_use]
    pub const fn with_screen_reader(mut self, screen_reader: bool) -> Self {
        self.screen_reader = screen_reader;
        self
    }

    /// Whether this session renders as linear text for a screen reader
    /// ([`crate::flat`]) instead of as a cell cockpit: no alternate
    /// screen, no mouse, no pixels, no redraw of what has settled.
    #[must_use]
    pub const fn screen_reader(&self) -> bool {
        self.screen_reader
    }

    /// The `TERM` value.
    #[must_use]
    pub fn term(&self) -> &str {
        &self.term
    }

    /// Is tmux between us and the terminal?
    #[must_use]
    pub const fn tmux(&self) -> bool {
        self.tmux
    }

    /// Does `MAGISTR_TUI_MODE=text` force the plain-text cockpit?
    #[must_use]
    pub fn forces_text(&self) -> bool {
        self.tui_mode
            .as_deref()
            .is_some_and(|mode| mode.trim().eq_ignore_ascii_case("text"))
    }

    /// Whether the cockpit may tell this terminal how far the loop has
    /// got ([`Progress`](magistr_cockpit::cockpit::Progress)).
    ///
    /// A terminal on the allowlist, reached directly: tmux does not
    /// forward an OSC it does not know, so a multiplexed session is
    /// silent rather than hoping; and a user who asked for the plain-text
    /// or the flat cockpit asked for a quiet one.
    #[must_use]
    pub fn shows_progress(&self) -> bool {
        self.progress && !self.tmux && !self.forces_text() && !self.screen_reader
    }

    /// Whether the cockpit may draw a curly, dotted, dashed or double
    /// underline and give it its own colour.
    ///
    /// A terminal on the allowlist, reached directly. tmux is refused
    /// for the same reason it is refused a progress bar: what it does
    /// with a sub-parameter it does not know depends on its version, and
    /// the plain `SGR 4` every style falls back to reads correctly
    /// everywhere.
    #[must_use]
    pub const fn styled_underlines(&self) -> bool {
        self.underlines && !self.tmux
    }

    /// How commands must be wrapped to reach the terminal.
    #[must_use]
    pub const fn passthrough(&self) -> Passthrough {
        Passthrough::for_tmux(self.tmux)
    }
}

#[cfg(test)]
#[path = "env.tests.rs"]
mod tests;
