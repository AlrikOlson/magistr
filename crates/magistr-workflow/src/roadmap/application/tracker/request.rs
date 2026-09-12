#[derive(Debug, Clone)]
pub struct SetupRequest {
    pub provider: String,
    pub into: String,
    pub name: Option<String>,
    pub band: Option<String>,
    /// Name for the initiative roof. `None` keeps whatever is configured (or
    /// the directory-basename default 10b decided) — absent must never CLEAR a
    /// name a human chose.
    pub initiative: Option<String>,
    pub push: bool,
    pub push_secs: u64,
    pub yes: bool,
    pub dry_run: bool,
}
