//! What a text leaf *is*, as distinct from how it is coloured.

/// The kind of text a leaf carries — the hint a renderer with real
/// typography reads to choose a face and a size.
///
/// # Why this is not a style
///
/// A [`Style`](crate::render::Style) says what colour a run is drawn in
/// and which attributes it wears. It cannot say whether a run is a
/// sentence or a file path, and on a cell grid it never needed to: every
/// glyph is one column wide, so prose and a path are typeset
/// identically. A window has faces, and the choice between them is a
/// question about the *content*, not about the pen.
///
/// # Why the terminal is safe
///
/// The role never reaches [`Node::flex_style`], and the cell painter
/// ([`runtime::paint`](crate::render::paint)) never reads it either. It
/// does reach the layout engine at exactly one seam — the
/// [`Measure`](super::Measure) a surface hands
/// [`LayoutTree::compute_on`](super::LayoutTree::compute_on), which
/// answers how wide a run is and how tall its rows are — and the
/// terminal's [`CellMeasure`](super::CellMeasure) *ignores the role*: a
/// column is a column and a row is one cell whatever a leaf claims to
/// be. So tagging a leaf cannot move a terminal frame by one byte, and
/// that is still a property of the *wiring* rather than of a golden
/// agreeing: `the_cell_painter_is_blind_to_the_semantic_hints` pins the
/// paint and `the_role_changes_nothing_on_a_terminal` pins the
/// measure.
///
/// The enum currently defaults to Data. New text leaves use Label:
/// interface words are proportional in the graphics renderer. Code, diffs,
/// aligned tables and ready-made cell bands select Data explicitly, preserving
/// their column alignment. The plain cell renderer ignores these roles.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum TextRole {
    /// Column-aligned content: code, diffs, paths, ids, numerals,
    /// gauges, chips and every ready-made cell band. Set in the mono
    /// face at the cell advance, so a column is a column.
    #[default]
    Data,
    /// Sentences meant to be read: an agent's message, a chunk's
    /// description, a note.
    Prose,
    /// A ledger entry's name — the chrome's own words where they are a
    /// *name* rather than a measurement: a tab name, a key hint's
    /// description, a button's verb, a checkpoint's label.
    Label,
    /// The composer's own placeholder and the words a reader types
    /// over: one step up from prose's neighbours, one step under it.
    Steer,
    /// A section's name: a panel title, a group divider, a tab's
    /// heading row.
    Heading,
    /// A mode badge — the smallest word the design sets, tracked so a
    /// three-letter shout stays readable inside its pill.
    Badge,
    /// A section cap: tracked, uppercased, the quietest label a panel
    /// wears above what it holds.
    Cap,
    /// A ledger row's meta — the smallest untracked mono voice, for the
    /// `now` and the `2 receipts after cp-4` a name is annotated with.
    Fine,
    /// A chip, a meter's readout, a phase in the bottom bar: the small
    /// mono voice most of the chrome's measurements are set in.
    Small,
    /// A diff line — the small mono voice at the code leading, which is
    /// the one thing that separates it from [`TextRole::Small`].
    Code,
    /// A breadcrumb, a charter rule, a value beside its key: the
    /// largest of the mono voices, where a measurement is read rather
    /// than glanced at.
    Value,
}

impl TextRole {
    /// Every voice, in the small-to-large order the design's scale runs
    /// in, with [`TextRole::Data`] — the one voice the *grid* sizes —
    /// first.
    ///
    /// Named here so a sweep (a metrics table, a blindness test, a
    /// mapping onto the oracle) visits the whole vocabulary rather
    /// than a list that drifts from it.
    pub const ALL: [Self; 11] = [
        Self::Data,
        Self::Badge,
        Self::Cap,
        Self::Fine,
        Self::Small,
        Self::Code,
        Self::Value,
        Self::Label,
        Self::Steer,
        Self::Prose,
        Self::Heading,
    ];

    /// Whether this role is set in the mono face.
    ///
    /// Named rather than matched at each site, so a new voice is a new
    /// variant and one arm here instead of a `match` to find in a
    /// painter.
    #[must_use]
    pub const fn is_mono(self) -> bool {
        matches!(
            self,
            Self::Data
                | Self::Badge
                | Self::Cap
                | Self::Fine
                | Self::Small
                | Self::Code
                | Self::Value
        )
    }

    /// Whether this role is measured by **counting columns** off the
    /// cell rather than by shaping.
    ///
    /// This was `is_mono`'s second job until the small scale arrived,
    /// and the two had to come apart: a badge and a diff line are set
    /// in the mono face *at their own size*, so counting their columns
    /// off the cell would measure them in a size they are not drawn
    /// in. Only [`TextRole::Data`] is grid content — a gauge, a
    /// sparkline, a ready-made cell band, a `{:<15}` key cap — and only
    /// its width is the cell's.
    #[must_use]
    pub const fn is_counted(self) -> bool {
        matches!(self, Self::Data)
    }
}

#[cfg(test)]
#[path = "text_role.tests.rs"]
mod tests;
