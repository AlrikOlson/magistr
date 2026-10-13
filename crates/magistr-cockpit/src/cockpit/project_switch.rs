//! Opening another project in the same window: the ask, the refusal in
//! words and the swap to the model the host seeded for the new project.
//!
//! The cockpit never re-roots itself. It asks the host once
//! ([`Cmd::OpenProject`]), keeps the current project usable until the
//! host answers with an addressed session frame. The window retains its surface,
//! size, theme, focus and zoom; observations and drafts belong to the session.

use super::{Cmd, Model, Msg, ProjectSeed, StageTab};

/// Whether the project at `index` can be opened from this window: it is
/// not the one already open, its directory is known, and the session's
/// host is one that can admit another project in place.
#[must_use]
pub fn can_open_project(model: &Model, index: usize) -> bool {
    model.hosts_work()
        && model
            .galaxy
            .projects()
            .get(index)
            .is_some_and(|project| !project.here && project.dir.is_some())
}

/// Select the project at `index` without stopping work in the current session.
/// An already pending selection waits for its admission outcome.
pub(super) fn open(model: &mut Model, index: usize) -> Vec<Cmd> {
    if !can_open_project(model, index) || model.galaxy.opening().is_some() {
        return Vec::new();
    }
    let Some(dir) = model.galaxy.projects()[index].dir.clone() else {
        return Vec::new();
    };
    model.galaxy.open(dir.clone());
    vec![Cmd::OpenProject(dir)]
}

/// The host could not open `dir`: the words stay on that project and the
/// current one is untouched.
pub(super) fn refused(model: &mut Model, dir: &str, error: &str) -> Vec<Cmd> {
    let words = format!("Magistr could not open the project. {error}");
    model.galaxy.refuse(dir.to_owned(), words.clone());
    vec![Cmd::Announce(words)]
}

/// The host selected a resident session: build its model
/// from `seed` and show its roadmap, with the window's own state carried
/// across.
pub(super) fn rehosted(model: &mut Model, seed: ProjectSeed) -> Vec<Cmd> {
    let mut next = seed.seed(Model::new(model.size).with_tab(StageTab::Turn));
    next.set_theme(model.theme);
    next.focused = model.focused;
    next.set_surface(model.surface());

    next.galaxy.keep_zoom(&model.galaxy);
    let size = model.size;
    *model = next;
    let mut cmds = super::update(model, Msg::Resize(size));
    model.show(StageTab::Roadmap);
    let name = model
        .galaxy
        .projects()
        .iter()
        .find(|project| project.here)
        .map_or_else(|| "the project".to_owned(), |project| project.name.clone());
    cmds.push(Cmd::Announce(format!("Opened {name}.")));
    cmds
}

#[cfg(test)]
#[path = "project_switch.tests.rs"]
mod tests;
