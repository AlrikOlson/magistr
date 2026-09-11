use std::path::PathBuf;
use std::time::Duration;

/// Native formatter executables and per-process resource limits.
#[derive(Debug, Clone)]
pub struct FormatterOptions {
    /// Explicit rustfmt executable, bypassing toolchain discovery.
    pub rustfmt: Option<PathBuf>,
    /// Explicit Biome executable, otherwise resolved from PATH.
    pub biome: Option<PathBuf>,
    /// Maximum lifetime of each formatter or discovery subprocess.
    pub timeout: Duration,
    /// Maximum input size and maximum bytes collected from each output stream.
    pub max_output_bytes: usize,
}

impl Default for FormatterOptions {
    fn default() -> Self {
        Self {
            rustfmt: None,
            biome: None,
            timeout: Duration::from_secs(10),
            max_output_bytes: 16 * 1024 * 1024,
        }
    }
}
