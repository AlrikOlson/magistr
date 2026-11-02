/// A CLI fixture owns Magistr's settings; callers can set the variables they
/// exercise after construction. Tool discovery and platform variables stay intact.
///
/// The one setting it never drops is where saved work lives: every fixture
/// process reads and writes an isolated data home, never the developer's own.
/// Under nextest that is the run's store from `scripts/nextest-isolated-store.sh`;
/// otherwise this test binary creates one temporary directory and shares it.
/// It is set as `XDG_DATA_HOME`, so a test that names its own data home or
/// `MAGISTR_WORKFLOW_DATA_DIR` still wins.
pub fn magistr_command() -> std::process::Command {
    let mut command = std::process::Command::new(env!("CARGO_BIN_EXE_magistr"));
    for (key, _) in std::env::vars_os() {
        if key.to_string_lossy().starts_with("MAGISTR_") {
            command.env_remove(key);
        }
    }
    command.env("XDG_DATA_HOME", isolated_store());
    command
}

fn isolated_store() -> std::path::PathBuf {
    static STORE: std::sync::OnceLock<std::path::PathBuf> = std::sync::OnceLock::new();
    STORE
        .get_or_init(|| {
            std::env::var_os("MAGISTR_WORKFLOW_DATA_DIR")
                .filter(|value| !value.is_empty())
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|| {
                    tempfile::Builder::new()
                        .prefix("magistr-test-store")
                        .tempdir()
                        .expect("a temporary workflow store")
                        .keep()
                })
        })
        .clone()
}
