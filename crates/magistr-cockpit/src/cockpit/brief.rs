//! What happened while you were away, dealt worst first.

use std::time::Duration;

use super::brief_body::BriefBody;
use super::brief_card::BriefCard;
use super::model::Model;
use super::star_state::StarState;
use super::widget::Widget;
use crate::theme::Severity;

/// The catch-up overlay: elapsed time away, outstanding work, and recent results.
///
/// Cards are derived from roadmap state, workbench changes, completed chunks,
/// and budget data on each frame. Only the time away is supplied by the entry
/// point through [`Model::with_away`](super::Model::with_away).
///
/// Cards follow [`Severity::ALL`] order. Severity sets their background and
/// icon color; it does not add a border.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Brief {
    away: String,
    cards: Vec<BriefCard>,
}

impl Brief {
    /// The Brief for `model` after `away`, or `None` when nothing
    /// happened worth stopping a person for.
    ///
    /// "Nothing" means *no card at all*: the spend is carried for the
    /// record and never on its own, so a cockpit opened on a quiet
    /// project goes straight to the sky rather than raising an overlay
    /// to say there is no news.
    #[must_use]
    pub fn of(model: &Model, away: Duration) -> Option<Self> {
        let mut cards: Vec<BriefCard> = [
            Self::stuck(model),
            Self::review(model),
            Self::shipped(model),
        ]
        .into_iter()
        .flatten()
        .collect();
        if cards.is_empty() {
            return None;
        }
        cards.extend(Self::spend(model));
        // Worst first. `Severity` is `Ord` in exactly that order, so
        // the deal is the band's own ordering rather than a table this
        // function keeps beside it.
        cards.sort_by_key(BriefCard::severity);
        Some(Self {
            away: elapsed(away),
            cards,
        })
    }

    /// How long you were away — `6 h 12 m`.
    #[must_use]
    pub fn away(&self) -> &str {
        &self.away
    }

    /// The cards, worst first.
    #[must_use]
    pub fn cards(&self) -> &[BriefCard] {
        &self.cards
    }

    /// The overlay's headline — `6 h 12 m · 4 items`.
    #[must_use]
    pub fn headline(&self) -> String {
        format!("{} · {} items", self.away, self.cards.len())
    }

    /// The alert card: the chunks the sky draws with the red ring
    /// ([`StarState::alerts`]), so the Brief and the constellation
    /// cannot disagree about what is stopped.
    fn stuck(model: &Model) -> Option<BriefCard> {
        let stopped: Vec<&super::roadmap_chunk::RoadmapChunk> = model
            .roadmap()
            .rows()
            .into_iter()
            .filter(|chunk| StarState::of(&chunk.status, false).alerts())
            .collect();
        let first = stopped.first()?;
        Some(BriefCard::new(
            Severity::Alert,
            format!("Stuck · {}", stopped.len()),
            BriefBody::Act {
                text: format!("{} requires a decision: {}.", first.id, first.name),
                verb: "Answer",
                widget: Widget::BriefAnswer,
            },
        ))
    }

    /// The attention card: the workbench journal's changes that have
    /// not been approved yet, with the gates' verdict when the run that
    /// produced them left one.
    fn review(model: &Model) -> Option<BriefCard> {
        let waiting: Vec<&crate::workbench::JournalEntry> = model
            .changes()
            .entries()
            .iter()
            .filter(|entry| !model.changes().is_approved(&entry.id))
            .collect();
        if waiting.is_empty() {
            return None;
        }
        let chunk = model.rail().chunk().unwrap_or("the working set").to_owned();
        let files = waiting.len();
        let mut text = format!("{chunk} is ready for review. {files} files.");
        if let Some(gates) = gates_verdict(model) {
            text.push(' ');
            text.push_str(gates);
        }
        Some(BriefCard::new(
            Severity::Attention,
            "Review",
            BriefBody::Act {
                text,
                verb: "Review",
                widget: Widget::BriefReview,
            },
        ))
    }

    /// The good card: what shipped, read off the roadmap's own log of
    /// the `chunk_completed` frames it folded.
    fn shipped(model: &Model) -> Option<BriefCard> {
        let shipped = model.roadmap().shipped();
        if shipped.is_empty() {
            return None;
        }
        Some(BriefCard::new(
            Severity::Good,
            format!("Done · {}", shipped.len()),
            BriefBody::Act {
                text: format!("{}. Merged.", shipped.join(", ")),
                verb: "View reports",
                widget: Widget::BriefReports,
            },
        ))
    }

    /// The quiet card: the bound budget as a meter, capped with how
    /// many sessions the launcher listed. Absent when nothing bound a
    /// budget — a meter to nowhere is furniture.
    fn spend(model: &Model) -> Option<BriefCard> {
        let budget = model.rail().budget()?;
        let sessions = model.launcher().map_or(0, |data| data.sessions.len());
        Some(BriefCard::new(
            Severity::Quiet,
            format!("Spend · {sessions} sessions"),
            BriefBody::Measure {
                readout: format!("{} USD", budget.readout()),
                fill: budget.fill_per_mille(),
                tick: budget.tick_per_mille(),
            },
        ))
    }
}

/// `Gates passed.` when every gate the run recorded is green, `Gates
/// failed.` when one is not, and nothing at all when no gate ran — a
/// clause the card only earns by having a verdict to report.
fn gates_verdict(model: &Model) -> Option<&'static str> {
    Some(match model.loop_status()?.gates_passed()? {
        true => "Gates passed.",
        false => "Gates failed.",
    })
}

/// A gap in the copy laws' duration units — `6 h 12 m`, `12 m`, `2 d
/// 3 h`. The coarsest two units that carry it, so the headline stays a
/// glance.
fn elapsed(away: Duration) -> String {
    let minutes = away.as_secs() / 60;
    let (hours, minutes) = (minutes / 60, minutes % 60);
    let (days, hours) = (hours / 24, hours % 24);
    match (days, hours) {
        (0, 0) => format!("{minutes} m"),
        (0, _) => format!("{hours} h {minutes} m"),
        _ => format!("{days} d {hours} h"),
    }
}

#[cfg(test)]
#[path = "brief.tests.rs"]
mod tests;
