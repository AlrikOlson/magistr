/// The manifest and revision hashing scheme. A snapshot written under
/// another scheme never compares equal to one written under this one, so a
/// change to what is hashed (or how) is a new scheme, not a silent drift.
pub const SCHEME: &str = "magistr-source-snapshot/1";
