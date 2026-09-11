use super::*;
use crate::config::ResolvedConfig;

#[test]
fn declared_argv_appends_arguments_without_shell_interpretation() {
    let mut config = ResolvedConfig::default();
    config.commands.insert("test".into(), "cargo test".into());
    assert_eq!(
        CommandAdmission::admit(&config, "test", &["--filter".into(), "has spaces".into()])
            .unwrap(),
        ["cargo", "test", "--filter", "has spaces"]
    );
}

#[test]
fn undeclared_and_empty_commands_are_refused() {
    let mut config = ResolvedConfig::default();
    assert!(CommandAdmission::admit(&config, "test", &[]).is_err());
    config.commands.insert("empty".into(), "  ".into());
    assert!(CommandAdmission::admit(&config, "empty", &["cargo".into()]).is_err());
}

#[test]
fn no_shell_guard_checks_operators_and_wrapped_program_positions() {
    for line in [
        "bash -c pwd",
        "cargo test && cargo build",
        "env FOO=1 /bin/bash",
        "npx -y C:\\bin\\sed.exe",
        "xargs --no-run-if-empty rg",
        "env /bin/cat",
    ] {
        let mut config = ResolvedConfig::default();
        config.commands.insert("test".into(), line.into());
        assert!(
            CommandAdmission::admit(&config, "test", &[]).is_err(),
            "{line}"
        );
    }
    let mut config = ResolvedConfig::default();
    config.commands.insert("test".into(), "cargo test".into());
    assert!(CommandAdmission::admit(&config, "test", &["|".into(), "cat".into()]).is_err());
    assert!(CommandAdmission::admit(&config, "test", &["cat".into()]).is_ok());
}
