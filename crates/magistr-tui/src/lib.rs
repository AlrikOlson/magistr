//! magistr-tui — the mission-control cockpit.
//!
//! The crate owns its terminal renderer: a cell buffer with damage-tracked
//! diffing, an encoder that writes the minimal cursor moves and SGR runs
//! for a frame wrapped in DEC 2026 synchronized output, a flexbox layout
//! tree ([`layout`], taffy underneath) that resolves widget rects in cells,
//! mouse hit-testing ([`hit`]) from those rects, and the wire side of the
//! pixel layer ([`graphics`]: the kitty graphics protocol, tmux passthrough
//! and the startup probe that picks pixels or plain text), the input
//! decoder ([`input`]: keys, kitty keyboard protocol, SGR mouse, paste,
//! focus and query replies from bytes), the terminal side ([`term`]: the
//! session sequences, the [`Terminal`](term::Terminal) trait, its rustix
//! implementation and the input thread), the Elm-style core ([`cockpit`]:
//! Model, Msg, update, view), the [`flat`] cockpit (the same session as
//! linear, append-only plain text for a screen reader), the [`runtime`]
//! that drives the core over a
//! terminal, the producers that feed it — [`job`]s off the loop thread,
//! the think-and-ship [`engine`] client and feed, the [`agent`] relay
//! over ACP — the [`turn`] (the agent's turn as source lines and the
//! rows they wrap to, kept current per delta), the [`theme`] (dark or
//! light from the OSC 11 reply, the palette from magistr-theme, the pens
//! the painter uses) and a scripted
//! fake terminal, agent and engines ([`harness`]) for tests.
//! Everything renders headlessly into a `Vec<u8>` and decodes from a
//! `&[u8]`, so tests capture the exact byte stream a terminal would
//! receive and feed the exact bytes it would send.
//!
//! Hygiene: dead and unused code is a compile error here — no
//! file in this crate silences that lint, and `scripts/lint-structure.sh`
//! refuses one that tries. Nothing under this crate may reference the
//! frozen legacy TUI tree (`tests/boundary.rs` walks the crate for it).

#![deny(dead_code, unused)]
#![forbid(unsafe_code)]

pub mod flat;
pub mod graphics;
pub mod harness;
pub mod pixel;
pub mod render;
pub mod runtime;
pub mod screenshot;
pub mod term;
