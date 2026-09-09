//! The flat cockpit: the session as linear, append-only plain text.
//!
//! A screen reader cannot read a diffed cell grid. Box drawing is spoken
//! as punctuation, a spinner re-reads itself sixteen times a second, and
//! the alternate screen takes the transcript out of the terminal's own
//! scrollback — which is exactly what breaks VoiceOver in Terminal.app.
//! So `MAGISTR_TUI_SCREEN_READER` ([`Env::SCREEN_READER`]) renders the
//! session as lines instead: one role-labelled line per thing that
//! happened, written once, never redrawn.
//!
//! Two facts make it small rather than a second renderer.
//!
//! 1. The [`Turn`](magistr_cockpit::turn::Turn) already holds the session twice —
//!    as **source lines** ([`Turn::lines`](magistr_cockpit::turn::Turn::lines))
//!    and as the **wrapped rows** the painter styles. The glyph columns
//!    are added at the wrap, so reading the source lines removes them
//!    without a single strip rule. What glyphs the cockpit *does* bake
//!    into its own text (`▸ what · detail`) go through [`plain`], which
//!    strips by Unicode range rather than by an icon list that would
//!    drift from [`Icons`](magistr_cockpit::theme::Icons) — and the agent's own
//!    prose is never touched by it.
//! 2. Redraw-free is a **settledness** question, and [`LineKind`] can
//!    answer it: a prose line grows only while it is the last one and
//!    deltas are arriving; a tool or edit line changes only until its
//!    status is terminal; everything else is final where it was pushed.
//!    A turn that is no longer streaming is settled whole, which is what
//!    stops a cancelled pending call from wedging the transcript.
//!
//! [`Transcript`] emits settled history: render, take the settled
//! *prefix*, write what is past the high-water mark. The prefix — rather
//! than every settled line — is what keeps the order the reader hears
//! the same as the order things happened.
//!
//! The live prompt is separate: `echo::Echo` edits only the current input
//! row and preserves every edit before frame batching. Transcript output
//! ends that draft row, appends fresh settled lines and restores the draft.
//!
//! [`Env::SCREEN_READER`]: magistr_cockpit::capability::Mode::SCREEN_READER_ENV
//! [`LineKind`]: magistr_cockpit::turn::LineKind

pub(crate) mod echo;
pub mod flat_line;
pub mod lines;
pub mod plain;
pub mod role;
pub mod transcript;

pub use flat_line::FlatLine;
pub use lines::lines;
pub use plain::plain;
pub use role::Role;
pub use transcript::Transcript;
