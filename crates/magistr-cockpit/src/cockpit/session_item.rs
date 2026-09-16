//! Timestamp-ordered projection of the session's independent data models.
use super::{Changes, Selected, ShipMoment, Trace, TraceStep};
use crate::workbench::{JournalEntry, RunRow};
use chrono::{DateTime, Utc};

/// One timeline row, borrowing its owning projection. Indices are hit targets.
#[derive(Clone, Copy, Debug)]
pub enum SessionItem<'a> {
    Step(usize, &'a TraceStep),
    Moment(usize, &'a ShipMoment),
    Change(usize, &'a JournalEntry),
    Run(usize, &'a RunRow),
}

impl<'a> SessionItem<'a> {
    /// Recorded instants first, with stable source order for ties. Legacy rows
    /// with missing/invalid stamps follow, visibly marked as unknown.
    #[must_use]
    pub fn collect(trace: &'a Trace, changes: &'a Changes) -> Vec<Self> {
        let mut rows: Vec<_> = trace
            .steps()
            .iter()
            .enumerate()
            .map(|(i, s)| Self::Step(i, s))
            .collect();
        rows.extend(
            trace
                .moments()
                .iter()
                .enumerate()
                .map(|(i, m)| Self::Moment(i, m)),
        );
        rows.extend(
            changes
                .entries()
                .iter()
                .enumerate()
                .map(|(i, c)| Self::Change(i, c)),
        );
        if let Some((_, runs)) = changes.runs() {
            rows.extend(runs.iter().enumerate().map(|(i, r)| Self::Run(i, r)));
        }
        rows.sort_by_cached_key(|row| {
            let instant = row.instant();
            (instant.is_none(), instant)
        });
        rows
    }

    /// Stable object identity, independent of the row's current position.
    #[must_use]
    pub fn selection(&self) -> Selected {
        match self {
            Self::Step(_, step) => Selected::Step(step.number),
            Self::Change(_, change) => Selected::Change(change.identity()),
            Self::Moment(_, ShipMoment::Action { task_id, id, .. }) => {
                Selected::Receipt(format!("action:{task_id}:{id}"))
            }
            Self::Moment(
                _,
                ShipMoment::Check {
                    task_id,
                    name,
                    timestamp,
                    ..
                },
            ) => Selected::Receipt(format!("check:{task_id}:{timestamp}:{name}")),
            Self::Run(index, _) => Selected::Run(*index),
        }
    }

    /// The original timestamp normalized to UTC, without inventing arrival time.
    #[must_use]
    pub fn instant(&self) -> Option<DateTime<Utc>> {
        let stamp = match self {
            Self::Step(_, step) => step.timestamp.as_deref()?,
            Self::Moment(
                _,
                ShipMoment::Action { timestamp, .. } | ShipMoment::Check { timestamp, .. },
            ) => timestamp,
            Self::Change(_, change) => return DateTime::from_timestamp_millis(change.at_ms?),
            Self::Run(_, _) => return None,
        };
        DateTime::parse_from_rfc3339(stamp)
            .ok()
            .map(|stamp| stamp.with_timezone(&Utc))
    }

    /// Compact UTC clock for rows; the card can show the full source timestamp.
    #[must_use]
    pub fn time_label(&self) -> String {
        self.instant().map_or_else(
            || "time unknown".into(),
            |stamp| stamp.format("%H:%M:%S").to_string(),
        )
    }
}

#[cfg(test)]
#[path = "session_item.tests.rs"]
mod tests;
