//! Track membership, counts and browsing state change together: every surface
//! navigates the same derived headings without changing selected work.
use super::{ChunkState, Readiness, Roadmap, RoadmapChunk};
use crate::cockpit::{SignalRow, Widget};
use crate::turn::Viewport;
use crate::widget_id::WidgetId;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TrackFocus {
    Track(String),
    Item(String),
    Signal(String),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TrackCounts {
    pub ready: usize,
    pub working: usize,
    pub waiting: usize,
    pub blocked: usize,
    pub backlog: usize,
}
impl TrackCounts {
    #[must_use]
    pub fn summary(self) -> String {
        format!(
            "Ready {} · In progress {} · Waiting {} · Blocked {} · Backlog {}",
            self.ready, self.working, self.waiting, self.blocked, self.backlog
        )
    }
    #[must_use]
    pub fn lines(self) -> [String; 2] {
        [
            format!(
                "Ready {} · In progress {} · Waiting {}",
                self.ready, self.working, self.waiting
            ),
            format!("Blocked {} · Backlog {}", self.blocked, self.backlog),
        ]
    }
}

#[derive(Clone, Debug)]
pub enum TrackRow<'a> {
    Heading {
        key: &'a str,
        expanded: bool,
        counts: TrackCounts,
    },
    Item(&'a RoadmapChunk, usize),
    Signal(usize, &'a SignalRow),
}
impl TrackRow<'_> {
    #[must_use]
    pub fn label(&self) -> &str {
        match self {
            Self::Heading { key, .. } => track_name(key),
            Self::Item(chunk, _) => &chunk.name,
            Self::Signal(_, signal) => signal.title(),
        }
    }
    #[must_use]
    pub fn counts(&self) -> Option<TrackCounts> {
        match self {
            Self::Heading { counts, .. } => Some(*counts),
            _ => None,
        }
    }
    #[must_use]
    pub fn focus(&self) -> TrackFocus {
        match self {
            Self::Heading { key, .. } => TrackFocus::Track((*key).into()),
            Self::Item(chunk, _) => TrackFocus::Item(chunk.id.clone()),
            Self::Signal(_, signal) => TrackFocus::Signal(signal.id.clone()),
        }
    }
    #[must_use]
    pub fn widget(&self, index: usize) -> WidgetId {
        match self {
            Self::Heading { .. } => Widget::roadmap_track(index),
            Self::Item(_, item) => Widget::roadmap_row(*item),
            Self::Signal(index, _) => Widget::signal_row(*index),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(in crate::cockpit) struct TrackBrowser {
    pub collapsed: BTreeSet<String>,
    pub focus: Option<TrackFocus>,
    pub track: Option<String>,
    pub query: String,
    pub editing: bool,
    pub dependencies: bool,
    pub saved_focus: Option<TrackFocus>,
    pub saved_scroll: Option<Viewport>,
}

fn key(chunk: &RoadmapChunk) -> &str {
    chunk.group.as_deref().map(str::trim).unwrap_or("")
}
fn track_name(key: &str) -> &str {
    if key.is_empty() { "Unassigned" } else { key }
}

impl Roadmap {
    #[must_use]
    pub fn track_rows(&self) -> Vec<TrackRow<'_>> {
        let mut groups: BTreeMap<&str, Vec<(usize, &RoadmapChunk)>> = BTreeMap::new();
        for (index, chunk) in self.rows().into_iter().enumerate() {
            groups.entry(key(chunk)).or_default().push((index, chunk));
        }
        let mut rows = Vec::new();
        for (group, mut items) in groups {
            let mut counts = TrackCounts::default();
            for chunk in self.chunks.iter().filter(|chunk| key(chunk) == group) {
                match self.readiness(chunk) {
                    Readiness::Ready => counts.ready += 1,
                    Readiness::Blocked(_) => counts.blocked += 1,
                    Readiness::Waiting(_) => counts.waiting += 1,
                    Readiness::Inactive(_) => match chunk.state {
                        ChunkState::InProgress => counts.working += 1,
                        ChunkState::Stuck | ChunkState::Blocked => counts.blocked += 1,
                        ChunkState::Backlog => counts.backlog += 1,
                        _ => {}
                    },
                }
            }
            // Stable track order is independent of item sorting; working items lead.
            items.sort_by_key(|(_, chunk)| !chunk.is_in_progress());
            let expanded = !self.browse.collapsed.contains(group) || !self.browse.query.is_empty();
            rows.push(TrackRow::Heading {
                key: group,
                expanded,
                counts,
            });
            if expanded {
                rows.extend(
                    items
                        .into_iter()
                        .map(|(index, chunk)| TrackRow::Item(chunk, index)),
                );
            }
        }
        rows.extend(
            self.signals()
                .into_iter()
                .enumerate()
                .filter(|(_, signal)| {
                    self.browse.query.is_empty()
                        || signal
                            .title()
                            .to_lowercase()
                            .contains(&self.browse.query.to_lowercase())
                })
                .map(|(index, signal)| TrackRow::Signal(index, signal)),
        );
        rows
    }
    #[must_use]
    pub fn track_focus(&self) -> Option<&TrackFocus> {
        self.browse.focus.as_ref()
    }
    pub fn focus_track_row(&mut self, index: usize) {
        if let Some(row) = self.track_rows().get(index) {
            self.browse.focus = Some(row.focus());
        }
    }
    #[must_use]
    pub fn focused_track_index(&self) -> Option<usize> {
        self.track_rows()
            .iter()
            .position(|row| Some(&row.focus()) == self.track_focus())
    }
    pub fn move_track_focus(&mut self, delta: isize) {
        let len = self.track_rows().len();
        if len == 0 {
            self.browse.focus = None;
            return;
        }
        let next = match self.focused_track_index() {
            Some(index) => index.saturating_add_signed(delta).min(len - 1),
            None => {
                if delta == isize::MAX {
                    len - 1
                } else {
                    0
                }
            }
        };
        self.focus_track_row(next);
    }
    pub fn toggle_focused_track(&mut self) {
        if let Some(TrackFocus::Track(key)) = self.browse.focus.as_ref()
            && !self.browse.collapsed.remove(key)
        {
            self.browse.collapsed.insert(key.clone());
        }
    }
    pub fn branch_track_focus(&mut self, expand: bool) {
        let focused = self.browse.focus.clone();
        match focused {
            Some(TrackFocus::Track(key)) => {
                if expand {
                    if !self.browse.collapsed.remove(&key) {
                        self.move_track_focus(1);
                    }
                } else {
                    self.browse.collapsed.insert(key);
                }
            }
            Some(TrackFocus::Item(id)) if !expand => {
                if let Some(chunk) = self.chunk(&id) {
                    self.browse.focus = Some(TrackFocus::Track(key(chunk).into()));
                }
            }
            _ => {}
        }
    }
    #[must_use]
    pub fn track_filter_label(&self) -> &str {
        self.browse
            .track
            .as_deref()
            .map_or("All tracks", track_name)
    }
    pub fn cycle_track_filter(&mut self) {
        let groups: BTreeSet<_> = self
            .chunks
            .iter()
            .map(|chunk| key(chunk).to_owned())
            .collect();
        self.browse.track = match self.browse.track.as_ref() {
            None => groups.first().cloned(),
            Some(current) => groups
                .range::<String, _>((
                    std::ops::Bound::Excluded(current),
                    std::ops::Bound::Unbounded,
                ))
                .next()
                .cloned(),
        };
    }
    #[must_use]
    pub fn track_query(&self) -> &str {
        &self.browse.query
    }
    #[must_use]
    pub fn is_track_search_editing(&self) -> bool {
        self.browse.editing
    }
    pub fn set_track_query(&mut self, query: String) {
        if self.browse.query.is_empty() && !query.is_empty() {
            self.browse.saved_focus = self.browse.focus.clone();
        }
        if !self.browse.query.is_empty() && query.is_empty() {
            self.browse.focus = self.browse.saved_focus.take();
        }
        self.browse.query = query;
    }
    #[must_use]
    pub fn is_dependencies_view(&self) -> bool {
        self.browse.dependencies
    }
    pub fn toggle_dependencies_view(&mut self) {
        self.browse.dependencies = !self.browse.dependencies;
    }
    #[must_use]
    pub fn is_selected_visible(&self) -> bool {
        self.selected.as_ref().is_none_or(|id| {
            self.track_rows()
                .iter()
                .any(|row| matches!(row, TrackRow::Item(chunk, _) if &chunk.id == id))
        })
    }
    pub fn reveal_selected(&mut self) -> bool {
        let Some(chunk) = self.selected() else {
            return false;
        };
        let (id, group, state) = (chunk.id.clone(), key(chunk).to_owned(), chunk.state);
        self.browse.track = None;
        self.set_track_query(String::new());
        self.filter = if state.is_done() || state == ChunkState::Obsoleted {
            Some(state)
        } else {
            None
        };
        self.browse.collapsed.remove(&group);
        self.browse.focus = Some(TrackFocus::Item(id));
        true
    }
    #[must_use]
    pub fn dependents(&self, id: &str) -> Vec<&RoadmapChunk> {
        self.chunks
            .iter()
            .filter(|chunk| chunk.deps.iter().any(|dep| dep == id))
            .collect()
    }
    #[must_use]
    pub fn track_of<'a>(&self, chunk: &'a RoadmapChunk) -> &'a str {
        track_name(key(chunk))
    }
}

#[cfg(test)]
#[path = "roadmap_tracks.tests.rs"]
mod tests;
