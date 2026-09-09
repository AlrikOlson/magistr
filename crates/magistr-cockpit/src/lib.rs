//! magistr-cockpit — the mission-control cockpit, independent of the
//! surface it is painted on.
//!
//! The crate is the Elm-style core ([`cockpit`]: Model, Msg, update,
//! view) and everything the core is made of or feeds on that says
//! nothing about a terminal or a window: the flexbox layout tree
//! ([`layout`], taffy underneath) that resolves widget rects in cells,
//! mouse hit-testing ([`hit`]) from those rects, the input vocabulary
//! ([`input`]: keys, mouse, paste, focus, query replies — and the
//! decoder that reads them from bytes), the [`turn`] (the agent's turn as
//! source lines and the rows they wrap to), the [`theme`] (the palette
//! from magistr-theme, the pens every painter draws with), the cell
//! vocabulary and the cell painter ([`render`]: a styled grapheme, a grid
//! of them, and `paint` putting a laid-out view into the grid), the
//! pixel chrome ([`pixel`]: the widgets that draw with tiny-skia into a
//! surface sized in cells), the producers — [`job`]s off the loop thread,
//! the think-and-ship [`engine`] client and feed, the [`agent`] relay
//! over ACP, the [`workbench`] and [`checkpoint`] ports — and the
//! [`channel`]s they post on. What a surface can show reaches the core
//! as one value, the [`capability`] vocabulary; how it shows it is the
//! renderer's business.
//!
//! Two renderers paint this core and fork none of it: magistr-tui (a
//! cell buffer diffed and encoded for a terminal, plus the terminal's
//! pixel protocols) and magistr-gui2 (the same tree as a draw list on a
//! winit window). Nothing here names a terminal protocol —
//! `tests/boundary.rs` walks the crate for one.
//!
//! Hygiene: dead and unused code is a compile error here — no
//! file in this crate silences that lint, and `scripts/lint-structure.sh`
//! refuses one that tries.

#![deny(dead_code, unused)]
#![forbid(unsafe_code)]

pub mod agent;
pub mod base64;
pub mod capability;
pub mod channel;
pub mod checkpoint;
pub mod cockpit;
pub mod engine;
pub mod fonts;
pub mod golden;
pub mod harness;
pub mod hit;
pub mod input;
pub mod job;
pub mod layout;
pub mod pixel;
pub mod render;
pub mod shelf;
pub mod theme;
pub mod turn;
pub mod widget_id;
pub mod wire;
pub mod workbench;

pub use widget_id::WidgetId;
