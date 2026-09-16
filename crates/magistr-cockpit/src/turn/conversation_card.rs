//! Readable summaries of recorded work in the conversation.
use super::{Line, LineKind, Turn};
use crate::agent::ToolStatus;
use crate::layout::{TextMetrics, TextRole};
use crate::pixel::Typography;

/// A card uses only facts present in the source record.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConversationCard {
    pub title: String,
    pub detail: String,
    pub status: String,
    pub failed: bool,
    pub running: bool,
}

impl ConversationCard {
    #[must_use]
    pub fn of(turn: &Turn, line: &Line) -> Option<Self> {
        let (title, detail, status, failed, running) = match line.kind() {
            LineKind::Tool { status, .. } => (
                line.text().to_owned(),
                line.record()
                    .locations
                    .first()
                    .map_or_else(String::new, |(path, _)| path.display().to_string()),
                status_label(*status).to_owned(),
                *status == ToolStatus::Failed,
                matches!(status, ToolStatus::Pending | ToolStatus::InProgress),
            ),
            LineKind::Edit {
                id,
                path,
                status,
                undone,
                ..
            } => (
                if *undone {
                    "File restored"
                } else {
                    line.title().unwrap_or("Update file")
                }
                .to_owned(),
                {
                    let summary = line
                        .record()
                        .text
                        .first()
                        .and_then(|text| text.lines().next())
                        .unwrap_or_default();
                    if summary.is_empty() {
                        path.clone()
                    } else {
                        format!("{path}\n{summary}")
                    }
                },
                turn.diff_stats(id).map_or_else(
                    || status_label(*status).to_owned(),
                    |(added, removed)| format!("+{added} −{removed}"),
                ),
                *status == ToolStatus::Failed,
                matches!(status, ToolStatus::Pending | ToolStatus::InProgress),
            ),
            LineKind::Check { name, passed, .. } => (
                name.clone(),
                line.text().lines().next().unwrap_or_default().to_owned(),
                if *passed { "Passed" } else { "Failed" }.to_owned(),
                !passed,
                false,
            ),
            _ => return None,
        };
        Some(Self {
            title,
            detail,
            status,
            failed,
            running,
        })
    }

    #[must_use]
    pub fn status_width(&self, face: &Typography) -> f32 {
        let width = face.advance(&self.status, TextRole::Label);
        if self.running {
            width.max(face.advance("No result reported", TextRole::Label))
        } else {
            width
        }
    }

    #[must_use]
    pub fn heights(&self, face: &Typography, width: f32) -> (usize, usize) {
        let status = self.status_width(face) + 20.0;
        let title = lines(&self.title, face, TextRole::Heading, width - 40.0 - status);
        let detail = if self.detail.is_empty() {
            0
        } else {
            lines(&self.detail, face, TextRole::Prose, width - 40.0)
        };
        let line = usize::from(face.cell().height);
        (title * line, detail * line)
    }
}

fn lines(text: &str, face: &Typography, role: TextRole, width: f32) -> usize {
    TextMetrics::new(text)
        .wrap_by(width.max(1.0), &mut |run| face.advance(run, role))
        .len()
        .max(1)
}

fn status_label(status: ToolStatus) -> &'static str {
    match status {
        ToolStatus::Pending => "Waiting",
        ToolStatus::InProgress => "Working",
        ToolStatus::Completed => "Done",
        ToolStatus::Failed => "Failed",
    }
}
