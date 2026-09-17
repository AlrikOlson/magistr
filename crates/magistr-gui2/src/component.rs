/// The component a text run belongs to — law that there is no naked text: every string
/// on a view is a status row's, a receipt row's, a meter's, a stat's, a key hint's, a
/// badge's, a cap's… and a free-floating line is a missing component.
///
/// A component's constructor stamps what it builds
/// ([`Element::claim`](crate::Element::claim)); a composition that sets a run of its
/// own names the component the run is, at the call. The sweep in `tests/copy_laws.rs`
/// reads every run of every view back through [`Element::runs`](crate::Element::runs)
/// and refuses one with no owner — so a string cannot reach the window without saying
/// what it is.
///
/// The variants are the volume's own atoms (specimen sheet II) plus the few text roles
/// a composition sets directly — a title, a paragraph of prose, a value, a list row —
/// named so the sweep's report reads as the design's vocabulary and not as file names.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Component {
    /// `.srow` — a thing and how it is doing: dot · name · state · meta.
    StatusRow,
    /// `.rr` — a receipt: key · value · verb, or key · LEDs, or a rule
    /// with its mode badge.
    ReceiptRow,
    /// The deliverables rail's row — a picture's name and its measure.
    DeliverableRow,
    /// A survey finding — the teal key and the fact beside it.
    FindingRow,
    /// A settings row — the name, its note and its control.
    SettingRow,
    /// A search hit — the title and the matched line.
    SearchRow,
    /// A ⌘K palette row — verb · name · badge.
    CommandRow,
    /// One row of a list a composition draws itself — a proposal's
    /// chunk, a plan's step, a pinned line.
    ListRow,
    /// `.meter` — a label, a pair and the limit tick.
    Meter,
    /// The stat block — `211 / 306`.
    Stat,
    /// A counter in a cap — `4 of 12`, `6 results`, `3 projects`.
    Counter,
    /// `.khint7` — a key cap and its verb.
    KeyHint,
    /// `.mode7` — OFF · AUTO · ASK.
    ModeBadge,
    /// The phases strip — research ✓ plan ✓ execute verify.
    PhaseRail,
    /// `.cap7` — a tracked uppercase caption over a block.
    Cap,
    /// `.chip7` — a nav chip, a pick chip, a scope chip, a data pill,
    /// a segment.
    Chip,
    /// `.crumb` — the breadcrumb's stops.
    Crumb,
    /// `.btn7` — an action's verb.
    Button,
    /// A field — the composer, the search query, an answer.
    Field,
    /// A dashed slot's copy and hint.
    Slot,
    /// The diff ledger's key · value lines.
    Ledger,
    /// A diff block's title and lines.
    Diff,
    /// A paragraph — a turn's row, a chapter, a question, a signal.
    Prose,
    /// A title — the chapter's, a card's, a surface's.
    Title,
    /// A value on its own — an id, a ref, a source path.
    Value,
    /// A star's or a cluster's label — the display name a list may show.
    Label,
    /// The identity row — sigil · name · path.
    IdentityRow,
    /// The settings sidebar's items.
    Nav,
    /// A theme card's label.
    ThemeCard,
}

impl Component {
    /// Every component, in the order the design's specimen sheet and
    /// this enum list them.
    pub const ALL: [Self; 29] = [
        Self::StatusRow,
        Self::ReceiptRow,
        Self::DeliverableRow,
        Self::FindingRow,
        Self::SettingRow,
        Self::SearchRow,
        Self::CommandRow,
        Self::ListRow,
        Self::Meter,
        Self::Stat,
        Self::Counter,
        Self::KeyHint,
        Self::ModeBadge,
        Self::PhaseRail,
        Self::Cap,
        Self::Chip,
        Self::Crumb,
        Self::Button,
        Self::Field,
        Self::Slot,
        Self::Ledger,
        Self::Diff,
        Self::Prose,
        Self::Title,
        Self::Value,
        Self::Label,
        Self::IdentityRow,
        Self::Nav,
        Self::ThemeCard,
    ];
}
