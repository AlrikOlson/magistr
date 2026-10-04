//! Physical conversation geometry over the unchanged source-row identities.
use super::{ConversationCard, ConversationRow, LineKind, RowKind, Turn};
use crate::pixel::{CellSize, Typography};

/// A measured projection; source rows remain the authority for actions and anchors.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ConversationLayout {
    rows: Vec<ConversationRow>,
    height: usize,
}

impl ConversationLayout {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            rows: Vec::new(),
            height: 0,
        }
    }

    #[must_use]
    pub fn of(turn: &Turn, width: f32, cell: CellSize, compact: bool) -> Self {
        let mut layout = Self::new();
        let mut face = Typography::default();
        face.set_cell(cell);
        let line_height = usize::from(cell.height);
        let mut previous_voice = None;
        for (index, line) in turn.lines().iter().enumerate() {
            let start = line.row_start();
            let end = turn
                .lines()
                .get(index + 1)
                .map_or(turn.len(), |next| next.row_start());
            if matches!(line.kind(), LineKind::Event) {
                // Timeline events remain addressable in Records and retain their source
                // positions; they take no physical space in the conversation.
                for source in start..end {
                    layout.rows.push(ConversationRow {
                        source,
                        line: index,
                        top: layout.height,
                        height: 0,
                        before: 0,
                        after: 0,
                        author: None,
                        card_head: false,
                    });
                }
                continue;
            }
            let voice = match line.kind() {
                LineKind::Prose(voice) => Some(*voice),
                _ => None,
            };
            let author = voice.filter(|voice| Some(*voice) != previous_voice);
            let card = ConversationCard::of(turn, line);
            for source in start..end {
                let first = source == start;
                let last = source + 1 == end;
                let author = first.then_some(author).flatten();
                let before = if compact && author == Some(super::Voice::User) {
                    0
                } else if author.is_some() {
                    line_height
                        + if compact {
                            8
                        } else {
                            8 + if source == 0 { 0 } else { 8 }
                        }
                } else {
                    0
                };
                let after = if last
                    && (card.is_some()
                        || (voice.is_some()
                            && !matches!(turn.rows()[source].kind, RowKind::Code | RowKind::Fence)))
                {
                    8
                } else {
                    0
                };
                let after = if source + 1 == turn.len() {
                    0
                } else if compact {
                    after.min(4)
                } else {
                    after
                };
                let body = if first {
                    card.as_ref().map(|card| {
                        let (title, detail) = card.heights(&face, width);
                        if compact {
                            title + 8
                        } else {
                            title + detail + 24 + usize::from(detail > 0) * 6
                        }
                    })
                } else {
                    None
                }
                .unwrap_or_else(|| {
                    if turn.rows()[source].text.is_empty()
                        && matches!(turn.rows()[source].kind, RowKind::Prose(_))
                    {
                        8
                    } else {
                        line_height
                    }
                });
                let height = before + body + after;
                layout.rows.push(ConversationRow {
                    source,
                    line: index,
                    top: layout.height,
                    height,
                    before,
                    after,
                    author,
                    card_head: first && card.is_some(),
                });
                layout.height = layout.height.saturating_add(height);
            }
            if start < end {
                previous_voice = voice;
            }
        }
        layout
    }

    #[must_use]
    pub fn rows(&self) -> &[ConversationRow] {
        &self.rows
    }

    #[must_use]
    pub const fn height(&self) -> usize {
        self.height
    }

    #[must_use]
    pub fn row_at(&self, pixel: usize) -> usize {
        self.rows
            .partition_point(|row| row.top + row.height <= pixel)
            .min(self.rows.len().saturating_sub(1))
    }

    #[must_use]
    pub fn offset(&self, source: usize) -> usize {
        self.rows.get(source).map_or(self.height, |row| row.top)
    }
}
