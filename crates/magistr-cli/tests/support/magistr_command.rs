/// A CLI fixture owns Magistr's settings; callers can set the variables they
/// exercise after construction. Tool discovery and platform variables stay intact.
pub fn magistr_command() -> std::process::Command {
    let mut command = std::process::Command::new(env!("CARGO_BIN_EXE_magistr"));
    for (key, _) in std::env::vars_os() {
        if key.to_string_lossy().starts_with("MAGISTR_") {
            command.env_remove(key);
        }
    }
    command
}
