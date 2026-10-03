//! The command palette: searchable verbs and destinations on each surface.

use super::binding::Binding;
use super::checkpoint_cmd::CheckpointCmd;
use super::dock_item::DockItem;
use super::loop_cmd::LoopCmd;
use super::loop_status::LoopStatus;
use super::model::Model;
use super::palette_row::{PaletteGo, PaletteRow};
use super::place::Place;

/// The command palette's query and selected result.
///
/// [`Palette::rows`] derives results from the current model so badges and
/// actions stay current on both terminal and native surfaces.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Palette {
    query: String,
    chosen: usize,
    choosing_track: bool,
}

impl Palette {
    pub(crate) fn tracks() -> Self {
        Self {
            choosing_track: true,
            ..Self::default()
        }
    }

    /// The purpose shown by every renderer.
    #[must_use]
    pub const fn title(&self) -> &'static str {
        if self.choosing_track {
            "Choose a track"
        } else {
            "Commands"
        }
    }

    /// Results the list is cut to — enough to choose from, few enough
    /// that ↑↓ stays a gesture rather than a scroll.
    const CUT: usize = 8;

    /// What has been typed.
    #[must_use]
    pub fn query(&self) -> &str {
        &self.query
    }

    /// Which result is chosen, an index into [`Palette::rows`].
    #[must_use]
    pub const fn chosen(&self) -> usize {
        self.chosen
    }

    /// Type one character into the query; the choice snaps back to the
    /// strongest match.
    pub(crate) fn type_char(&mut self, c: char) {
        self.query.push(c);
        self.chosen = 0;
    }

    /// Erase the last character; the choice snaps back likewise.
    pub(crate) fn backspace(&mut self) {
        self.query.pop();
        self.chosen = 0;
    }

    /// Walk the choice up.
    pub(crate) const fn up(&mut self) {
        self.chosen = self.chosen.saturating_sub(1);
    }

    /// Walk the choice down, clamped to the last of `len` rows.
    pub(crate) fn down(&mut self, len: usize) {
        self.chosen = (self.chosen + 1).min(len.saturating_sub(1));
    }

    /// The results for the model's open palette, strongest first —
    /// empty when the palette is closed or nothing matches.
    ///
    /// The universe is every place the model really keeps — the
    /// project, the altitudes, the launcher's sessions, the roadmap's
    /// chunks — and every verb in [`Binding::ALL`]. Places outrank
    /// verbs at equal score, which is "places, not menus".
    #[must_use]
    pub fn rows(model: &Model) -> Vec<PaletteRow> {
        let Some(palette) = model.palette() else {
            return Vec::new();
        };
        let query = palette.query();
        if palette.choosing_track {
            let mut rows: Vec<_> = model
                .roadmap
                .loop_tracks()
                .into_iter()
                .map(|track| {
                    let mut row = PaletteRow::place(
                        track.clone(),
                        PaletteGo::Launch(super::LaunchAction::Loop(super::LoopScope::Track(
                            track,
                        ))),
                    );
                    row.verb = "Loop on".into();
                    row
                })
                .chain(std::iter::once({
                    let mut row = PaletteRow::place(
                        "All tracks",
                        PaletteGo::Launch(super::LaunchAction::Loop(super::LoopScope::AllTracks)),
                    );
                    row.verb = "Loop on".into();
                    row
                }))
                .filter_map(|row| score(query, &row.haystack()).map(|score| (score, row)))
                .collect();
            rows.sort_by_key(|row| std::cmp::Reverse(row.0));
            return rows.into_iter().map(|(_, row)| row).collect();
        }
        let mut scored: Vec<(i32, usize, PaletteRow)> = Vec::new();
        let mut order = 0;
        let mut push = |row: PaletteRow| {
            let score = if query.is_empty() {
                Some(0)
            } else {
                score(query, &row.haystack())
            };
            if let Some(score) = score {
                scored.push((score, order, row));
                order += 1;
            }
        };
        if model.has_pixels() || model.surface().is_native() {
            push(PaletteRow::place("Settings", PaletteGo::Settings));
            push(PaletteRow::place("Search recorded work", PaletteGo::Search));
            push(PaletteRow::place("Keyboard help", PaletteGo::Help));
        }
        if model.uses_flight_recorder() || model.surface().is_native() {
            if super::flight_divider::FlightDivider::ALL
                .into_iter()
                .any(|d| d.visible(model))
            {
                let mut row = PaletteRow::place("Resize panels", PaletteGo::ResizePanels);
                row.verb = "Resize panels".into();
                row.name = None;
                push(row);
            }
            let mut row = PaletteRow::place("Reset panel sizes", PaletteGo::ResetPanels);
            row.verb = "Reset panel sizes".into();
            row.name = None;
            push(row);
        }
        {
            for tab in super::stage_tab::StageTab::ALL {
                let label = if model.has_pixels() || model.surface().is_native() {
                    match tab {
                        super::StageTab::Turn => "Conversation",
                        super::StageTab::Session => "Recorded work",
                        super::StageTab::Roadmap => "Roadmap",
                    }
                } else {
                    tab.label()
                };
                push(PaletteRow::place(label, PaletteGo::Tab(tab)));
            }
        }
        if let Some(project) = model.launcher().filter(|data| !data.project.is_empty()) {
            let mut row = PaletteRow::place(
                project.project.clone(),
                PaletteGo::Tab(super::stage_tab::StageTab::Roadmap),
            );
            row.sigil = true;
            let pending = model.dock().len();
            if pending > 0 {
                let plural = if pending == 1 { "" } else { "s" };
                row.badge = Some(format!("{pending} action{plural} required"));
            }
            if matches!(model.dock().front(), Some(DockItem::Permission(_))) {
                row.act = Some("Answer permission request".to_owned());
            }
            push(row);
        }
        // The other projects this window's host can open in place: the
        // same ask a press on the home page makes.
        for (index, project) in model.galaxy().projects().iter().enumerate() {
            if super::can_open_project(model, index) {
                let mut row = PaletteRow::place(project.name.clone(), PaletteGo::Project(index));
                row.verb = "Open project".to_owned();
                row.sigil = true;
                push(row);
            }
        }
        for (name, place) in [
            ("projects", Place::Galaxy),
            ("signals", Place::Signals),
            ("changes", Place::Changes),
            ("trace", Place::Trace),
            ("Report", Place::Report),
        ] {
            if model.surface().is_native()
                && (place != Place::Report
                    || model
                        .roadmap()
                        .selected()
                        .is_some_and(|chunk| chunk.state.is_done()))
            {
                push(PaletteRow::place(name, PaletteGo::Place(place)));
            }
        }
        if let Some(data) = model.launcher() {
            for (index, session) in data.sessions.iter().enumerate() {
                let mut row = PaletteRow::place(
                    format!("{} · {}", session.when, session.preview),
                    PaletteGo::Session(index),
                );
                row.verb = "Resume".to_owned();
                push(row);
            }
        }
        for chunk in model.roadmap().rows() {
            let name = if chunk.title.is_empty() {
                &chunk.name
            } else {
                &chunk.title
            };
            let mut row = PaletteRow::place(name.clone(), PaletteGo::Chunk(chunk.id.clone()));
            if chunk.is_in_progress() {
                row.badge = Some("in progress".to_owned());
            }
            push(row);
        }
        // The previewed checkpoint's verbs:
        // the rail reveals Restore · Fork on the row under the pointer;
        // a keyboard previews the row and finds the same two here.
        let checkpoints: Vec<_> = if model.uses_flight_recorder() {
            model
                .rail()
                .spine()
                .entries()
                .iter()
                .rev()
                .filter(|cp| matches!(cp.state(), super::CheckpointState::Saved { .. }))
                .collect()
        } else {
            model.rail().spine().previewed().into_iter().collect()
        };
        for cp in checkpoints {
            let name = cp.label().to_owned();
            for (verb, cmd) in [
                (
                    "Restore",
                    CheckpointCmd::Restore {
                        id: cp.id().to_owned(),
                    },
                ),
                (
                    "Fork",
                    CheckpointCmd::Fork {
                        id: cp.id().to_owned(),
                        branch: None,
                    },
                ),
            ] {
                let mut row = PaletteRow::place(name.clone(), PaletteGo::Checkpoint(cmd));
                row.verb = verb.to_owned();
                push(row);
            }
        }
        // The charter's offers: the rail proposes a
        // widening from the reader's record; the keyboard accepts it
        // here, the row naming the scope and the record it stands on.
        for offer in model.rail().offers() {
            let mut row = PaletteRow::place(
                format!("{} \u{b7} {}", offer.subject(), offer.tally()),
                PaletteGo::Delegate(offer.key().to_owned()),
            );
            row.verb = "Delegate".to_owned();
            push(row);
        }
        let paused = model.loop_status().is_some_and(LoopStatus::is_paused);
        for binding in Binding::ALL {
            // Kill is press-and-hold: a row is a press
            // with no release, and a hold nobody lets go of would
            // complete on the timer — so the sheet lists it and the
            // palette never dispatches it.
            if matches!(*binding, Binding::KILL | Binding::ROADMAP)
                || (model.uses_flight_recorder() && *binding == Binding::SHEET)
                || (!model.surface().is_native()
                    && matches!(
                        *binding,
                        Binding::RESUME | Binding::ZOOM | Binding::ZOOM_FIT
                    ))
            {
                continue;
            }
            // A hosted session starts an item or the loop in place; the
            // checks stay the launcher's, which hands them to the shell.
            let startable = model.hosts_work() && matches!(*binding, Binding::NEXT | Binding::LOOP);
            if matches!(*binding, Binding::NEXT | Binding::LOOP | Binding::GATES)
                && model.launcher().is_none()
                && !startable
            {
                continue;
            }
            let mut row = PaletteRow::binding(
                if *binding == Binding::PALETTE && model.surface().is_native() {
                    Binding::PALETTE_CHORD
                } else {
                    *binding
                },
            );
            if *binding == Binding::NEXT {
                row.go = PaletteGo::Launch(super::launch_action::LaunchAction::Next);
            } else if *binding == Binding::LOOP {
                row.go = PaletteGo::ChooseLoop;
            } else if *binding == Binding::GATES {
                row.go = PaletteGo::Launch(super::launch_action::LaunchAction::Gates);
            }
            // The loop's verbs go as verbs, never as their keys: `s`
            // dispatched while the loop runs would be the prompt's
            // letter, and a row that types is not a row that steps.
            if *binding == Binding::PAUSE {
                row.go = PaletteGo::Loop(if paused {
                    LoopCmd::Resume
                } else {
                    LoopCmd::Pause
                });
            } else if *binding == Binding::STEP {
                row.go = PaletteGo::Loop(LoopCmd::Step);
            } else if *binding == Binding::SKIP {
                row.go = PaletteGo::Loop(LoopCmd::Skip);
            } else if *binding == Binding::ABORT {
                // A window's Esc climbs: the row wears
                // the chord the window really takes.
                if model.surface().is_native() {
                    row = PaletteRow::binding(Binding::ABORT_CHORD);
                }
                row.go = PaletteGo::Loop(LoopCmd::Abort);
            } else if *binding == Binding::CHECKPOINT_NOW {
                row.go = PaletteGo::CheckpointNow;
            } else if *binding == Binding::DOCK && model.surface().is_native() {
                // The window's dock verb is the chord:
                // the row wears ⌘↵ and dispatches it.
                row = PaletteRow::binding(Binding::DOCK_CHORD);
            }
            push(row);
        }
        scored.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
        scored
            .into_iter()
            .take(Self::CUT)
            .map(|(_, _, row)| row)
            .collect()
    }
}

/// A subsequence match of `query` in `haystack`, scored the way the
/// command-palette matchers score (VS Code's rules, small): every query
/// character must appear in order; a hit at the very start, at a word
/// boundary, or adjacent to the previous hit earns a bonus, and every
/// gap costs a little, so `thi` prefers `think-and-ship` over a word
/// that merely contains those letters. `None` when the query is not a
/// subsequence at all.
fn score(query: &str, haystack: &str) -> Option<i32> {
    let haystack: Vec<char> = haystack.chars().flat_map(char::to_lowercase).collect();
    let mut total = 0i32;
    let mut at = 0usize;
    let mut last_hit: Option<usize> = None;
    for q in query.chars().flat_map(char::to_lowercase) {
        let found = haystack[at..].iter().position(|&h| h == q)? + at;
        total += 1;
        if found == 0 {
            total += 8;
        } else if !haystack[found - 1].is_alphanumeric() {
            total += 6;
        }
        if last_hit == Some(found.wrapping_sub(1)) {
            total += 4;
        }
        total -= i32::try_from(found - at).unwrap_or(i32::MAX) / 4;
        last_hit = Some(found);
        at = found + 1;
    }
    Some(total)
}

#[cfg(test)]
#[path = "palette.tests.rs"]
mod tests;
