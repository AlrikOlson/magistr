use super::ConfigurationOwner;

/// A resolved setting together with the source that currently owns its value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedValue<T> {
    pub value: T,
    pub owner: ConfigurationOwner,
}
