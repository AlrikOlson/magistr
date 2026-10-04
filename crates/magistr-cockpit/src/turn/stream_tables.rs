//! Conversation tables use the same labeled cells and measuring contract as documents.
use super::Turn;
use crate::pixel::Typography;
use crate::turn::{LineKind, TextDocument};

impl Turn {
    pub(super) fn refresh_tables(&mut self, from: usize) {
        let mut at = from.min(self.lines.len());
        while at > 0 && self.lines[at - 1].text.contains('|') && !self.lines[at - 1].fenced {
            at -= 1;
        }
        while at + 1 < self.lines.len() {
            if self.lines[at].fenced
                || !matches!(self.lines[at].kind, LineKind::Prose(_))
                || !TextDocument::is_table_start(&self.lines[at].text, &self.lines[at + 1].text)
            {
                at += 1;
                continue;
            }
            let start = at;
            let voice = self.lines[start].kind.clone();
            at += 2;
            while at < self.lines.len()
                && self.lines[at].kind == voice
                && !self.lines[at].fenced
                && self.lines[at].text.contains('|')
            {
                at += 1;
            }
            let end = at;
            let source = self.lines[start..end]
                .iter()
                .map(|line| line.text.as_str())
                .collect::<Vec<_>>()
                .join("\n");
            let document = TextDocument::markdown(source, self.mode);
            let mut face = Typography::default();
            face.set_cell(self.cell);
            let width = self
                .pixel_wrap
                .map_or(f32::from(self.width), |policy| policy.width as f32);
            let rows = document.rows(width, &mut |text, role| {
                if self.pixel_wrap.is_some() {
                    face.advance(text, role)
                } else {
                    f32::from(crate::render::width(text))
                }
            });
            let old_start = self.lines[start].row_start;
            let old_end = self
                .lines
                .get(end)
                .map_or(self.rows.len(), |line| line.row_start);
            let mut fresh = Vec::new();
            for index in start..end {
                self.lines[index].row_start = old_start + fresh.len();
                if index == start && end == start + 2 {
                    fresh.extend(rows.iter().map(|row| row.row.clone()));
                } else if index >= start + 2 {
                    fresh.extend(
                        rows.iter()
                            .filter(|row| row.anchor.line == index - start)
                            .map(|row| row.row.clone()),
                    );
                }
            }
            let had = old_end - old_start;
            let count = fresh.len();
            self.rows.splice(old_start..old_end, fresh);
            for line in &mut self.lines[end..] {
                line.row_start = (line.row_start + count).saturating_sub(had);
            }
            self.layout = self.layout.wrapping_add(1);
            self.rebuilt = count;
            self.conversation_dirty = true;
        }
    }
}

#[cfg(test)]
#[path = "stream_tables.tests.rs"]
mod tests;
