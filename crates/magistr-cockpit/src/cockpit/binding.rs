//! The cockpit's keyboard registry: every accelerator it registers,
//! with the group the accelerator sheet lists it under.
//!
//! The sheet and the footer's contextual hints are *generated* from
//! this table — never hand-written. `binding.tests.rs` scans the other
//! cockpit sources and fails on a key-hint literal outside this file,
//! and asserts every footer context is a subset of [`Binding::ALL`].

use crate::input::{KeyCode, Modifiers};

/// One keyboard accelerator: the keys as the sheet spells them, what
/// pressing them does, the sheet group it belongs to — and one
/// *canonical* key, the dispatchable form the ⌘K palette sends through
/// the fold when the binding is run from a list rather than typed.
/// Multi-key spellings (`↑↓`, `ctrl-a/e/u/k/w`) canonicalize to their
/// first key, so running one is pressing its opening gesture.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Binding {
    /// The keys, as the sheet spells them (`ctrl-d`, `↑↓`).
    pub keys: &'static str,
    /// What pressing them does.
    pub does: &'static str,
    /// The sheet's group heading.
    pub group: &'static str,
    /// The canonical key, the one a palette dispatch sends.
    pub code: KeyCode,
    /// The modifiers that key is sent with.
    pub mods: Modifiers,
}

impl Binding {
    const fn new(
        keys: &'static str,
        does: &'static str,
        group: &'static str,
        code: KeyCode,
        mods: Modifiers,
    ) -> Self {
        Self {
            keys,
            does,
            group,
            code,
            mods,
        }
    }

    // ── trace ───────────────────────────────────────────────────────
    /// Walk the reasoning steps on the Trace tab.
    pub const TRACE_STEP: Self = Self::new("↑↓", "select", "trace", KeyCode::Up, Modifiers::NONE);
    /// Show every field of the selected step, or the brief cut again.
    pub const TRACE_DETAIL: Self =
        Self::new("enter", "detail", "trace", KeyCode::Enter, Modifiers::NONE);

    // ── global ──────────────────────────────────────────────────────
    /// Cycle the stage tabs.
    pub const TAB: Self = Self::new(
        "tab / shift-tab",
        "switch stage tab",
        "global",
        KeyCode::Tab,
        Modifiers::NONE,
    );
    /// Open the accelerator sheet (on an empty prompt).
    pub const SHEET: Self = Self::new(
        "?",
        "keyboard help",
        "global",
        KeyCode::Char('?'),
        Modifiers::NONE,
    );
    /// Close the sheet.
    pub const ESC_SHEET: Self =
        Self::new("esc", "close help", "global", KeyCode::Esc, Modifiers::NONE);
    /// Quit on an empty prompt.
    pub const QUIT: Self = Self::new(
        "ctrl-d",
        "quit",
        "global",
        KeyCode::Char('d'),
        Modifiers::CTRL,
    );
    /// Cancel a streaming turn — quit otherwise.
    pub const CANCEL: Self = Self::new(
        "ctrl-c",
        "cancel",
        "global",
        KeyCode::Char('c'),
        Modifiers::CTRL,
    );

    // ── launcher (Home tab) ─────────────────────────────────────────
    /// Start one iteration.
    pub const NEXT: Self = Self::new(
        "n",
        "next iteration",
        "launcher",
        KeyCode::Char('n'),
        Modifiers::NONE,
    );
    /// Start the autonomous loop.
    pub const LOOP: Self = Self::new("l", "loop", "launcher", KeyCode::Char('l'), Modifiers::NONE);
    /// Run the quality gates.
    pub const GATES: Self = Self::new(
        "g",
        "gates",
        "launcher",
        KeyCode::Char('g'),
        Modifiers::NONE,
    );
    /// Open the Roadmap tab.
    pub const ROADMAP: Self = Self::new(
        "m",
        "roadmap",
        "launcher",
        KeyCode::Char('m'),
        Modifiers::NONE,
    );
    /// Walk the sessions card and continue the selected session.
    pub const RESUME: Self = Self::new(
        "↑↓ enter",
        "resume session",
        "launcher",
        KeyCode::Down,
        Modifiers::NONE,
    );

    // ── roadmap (the tech-tree) ─────────────────────────────────────
    /// Step the tech-tree's zoom in or out (on an empty prompt, where
    /// the terminal draws it as a picture rather than a legend).
    pub const ZOOM: Self = Self::new(
        "+ / -",
        "zoom tech-tree",
        "roadmap",
        KeyCode::Char('+'),
        Modifiers::NONE,
    );
    /// Put the zoom and the pan back to the fit.
    pub const ZOOM_FIT: Self = Self::new(
        "0",
        "fit tech-tree",
        "roadmap",
        KeyCode::Char('0'),
        Modifiers::NONE,
    );

    // ── prompt ──────────────────────────────────────────────────────
    /// Send the prompt as a turn.
    pub const SEND: Self = Self::new("enter", "send", "prompt", KeyCode::Enter, Modifiers::NONE);
    /// Insert a newline instead of sending.
    pub const NEWLINE: Self = Self::new(
        "alt-enter",
        "newline",
        "prompt",
        KeyCode::Enter,
        Modifiers::ALT,
    );
    /// Walk the prompt history.
    pub const HISTORY: Self = Self::new("↑↓", "history", "prompt", KeyCode::Up, Modifiers::NONE);
    /// Open file completion.
    pub const AT_FILES: Self = Self::new(
        "@",
        "file completion",
        "prompt",
        KeyCode::Char('@'),
        Modifiers::NONE,
    );
    /// Open command completion at the line start.
    pub const SLASH: Self = Self::new(
        "/",
        "command palette",
        "prompt",
        KeyCode::Char('/'),
        Modifiers::NONE,
    );
    /// Emacs-style line editing.
    pub const EDIT: Self = Self::new(
        "ctrl-a/e/u/k/w",
        "line editing",
        "prompt",
        KeyCode::Char('a'),
        Modifiers::CTRL,
    );

    // ── completion popup ────────────────────────────────────────────
    /// Move the popup selection.
    pub const SELECT: Self = Self::new("↑↓", "select", "completion", KeyCode::Up, Modifiers::NONE);
    /// Accept the selected candidate.
    pub const ACCEPT_COMPLETION: Self = Self::new(
        "enter",
        "accept",
        "completion",
        KeyCode::Enter,
        Modifiers::NONE,
    );
    /// Close the popup.
    pub const ESC_CLOSE: Self =
        Self::new("esc", "close", "completion", KeyCode::Esc, Modifiers::NONE);

    // ── stage ───────────────────────────────────────────────────────
    /// Scroll the stage.
    pub const SCROLL: Self = Self::new(
        "pgup/pgdn",
        "scroll",
        "stage",
        KeyCode::PageUp,
        Modifiers::NONE,
    );
    /// Jump to the top or the tail (on an empty prompt).
    pub const HOME_END: Self = Self::new(
        "home/end",
        "top / tail",
        "stage",
        KeyCode::Home,
        Modifiers::NONE,
    );
    /// Fold or unfold the most recent card (on an empty prompt).
    pub const FOLD: Self = Self::new(
        "enter",
        "fold last card",
        "stage",
        KeyCode::Enter,
        Modifiers::NONE,
    );
    /// Return every open card's terminal pane to its live tail (on an
    /// empty prompt, while one is scrolled up) — the jump pill's key.
    pub const FOLLOW: Self = Self::new(
        "f",
        "follow pane tail",
        "stage",
        KeyCode::Char('f'),
        Modifiers::NONE,
    );
    /// Raise the running card's terminal into the dock over the
    /// session, or lower it — the terminal's
    /// key; a window takes [`Binding::DOCK_CHORD`].
    pub const DOCK: Self = Self::new(
        "ctrl-t",
        "dock terminal",
        "stage",
        KeyCode::Char('t'),
        Modifiers::CTRL,
    );
    /// Kill the running card's program by holding the key half a
    /// second — on an empty
    /// prompt, and only where the key's release is reported: a window
    /// always, a terminal whose kitty keyboard flags carry event types.
    /// Never a ⌘K row: a row is a press with no release.
    pub const KILL: Self = Self::new(
        "k",
        "kill \u{b7} hold",
        "stage",
        KeyCode::Char('k'),
        Modifiers::NONE,
    );
    /// Put the running card's stripped transcript — the scrollback and
    /// the screen — on the clipboard, on an
    /// empty prompt.
    pub const COPY: Self = Self::new(
        "y",
        "copy transcript",
        "stage",
        KeyCode::Char('y'),
        Modifiers::NONE,
    );
    /// The window's dock verb, `⌘↵` — a chord, so never a sheet row and
    /// off [`Binding::ALL`]: a terminal never reports `SUPER`. The ⌘K
    /// row wears it instead of [`Binding::DOCK`]'s `ctrl-t`.
    pub const DOCK_CHORD: Self = Self::new(
        "\u{2318}\u{21b5}",
        "dock terminal",
        "stage",
        KeyCode::Enter,
        Modifiers::SUPER,
    );
    /// The window's maximize, `⌘⇧↵`: the dock takes the stage, the
    /// rail stays; again, back to the dragged height. A chord, off the
    /// sheet like [`Binding::DOCK_CHORD`].
    pub const DOCK_MAX_CHORD: Self = Self::new(
        "\u{2318}\u{21e7}\u{21b5}",
        "maximize dock",
        "stage",
        KeyCode::Enter,
        Modifiers::SUPER.union(Modifiers::SHIFT),
    );
    /// Close the Changes or Trace viewer pane.
    pub const ESC_VIEWER: Self = Self::new(
        "esc",
        "close viewer",
        "stage",
        KeyCode::Esc,
        Modifiers::NONE,
    );

    // ── needs-you dock ──────────────────────────────────────────────
    /// Allow the pending permission once.
    pub const ALLOW: Self = Self::new(
        "A",
        "allow",
        "needs-you",
        KeyCode::Char('a'),
        Modifiers::NONE,
    );
    /// Allow it for the whole session.
    pub const ALWAYS: Self = Self::new(
        "\u{21e7}A",
        "always",
        "needs-you",
        KeyCode::Char('A'),
        Modifiers::SHIFT,
    );
    /// Deny it once.
    pub const DENY: Self = Self::new(
        "D",
        "deny",
        "needs-you",
        KeyCode::Char('d'),
        Modifiers::NONE,
    );
    /// Deny it for the whole session.
    pub const NEVER: Self = Self::new(
        "\u{21e7}D",
        "never",
        "needs-you",
        KeyCode::Char('D'),
        Modifiers::SHIFT,
    );
    /// Show the pictures a Moment's ask carries.
    pub const SHOW_IMAGES: Self = Self::new(
        "space",
        "show images",
        "needs-you",
        KeyCode::Char(' '),
        Modifiers::NONE,
    );
    /// Open a Moment's conditional-approval register.
    pub const CONDITION: Self = Self::new(
        "\u{21e5}",
        "condition",
        "needs-you",
        KeyCode::Tab,
        Modifiers::NONE,
    );
    /// Accept the front proposal (on an empty prompt).
    pub const ACCEPT: Self = Self::new(
        "y",
        "accept",
        "needs-you",
        KeyCode::Char('y'),
        Modifiers::NONE,
    );
    /// Reject it.
    pub const REJECT: Self = Self::new(
        "n",
        "reject",
        "needs-you",
        KeyCode::Char('n'),
        Modifiers::NONE,
    );
    /// Clear the front blocker.
    pub const CLEAR: Self = Self::new(
        "c",
        "clear",
        "needs-you",
        KeyCode::Char('c'),
        Modifiers::NONE,
    );

    // ── loop ────────────────────────────────────────────────────────
    /// Pause the loop at the iteration boundary, or resume it.
    pub const PAUSE: Self = Self::new(
        "space",
        "pause / resume",
        "loop",
        KeyCode::Char(' '),
        Modifiers::NONE,
    );
    /// Run one iteration from the held boundary, then hold the next.
    pub const STEP: Self = Self::new("s", "step", "loop", KeyCode::Char('s'), Modifiers::NONE);
    /// Skip the next iteration while paused.
    pub const SKIP: Self = Self::new("x", "skip", "loop", KeyCode::Char('x'), Modifiers::NONE);
    /// Take a checkpoint of the held state while paused.
    pub const CHECKPOINT_NOW: Self = Self::new(
        "c",
        "checkpoint now",
        "loop",
        KeyCode::Char('c'),
        Modifiers::NONE,
    );
    /// Abort the loop while paused — the terminal's row: on a window
    /// Esc climbs and the verb is [`Binding::ABORT_CHORD`].
    pub const ABORT: Self = Self::new("esc", "abort", "loop", KeyCode::Esc, Modifiers::NONE);
    /// Abort a live loop from any state on a window: `⌘.`, the fourth
    /// verb draws on the ledger head. A chord, so never a
    /// sheet row and off [`Binding::ALL`] — a terminal never reports
    /// `SUPER` — the ledger head's pair and the ⌘K row wear it instead
    /// of [`Binding::ABORT`]'s `esc`.
    pub const ABORT_CHORD: Self = Self::new(
        "\u{2318}.",
        "abort",
        "loop",
        KeyCode::Char('.'),
        Modifiers::SUPER,
    );

    /// Every accelerator, in sheet order — the sheet renders exactly
    /// this list, grouped by [`Binding::group`].
    pub const ALL: &'static [Self] = &[
        Self::TAB,
        Self::SHEET,
        Self::ESC_SHEET,
        Self::QUIT,
        Self::CANCEL,
        Self::NEXT,
        Self::LOOP,
        Self::GATES,
        Self::ROADMAP,
        Self::RESUME,
        Self::ZOOM,
        Self::ZOOM_FIT,
        Self::SEND,
        Self::NEWLINE,
        Self::HISTORY,
        Self::AT_FILES,
        Self::SLASH,
        Self::EDIT,
        Self::SELECT,
        Self::ACCEPT_COMPLETION,
        Self::ESC_CLOSE,
        Self::SCROLL,
        Self::HOME_END,
        Self::TRACE_STEP,
        Self::TRACE_DETAIL,
        Self::FOLD,
        Self::FOLLOW,
        Self::DOCK,
        Self::KILL,
        Self::COPY,
        Self::ESC_VIEWER,
        Self::ALLOW,
        Self::ALWAYS,
        Self::DENY,
        Self::NEVER,
        Self::SHOW_IMAGES,
        Self::CONDITION,
        Self::ACCEPT,
        Self::REJECT,
        Self::CLEAR,
        Self::PAUSE,
        Self::STEP,
        Self::SKIP,
        Self::CHECKPOINT_NOW,
        Self::ABORT,
    ];

    /// The footer's completion-popup context.
    pub const HINT_COMPLETION: &'static [Self] =
        &[Self::SELECT, Self::ACCEPT_COMPLETION, Self::ESC_CLOSE];
    /// The footer while a permission waits on the dock's front.
    pub const HINT_PERMISSION: &'static [Self] = &[Self::ALLOW, Self::ALWAYS, Self::DENY];
    /// The footer while a proposal waits on the dock's front.
    pub const HINT_PROPOSAL: &'static [Self] = &[Self::ACCEPT, Self::REJECT];
    /// The footer while a blocker waits on the dock's front.
    pub const HINT_BLOCKER: &'static [Self] = &[Self::CLEAR];
    /// The footer while a viewer pane is open.
    pub const HINT_VIEWER: &'static [Self] = &[Self::ESC_VIEWER, Self::TAB];
    /// The footer while the loop is paused. Checkpoint now stays on the
    /// sheet and the palette: six pairs overrun an 80-column bar.
    pub const HINT_PAUSED: &'static [Self] =
        &[Self::PAUSE, Self::STEP, Self::SKIP, Self::ABORT, Self::QUIT];
    /// The footer while the loop runs and the turn streams.
    pub const HINT_LOOP_STREAMING: &'static [Self] = &[Self::PAUSE, Self::CANCEL, Self::QUIT];
    /// The footer while the loop runs between turns.
    pub const HINT_LOOP_IDLE: &'static [Self] = &[Self::PAUSE, Self::SEND, Self::QUIT];
    /// The footer while a turn streams.
    pub const HINT_STREAMING: &'static [Self] = &[Self::CANCEL, Self::QUIT];
    /// The footer at rest.
    pub const HINT_IDLE: &'static [Self] = &[Self::SEND, Self::HISTORY, Self::QUIT];
    /// The footer on a Trace tab with steps: the selection walks and
    /// Enter opens the full card.
    pub const HINT_TRACE: &'static [Self] = &[Self::TRACE_STEP, Self::TRACE_DETAIL, Self::QUIT];
    /// The footer on the Home tab.
    pub const HINT_HOME: &'static [Self] = &[
        Self::NEXT,
        Self::LOOP,
        Self::GATES,
        Self::ROADMAP,
        Self::SHEET,
        Self::QUIT,
    ];
    /// The footer while the sheet is open. It names the scroll keys
    /// because the registry outgrew a small terminal's stage and the
    /// sheet is exactly where a user on one looks.
    pub const HINT_SHEET: &'static [Self] = &[Self::ESC_SHEET, Self::SCROLL];

    /// Rows the sheet's body draws for the whole registry: one per
    /// binding, plus the blank line and the labelled divider each group
    /// opens with.
    ///
    /// The sheet scrolls, and this is the total
    /// its window is measured against — kept here, beside `ALL`, so the
    /// count cannot drift from the list it counts.
    #[must_use]
    pub fn sheet_rows() -> usize {
        let groups = Self::ALL
            .windows(2)
            .filter(|pair| pair[0].group != pair[1].group)
            .count()
            + usize::from(!Self::ALL.is_empty());
        Self::ALL.len() + groups * 2
    }

    /// A footer hint line: the context's bindings joined `keys does ·
    /// keys does`.
    #[must_use]
    pub fn hint(context: &[Self]) -> String {
        context
            .iter()
            .map(|binding| format!("{} {}", binding.keys, binding.does))
            .collect::<Vec<_>>()
            .join(" · ")
    }
}

#[cfg(test)]
#[path = "binding.tests.rs"]
mod tests;
