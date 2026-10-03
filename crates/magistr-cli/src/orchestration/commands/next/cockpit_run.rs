//! The cockpit entry state, including whether the user chooses a loop track.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CockpitRun {
    Single,
    Loop,
    ChooseTrack,
}
