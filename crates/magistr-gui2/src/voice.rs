use std::borrow::Cow;

use magistr_cockpit::layout::TextRole;
use qs_text::Features;

/// Typography derived from the reference stylesheet.
///
/// `scripts/almagest-tokens.py` reads `design/almagest-seventh-edition.html` and
/// generates `design/almagest7_tokens.rs` plus JSON. The crate includes the generated
/// table, and the extractor rejects missing voices.
///
/// A voice specifies font family, size, weight, tracking, case transformation, and line
/// height. Color remains the responsibility of [`Pen`](magistr_cockpit::theme::Pen).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Voice {
    /// `"mono"`, `"sans"` or `"serif"` — the three embedded families.
    pub family: &'static str,
    /// The size in logical pixels, at the design's own density.
    pub px: f32,
    /// The CSS weight. The family breaks at 600, so 550 is the regular
    /// face and not a third cut.
    pub weight: u16,
    /// Letter-spacing as a share of the size, added after every
    /// grapheme cluster the way CSS adds it.
    pub tracking_em: f32,
    /// The row height as a multiple of the size, when the volume
    /// declares one; `0.0` means "the window's own leading".
    pub line_height: f32,
    /// Whether the volume uppercases the voice before setting it.
    pub upper: bool,
}

// The generated table. `Voice`'s fields are the extractor's own
// initializer order, so a field added there is a compile error here
// rather than a silently dropped column.
//
// Keep all generated stylesheet tokens, including those without a Rust consumer.
// This preserves a complete mapping between the reference and generated table.
#[allow(dead_code)]
mod oracle {
    use super::Voice;
    include!("../design/almagest7_tokens.rs");
}

impl Voice {
    /// The window's own leading — the volume's `.app7 { line-height }` —
    /// which every voice that declares none of its own is set on.
    pub const LEADING: f32 = oracle::LEADING;
    /// The budget meter's track thickness, in design pixels.
    pub const METER_TRACK_HEIGHT: f32 = oracle::METER_TRACK_HEIGHT;
    /// The width of the meter's breaker tick.
    pub const METER_TICK_WIDTH: f32 = oracle::METER_TICK_WIDTH;

    /// The supervision rail's width — view 05's `width:248px`, the
    /// hairline included.
    pub const RAIL_WIDTH: f32 = oracle::RAIL_WIDTH;
    /// The rail's own padding (`padding:18px 16px`).
    pub const RAIL_PAD_TOP: f32 = oracle::RAIL_PAD_TOP;
    pub const RAIL_PAD_SIDE: f32 = oracle::RAIL_PAD_SIDE;
    /// The air under the rail's first cap (`.cap7 { margin-bottom:12px }`).
    pub const RAIL_CAP_GAP: f32 = oracle::RAIL_CAP_GAP;
    /// The spine's indent past its line (`padding-left:14px`) and the
    /// air between two of its entries (`margin-bottom:11px`).
    pub const RAIL_LEDGER_INDENT: f32 = oracle::RAIL_LEDGER_INDENT;
    pub const RAIL_LEDGER_PITCH: f32 = oracle::RAIL_LEDGER_PITCH;
    /// The air over a later section's cap (`margin:18px 0 8px`) and
    /// under it.
    pub const RAIL_SECTION_GAP: f32 = oracle::RAIL_SECTION_GAP;
    pub const RAIL_SECTION_CAP_GAP: f32 = oracle::RAIL_SECTION_CAP_GAP;

    /// The action verb's voice — `.btn7`'s own sans 12/600.
    pub const BUTTON: Self = oracle::BUTTON;
    /// The key cap's voice — `.kbd7`'s mono 9.5.
    pub const KBD: Self = oracle::KBD;
    /// The theme card label's voice — `.tcard .tl2`'s sans 11.5.
    pub const THEME_LABEL: Self = oracle::THEME_LABEL;
    /// The dashed slot's copy — the volume's inline sans 12.
    pub const SLOT: Self = oracle::SLOT;
    /// The crumb's voice — `.crumb`'s mono 11.
    pub const CRUMB: Self = oracle::CRUMB;
    /// The meter value's voice — `.meter .mv`'s mono 10.5.
    pub const METER_VALUE: Self = oracle::METER_VALUE;
    /// The chip's voice — `.chip7`'s mono 10.5, by its own name (the
    /// role mapping reaches it as [`TextRole::Small`], the atoms by
    /// what the volume calls it).
    pub const CHIP: Self = oracle::CHIP;
    /// The cap's voice — `.cap7`'s tracked uppercase mono 9.5.
    pub const CAP: Self = oracle::CAP;
    /// The meter's stop note — the cap at the `.06em` the volume sets
    /// on it inline (`.mf .cap7 { letter-spacing:.06em }`), which the
    /// extractor, reading class rules, does not see: the one place a
    /// voice varies from its class by an inline style. `Stop at 10.00`
    /// is 7px narrower on the capture than the cap's `.12em` sets it.
    pub const METER_NOTE: Self = Self {
        tracking_em: 0.06,
        ..oracle::CAP
    };
    /// The status row name's voice — `.srow .sn`'s sans 12/550.
    pub const LEDGER_NAME: Self = oracle::LEDGER_NAME;
    /// The status row meta's voice — `.srow .sm`'s mono 10, which the
    /// receipt row's accent action (`.rr .ra`) shares.
    pub const LEDGER_META: Self = oracle::LEDGER_META;
    /// The receipt key's voice — `.rr .rk`'s tracked uppercase mono 9.5.
    pub const RECEIPT_KEY: Self = oracle::RECEIPT_KEY;
    /// The receipt value's voice — `.rr .rv`'s mono 11.
    pub const VALUE: Self = oracle::VALUE;
    /// The mode badge's voice — `.mode7`'s tracked mono 9.
    pub const BADGE: Self = oracle::BADGE;
    /// The key hint label's voice — `.khint7`'s sans 10.5.
    pub const HINT: Self = oracle::HINT;
    /// The key hint at the ledger head's size — view 05's rail sets its
    /// three pairs inline at `font-size:9.5px`, which the extractor,
    /// reading class rules, does not see.
    pub const LEDGER_HINT: Self = Self {
        px: 9.5,
        ..oracle::HINT
    };
    /// The rail's accent verb — a checkpoint row's `Preview` and the meter's `Raise`:
    /// `.rr.ra`'s mono in the mauve, set inline on view 05's rail at `font-size:9.5px`
    /// (the eighth edition), which the extractor, reading class rules, does not see.
    pub const RAIL_VERB: Self = Self {
        px: 9.5,
        ..oracle::LEDGER_META
    };
    /// The stat counter's voice — the indicator figure's mono 13.
    pub const STAT: Self = oracle::STAT;
    /// The phase word's voice — the footer phases' mono 10.5.
    pub const PHASE: Self = oracle::PHASE;
    /// The diff line's voice — `.diffline`'s mono 10.5 on its own 1.75.
    pub const DIFFLINE: Self = oracle::DIFFLINE;
    /// The typed query's voice — the Steer placeholder's sans 12.5,
    /// which the search field's query shares.
    pub const STEER: Self = oracle::STEER;
    /// The display voice — the chapter title's serif 24 at −0.01 em, the volume's one
    /// use of `--serif`. reserves the serif for report titles and the project cover at
    /// ≥20 px, so this is the only serif voice there is and
    /// [`ChapterTitle`](crate::ChapterTitle) is its consumer; `tests/design_oracle.rs`
    /// pins both halves against the table.
    pub const DISPLAY: Self = oracle::CHAPTER_TITLE;

    /// The grid's own voice: the mono family at whatever size the cell
    /// metrics make it. Its `px` is zero because there is no number to
    /// put there — the cell decides, and [`TypeScale`] reads the cell
    /// for exactly this role. A voice with no size is safe precisely
    /// because it is the one role nothing sizes from the table.
    ///
    /// [`TypeScale`]: crate::TypeScale
    pub const DATA: Self = Self {
        family: "mono",
        px: 0.0,
        weight: 400,
        tracking_em: 0.0,
        line_height: 0.0,
        upper: false,
    };

    /// The voice `role` is set in.
    ///
    /// Every role but [`TextRole::Data`] names one of the volume's own
    /// voices, and this mapping — which role wears which voice — is
    /// the crate's one hand-made decision about the table, which is
    /// exactly what `tests/design_oracle.rs` pins against the JSON.
    #[must_use]
    pub const fn of(role: TextRole) -> Self {
        match role {
            TextRole::Data => Self::DATA,
            TextRole::Badge => oracle::BADGE,
            TextRole::Cap => oracle::CAP,
            TextRole::Fine => oracle::LEDGER_META,
            TextRole::Small => oracle::CHIP,
            TextRole::Code => oracle::DIFFLINE,
            TextRole::Value => oracle::VALUE,
            TextRole::Label => oracle::LEDGER_NAME,
            TextRole::Steer => oracle::STEER,
            TextRole::Prose => oracle::PROSE,
            TextRole::Heading => oracle::HEADING,
        }
    }

    /// The name the oracle's table files this voice under, for the test
    /// that reads the JSON back. `None` for [`TextRole::Data`], which
    /// the volume has no voice for because it is the grid's.
    #[must_use]
    pub const fn token(role: TextRole) -> Option<&'static str> {
        Some(match role {
            TextRole::Data => return None,
            TextRole::Small => "chip",
            TextRole::Badge => "badge",
            TextRole::Cap => "cap",
            TextRole::Fine => "ledger_meta",
            TextRole::Code => "diffline",
            TextRole::Value => "value",
            TextRole::Label => "ledger_name",
            TextRole::Steer => "steer",
            TextRole::Prose => "prose",
            TextRole::Heading => "heading",
        })
    }

    /// How tall one row of this voice is at `px`, before the face's own
    /// line box is allowed to win.
    ///
    /// One rule with a per-voice leading, rather than a constant and a
    /// family branch: the volume gives prose 1.65 and a diff line 1.75
    /// and says nothing about the rest, so the rest are set on the
    /// window's own 1.5. It is `.app7`'s and not `body`'s: the page
    /// sets 1.6 for the volume's prose, every one of the nineteen views
    /// sets its own 1.5, and a voice inside a view inherits that one.
    #[must_use]
    pub fn leading(&self) -> f32 {
        if self.line_height > 0.0 {
            self.line_height
        } else {
            Self::LEADING
        }
    }

    /// `text` as the volume sets it — uppercased when the voice asks,
    /// borrowed unchanged when it does not, so the ordinary run costs
    /// no allocation.
    #[must_use]
    pub fn transform<'a>(&self, text: &'a str) -> Cow<'a, str> {
        if self.upper && text.chars().any(char::is_lowercase) {
            Cow::Owned(text.to_uppercase())
        } else {
            Cow::Borrowed(text)
        }
    }

    /// Whether this voice is set in the mono family.
    #[must_use]
    pub fn is_mono(&self) -> bool {
        self.family == "mono" || self.family == "mono-italic"
    }

    pub(crate) fn italic(mut self) -> Self {
        self.family = if self.is_mono() {
            "mono-italic"
        } else {
            "sans-italic"
        };
        self
    }

    pub(crate) fn is_italic(&self) -> bool {
        matches!(self.family, "mono-italic" | "sans-italic")
    }

    /// Whether this voice is set in the serif display family — the
    /// volume's `--serif`, which appears on the chapter title and
    /// nowhere else.
    #[must_use]
    pub fn is_serif(&self) -> bool {
        self.family == "serif"
    }

    /// The shaping features this voice is set under.
    ///
    /// The mono family is the *data* voice — numerals, ids, receipts,
    /// timers — and the design engraves it: the slashed zero and
    /// tabular figures (`font-feature-settings: "zero"`,
    /// `tabular-nums` in the volume's `.mono`/`.num` rules). The face's
    /// own default zero is dotted — the terminal's ornament, drawn by a
    /// rasterizer that never shapes — so the window asks for the
    /// alternate by feature rather than by a second face: one set of
    /// outlines, two renderers, and the cell contract untouched because
    /// an alternate glyph has the same advance. The sans and the serif
    /// carry no data and are set plain.
    #[must_use]
    pub fn features(&self) -> Features {
        if self.is_mono() {
            Features {
                tabular_figures: true,
                slashed_zero: true,
            }
        } else {
            Features::default()
        }
    }
}

#[cfg(test)]
#[path = "voice.tests.rs"]
mod tests;
