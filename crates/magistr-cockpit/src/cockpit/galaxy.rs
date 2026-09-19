//! The Galaxy sub-state: the projects, the selection and the zoom.

use super::galaxy_data::GalaxyData;
use super::galaxy_project::GalaxyProject;

/// The Galaxy as the model keeps it: the
/// data its job delivered, which cluster the keys have walked onto, and
/// how far the wheel has zoomed the field. Native-only state in the
/// shared model, like [`Place`](super::Place): the terminal has no
/// altitude 0 and never reads it.
///
/// Placement is not here. Where a cluster sits and what its stars look
/// like is the surface's derivation from the project's numbers, so the
/// same data draws the same picture on any window.
#[derive(Clone, Debug, PartialEq)]
pub struct Galaxy {
    data: Option<GalaxyData>,
    current: Option<GalaxyProject>,
    selected: Option<usize>,
    zoom: f32,
    /// The directory of the project the host was asked to open and has
    /// not answered for yet.
    opening: Option<String>,
    /// The last refused open: the directory and why, in words.
    refusal: Option<(String, String)>,
}

impl Default for Galaxy {
    fn default() -> Self {
        Self {
            data: None,
            current: None,
            selected: None,
            zoom: 1.0,
            opening: None,
            refusal: None,
        }
    }
}

impl Galaxy {
    /// How far out and how far in the wheel may take the field.
    pub const ZOOM_MIN: f32 = 0.5;
    pub const ZOOM_MAX: f32 = 2.5;

    /// Take the job's data. A selection past the new list's end is
    /// dropped rather than left pointing at nothing.
    pub(super) fn seed(&mut self, data: GalaxyData) {
        if self
            .selected
            .is_some_and(|index| index >= data.projects.len())
        {
            self.selected = None;
        }
        self.data = Some(data);
    }

    /// The delivered data, once the job returned.
    #[must_use]
    pub const fn data(&self) -> Option<&GalaxyData> {
        self.data.as_ref()
    }

    /// The host's current project is available before the project list arrives.
    pub(super) fn set_current(&mut self, name: String, dir: String) {
        self.current = Some(GalaxyProject {
            name,
            dir: Some(dir),
            here: true,
            ..GalaxyProject::default()
        });
    }

    /// Known projects, including the host's current project before discovery.
    #[must_use]
    pub fn projects(&self) -> &[GalaxyProject] {
        self.data
            .as_ref()
            .filter(|data| !data.projects.is_empty())
            .map_or_else(|| self.current.as_slice(), |data| &data.projects)
    }

    /// The window's own project, when the data names one.
    #[must_use]
    pub fn home(&self) -> Option<usize> {
        self.projects().iter().position(|project| project.here)
    }

    /// The cluster the keys walked onto, if any.
    #[must_use]
    pub const fn selected(&self) -> Option<usize> {
        self.selected
    }

    /// Select the cluster at `index`; `false` when there is none.
    pub(super) fn select(&mut self, index: usize) -> bool {
        if index >= self.projects().len() {
            return false;
        }
        self.selected = Some(index);
        true
    }

    /// Walk the selection one cluster `forward` (or back), opening it
    /// at the first (or last) cluster when nothing is selected and
    /// stopping at either end. `false` while there is nothing to walk.
    pub(super) fn select_step(&mut self, forward: bool) -> bool {
        let Some(last) = self.projects().len().checked_sub(1) else {
            return false;
        };
        self.selected = Some(match (self.selected, forward) {
            (None, true) => 0,
            (None, false) => last,
            (Some(index), true) => index.saturating_add(1).min(last),
            (Some(index), false) => index.saturating_sub(1),
        });
        true
    }

    /// The directory of the project being opened, until the host answers.
    #[must_use]
    pub fn opening(&self) -> Option<&str> {
        self.opening.as_deref()
    }

    /// Why the project in `dir` could not be opened, until it is tried again.
    #[must_use]
    pub fn refusal(&self, dir: &str) -> Option<&str> {
        self.refusal
            .as_ref()
            .filter(|(refused, _)| refused == dir)
            .map(|(_, words)| words.as_str())
    }

    /// The host was asked for `dir`; an earlier refusal no longer stands.
    pub(super) fn open(&mut self, dir: String) {
        self.refusal = None;
        self.opening = Some(dir);
    }

    /// The open of `dir` was refused with `words`.
    pub(super) fn refuse(&mut self, dir: String, words: String) {
        self.opening = None;
        self.refusal = Some((dir, words));
    }

    /// Carry the window's zoom onto the next project's field.
    pub(super) fn keep_zoom(&mut self, earlier: &Self) {
        self.zoom = earlier.zoom;
    }

    /// The field's zoom, `1.0` at rest.
    #[must_use]
    pub const fn zoom(&self) -> f32 {
        self.zoom
    }

    /// Zoom the field by `factor`, held between [`Self::ZOOM_MIN`] and
    /// [`Self::ZOOM_MAX`]; `false` when the zoom was already at the
    /// stop it was pushed against.
    pub(super) fn zoom_by(&mut self, factor: f32) -> bool {
        let next = (self.zoom * factor).clamp(Self::ZOOM_MIN, Self::ZOOM_MAX);
        if (next - self.zoom).abs() < f32::EPSILON {
            return false;
        }
        self.zoom = next;
        true
    }

    /// Put the field back to rest.
    pub(super) fn reset_zoom(&mut self) -> bool {
        let moved = (self.zoom - 1.0).abs() >= f32::EPSILON;
        self.zoom = 1.0;
        moved
    }

    /// Live sessions across every project — the header's `N running`.
    #[must_use]
    pub fn running(&self) -> u32 {
        self.projects().iter().map(|project| project.running).sum()
    }

    /// Decisions waiting across every project — the header's `N action
    /// required`.
    #[must_use]
    pub fn asking(&self) -> u32 {
        self.projects().iter().map(|project| project.asking).sum()
    }
}

#[cfg(test)]
#[path = "galaxy.tests.rs"]
mod tests;
