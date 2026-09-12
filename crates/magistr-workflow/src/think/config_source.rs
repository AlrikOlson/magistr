/// Non-secret authority that supplied one resolved configuration setting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigSource {
    Default,
    Environment(&'static str),
    Explicit,
    ProjectIdentity,
}
