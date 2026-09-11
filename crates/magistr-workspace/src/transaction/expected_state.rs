/// Explicit existence and SHA-256 anchor; missing is distinct from an empty file.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ExpectedState {
    Missing,
    Hash(String),
}
