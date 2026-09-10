#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetPhase {
    /// It is there. Carry on.
    Present,
    /// It was not there and we made it.
    Created,
    /// The provider cannot be asked. Carry on, but do not claim it exists.
    Unverifiable,
    /// It was not there and we were not permitted to create it. STOP — writing
    /// local config now would name somewhere that does not exist.
    MissingAndNotCreated,
}
