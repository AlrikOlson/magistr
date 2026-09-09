/// Layer that supplied a resolved setting. Ownership is descriptive; a project
/// update never grants authority to overwrite a user or invocation layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigurationOwner {
    BuiltIn,
    User,
    Project,
    Invocation,
}
