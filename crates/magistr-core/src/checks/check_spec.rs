/// One check to run: its name, the exact command, and whether a red is a
/// real failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckSpec {
    pub name: String,
    pub command: String,
    pub required: bool,
}
