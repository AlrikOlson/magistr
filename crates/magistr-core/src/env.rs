//! Process formatting choice supplied to the formatter; project and provider inputs have their own owner.
/// The process choice used by `Formatter::with_env`.
#[derive(Debug, Clone, Default)]
pub struct MagistrEnv {
    pub no_autoformat: bool,
}
impl MagistrEnv {
    #[must_use]
    pub fn from_process_env() -> Self {
        Self {
            no_autoformat: std::env::var("MAGISTR_NO_AUTOFORMAT").is_ok_and(|value| value == "1"),
        }
    }
}
