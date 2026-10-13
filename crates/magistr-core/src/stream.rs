//! Frontend-agnostic stream content types.
//!
//! These types represent streaming agent output in a way that both desktop and command-line consumers
//! frontends can consume. No rendering framework dependencies — pure data.
//!
//! # Types
//!
//! - [`TurnRole`] — conversation turn ownership (user / assistant / system)
//! - [`TextStyle`] — semantic style tag for text segments
//! - [`StyledSegment`] — text + style pair (serializable, `String`-based)
//! - [`StreamBlock`] — one agent turn's metadata (serializable, no `Instant`)

use serde::{Deserialize, Serialize};

/// The role of a conversation turn.
///
/// Used by frontends to render role indicators (gutter labels, avatars,
/// colored borders) for visual separation between turns.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TurnRole {
    /// User message.
    User,
    /// Assistant response.
    Assistant,
    /// System/tool content.
    System,
}

impl TurnRole {
    /// Short label for compact display.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::User => "you",
            Self::Assistant => "mag",
            Self::System => "sys",
        }
    }
}

/// Semantic style tags for stream content segments.
///
/// Each variant describes *what* the text represents, not *how* it should
/// look. Frontends map these tags to their own rendering primitives
/// (ratatui `Style`, CSS classes, GPU shader uniforms, etc.).
///
/// The `#[repr(u8)]` ensures stable discriminant values for compact
/// serialization (e.g., spill-buffer persistence).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum TextStyle {
    /// Normal body text.
    Normal = 0,
    /// Bold text (from `**text**`).
    Bold = 1,
    /// Inline code (from `` `text` ``).
    Code = 2,
    /// Heading text (from `# text`).
    Heading = 3,
    /// Muted/dim text.
    Muted = 4,
    /// Success-colored text.
    Success = 5,
    /// Error-colored text.
    Error = 6,
    /// Warning-colored text.
    Warning = 7,
    /// Tool call prefix (▸ symbol).
    ToolCall = 8,
    /// Tool name (bold).
    ToolName = 9,
    /// Tool arguments/path (dim).
    ToolDetail = 10,
    /// Tool success result (green).
    ToolOk = 11,
    /// Tool result line (e.g., "250 lines").
    ToolResult = 12,
    /// Task tag prefix (dim, e.g., `[agent: desc...]`).
    TaskTag = 13,
    // --- Thinking block styles ---
    /// Thinking block header text.
    ThinkingHeader = 14,
    /// Thinking block left border accent.
    ThinkingBorder = 15,
    /// Thinking block body text (slightly dimmed).
    ThinkingBody = 16,
    // --- Code block styles ---
    /// Code block left border accent.
    CodeBlockBorder = 17,
    /// Code block line numbers.
    CodeBlockLineNum = 18,
    /// Code block header language badge.
    CodeBlockBadge = 19,
    /// Code block header line count indicator.
    CodeBlockLineCount = 20,
    /// Code block body text (unfocused code blocks).
    CodeBlockBody = 21,
    /// Syntax: language keywords.
    SyntaxKeyword = 22,
    /// Syntax: string literals.
    SyntaxString = 23,
    /// Syntax: comments.
    SyntaxComment = 24,
    /// Syntax: type names.
    SyntaxType = 25,
    /// Syntax: function names.
    SyntaxFunction = 26,
    /// Syntax: numeric literals.
    SyntaxNumber = 27,
    /// Syntax: operators and punctuation.
    SyntaxOperator = 28,
    /// Syntax: default code text (no special highlighting).
    SyntaxDefault = 29,
    // --- Rich inline formatting ---
    /// Italic text (from `*text*`).
    Italic = 30,
    /// Strikethrough text (from `~~text~~`).
    Strikethrough = 31,
    /// Horizontal rule (from `---`, `***`, `___`).
    HorizontalRule = 32,
    /// List marker (•, ◦, ▪ for bullets; "1." for numbered).
    ListMarker = 33,
    /// URL (`http://` or `https://`).
    Url = 34,
    // --- Diff rendering ---
    /// Diff added line.
    DiffAdd = 35,
    /// Diff removed line.
    DiffRemove = 36,
    /// Diff header line (`---`, `+++`, `@@` markers).
    DiffHeader = 37,
    /// Diff gutter marker (`+`, `-`, `±`).
    DiffGutter = 38,
    // --- Block quote ---
    /// Block quote left border accent bar.
    BlockQuoteBorder = 39,
    /// Block quote body text (dimmed).
    BlockQuote = 40,
    // --- Table rendering ---
    /// Table border characters.
    TableBorder = 41,
    /// Table header cell text (bold).
    TableHeader = 42,
    /// Table body cell text.
    TableCell = 43,
    // --- Image placeholder ---
    /// Image placeholder border.
    ImageBorder = 44,
    /// Image placeholder label.
    ImageLabel = 45,
    // --- Hyperlink ---
    /// File path hyperlink (underlined, clickable).
    Hyperlink = 46,
    // --- Search highlight ---
    /// Search match (background highlight).
    SearchMatch = 47,
    /// Active/current search match (brighter highlight).
    SearchMatchActive = 48,
    // --- Output block ---
    /// Block separator line.
    BlockSeparator = 49,
    /// Block header metadata (turn number, timestamp, cost).
    BlockHeader = 50,
    // --- Bookmark ---
    /// Bookmark marker in scrollbar gutter.
    BookmarkMarker = 51,
    // --- Progressive disclosure ---
    /// Disclosure indicator ("⋯ N more lines ⋯").
    DisclosureIndicator = 52,
    /// "Jump to end" indicator.
    JumpToEnd = 53,
    // --- Rich inline elements ---
    /// File path badge: icon + path with background tint.
    FilePathBadge = 54,
    /// URL domain portion (bold accent).
    UrlDomain = 55,
    /// Inline diff added text.
    InlineDiffAdd = 56,
    /// Inline diff removed text.
    InlineDiffRemove = 57,
    /// Progress mention fraction.
    ProgressMention = 58,
    // --- Tool card ---
    /// Tool call duration badge.
    ToolDuration = 59,
    /// Tool card expand/collapse toggle indicator.
    ToolToggle = 60,
}

impl TextStyle {
    /// Total number of variants — useful for discriminant range checks.
    pub const COUNT: usize = 61;

    /// Reconstruct from a `u8` discriminant (e.g., deserialized from spill buffer).
    ///
    /// Returns `None` if the discriminant is out of range.
    #[must_use]
    pub fn from_discriminant(n: u8) -> Option<Self> {
        if (n as usize) < Self::COUNT {
            // SAFETY: TextStyle is #[repr(u8)] with contiguous discriminants 0..COUNT.
            Some(unsafe { std::mem::transmute::<u8, Self>(n) })
        } else {
            None
        }
    }

    /// The `u8` discriminant of this variant.
    #[must_use]
    pub fn discriminant(self) -> u8 {
        self as u8
    }
}

/// A segment of styled text — the serializable, frontend-agnostic unit.
///
/// This is the wire format for stream content. Frontends convert to their
/// own optimized representations (e.g., TUI uses `Cow<'static, str>` for
/// zero-copy static strings).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StyledSegment {
    /// The text content.
    pub text: String,
    /// Semantic style tag.
    pub style: TextStyle,
}

/// Metadata for one agent turn — the serializable, frontend-agnostic block.
///
/// Frontends augment this with rendering-specific fields (e.g., TUI adds
/// `Instant` for live elapsed-time display).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StreamBlock {
    /// Unique block ID (monotonic counter).
    pub id: usize,
    /// Turn number (1-indexed, displayed to user).
    pub turn_number: usize,
    /// Milliseconds since UNIX epoch when the block was created.
    pub created_at_ms: u64,
    /// Whether the block is collapsed.
    pub collapsed: bool,
    /// Cost for this turn in USD (set when result arrives).
    pub cost_usd: Option<f64>,
    /// Role of this turn.
    pub role: TurnRole,
}

#[cfg(test)]
#[path = "stream.tests.rs"]
mod tests;
