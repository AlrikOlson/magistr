//! One ⌘K result, as `update` computes it and a composition renders it.

use super::checkpoint_cmd::CheckpointCmd;
use super::loop_cmd::LoopCmd;
use super::place::Place;
use crate::input::{KeyCode, Modifiers};

/// One result in the ⌘K palette: a *place* (the verb `Open` and the
/// identity it names, with whatever live badge and pending act the
/// place carries) or a *verb* (a [`Binding`](super::Binding), wearing
/// its keys). What confirming it dispatches stays crate-side — a
/// surface renders the row; only `update` acts on it.
#[derive(Clone, Debug, PartialEq)]
pub struct PaletteRow {
    /// The row's verb — `Open` for a place, the registry's `does` for
    /// a binding.
    pub verb: String,
    /// The place's identity — the project, an altitude, a session, a
    /// chunk. `None` on a binding row, whose `verb` is the whole label.
    pub name: Option<String>,
    /// Whether `name` wears the project sigil.
    pub sigil: bool,
    /// The live badge the place carries (`1 action required`).
    pub badge: Option<String>,
    /// The pending act ⌘↵ takes (`Answer pending decision`).
    pub act: Option<String>,
    /// A binding row's keys, as the sheet spells them.
    pub keys: Option<&'static str>,
    /// What ↵ dispatches.
    pub(crate) go: PaletteGo,
}

/// What confirming a row does: navigate the breadcrumb, or dispatch a
/// binding's canonical key as the `Msg` the terminal sends.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum PaletteGo {
    /// Start an existing host command.
    Launch(super::launch_action::LaunchAction),
    /// Open a terminal tab.
    Tab(super::stage_tab::StageTab),
    /// Move the camera to a place.
    Place(Place),
    /// Open the sky centred on a chunk.
    Chunk(String),
    /// Walk to the launcher's session row.
    Session(usize),
    /// Dispatch the binding's key through the same fold.
    Key(KeyCode, Modifiers),
    /// A loop verb, dispatched as itself — never as its key, which is
    /// the prompt's letter while the loop runs.
    Loop(LoopCmd),
    /// Take a checkpoint of the state as it stands.
    CheckpointNow,
    /// A checkpoint row's verb on the previewed state — `Restore
    /// cp-3`, `Fork cp-3` — the keyboard's way to the verbs the rail
    /// reveals on hover.
    Checkpoint(CheckpointCmd),
    /// Accept the charter's offer for a scope — the keyboard's way to
    /// the `Delegate` the rail draws on the offer's row.
    Delegate(String),
}

impl PaletteRow {
    /// A place row: `Open name`, with whatever it carries.
    pub(crate) fn place(name: impl Into<String>, go: PaletteGo) -> Self {
        Self {
            verb: "Open".to_owned(),
            name: Some(name.into()),
            sigil: false,
            badge: None,
            act: None,
            keys: None,
            go,
        }
    }

    /// A binding row: the registry's `does` with its keys.
    pub(crate) fn binding(binding: super::Binding) -> Self {
        Self {
            verb: binding.does.to_owned(),
            name: None,
            sigil: false,
            badge: None,
            act: None,
            keys: Some(binding.keys),
            go: PaletteGo::Key(binding.code, binding.mods),
        }
    }

    /// The text a query is matched against.
    pub(crate) fn haystack(&self) -> String {
        match &self.name {
            Some(name) => format!("{} {name}", self.verb),
            None => self.verb.clone(),
        }
    }
}
