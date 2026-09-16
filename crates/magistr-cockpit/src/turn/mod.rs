//! The agent's turn as the stage shows it: source lines and the rows they
//! wrap to at the stage width.
//!
//! A [`Turn`] takes the agent's deltas as they stream — text, thought,
//! the user's echoed prompt, tool calls and their updates, notes and
//! errors — and keeps the wrapped rows current incrementally: a delta
//! re-wraps only the line it lands on, a tool update replaces its one
//! row in place, a resize re-wraps everything once. A row is styled by
//! its block kind ([`RowKind`]: a heading, a fenced code block, a list
//! item, a tool line by status) with inline [`Span`]s over its text —
//! markdown emphasis, inline code and links parsed by [`parse_inline`],
//! and fenced lines coloured by [`Highlight`], syntect run once per
//! closed line. The [`Viewport`] follows the tail until the user scrolls.
//!
//! Pure, like the cockpit core: `tests/purity.rs` walks this module too.

pub mod accent;
pub mod anchor;
pub mod conversation_card;
pub mod conversation_layout;
pub mod conversation_row;
pub mod diff;
pub mod diff_kind;
pub mod highlight;
pub mod hunk;
pub mod hunk_row;
pub mod hunk_span;
pub mod image;
pub mod inline;
pub mod line;
pub mod line_kind;
pub mod pixel_wrap;
pub mod receipt;
pub mod row;
pub mod row_kind;
pub mod span;
pub mod span_style;
pub mod stream;
pub mod viewport;
pub mod voice;
pub mod wrap;

pub use accent::Accent;
pub use anchor::Anchor;
pub use conversation_card::ConversationCard;
pub use conversation_layout::ConversationLayout;
pub use conversation_row::ConversationRow;
pub use diff_kind::diff_kind;
pub use highlight::Highlight;
pub use hunk::Hunk;
pub use hunk_row::HunkRow;
pub use hunk_span::HunkSpan;
pub use image::{MAX_IMAGE_ROWS, StreamImage};
pub use inline::parse_inline;
pub use line::Line;
pub use line_kind::LineKind;
pub use receipt::Receipt;
pub use row::Row;
pub use row_kind::RowKind;
pub use span::Span;
pub use span_style::SpanStyle;
pub use stream::Turn;
pub use viewport::Viewport;
pub use voice::Voice;
pub use wrap::wrap_line;
