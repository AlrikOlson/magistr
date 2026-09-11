/// Caller intent; undo must disable formatting to restore exact journal bytes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FormatMode {
    Enabled,
    Disabled,
}
