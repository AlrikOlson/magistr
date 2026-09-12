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
        "env NAME=value nice env OTHER=value /bin/sh",
        "time nice cat Cargo.toml",
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

#[test]
fn wrapper_options_and_operands_are_refused_without_guessing_the_program() {
    for line in [
        "env -u NAME sh -c pwd",
        "env -u NAME cat Cargo.toml",
        "nice -n 10 sh -c pwd",
        "nice -n 10 sed -n 1p Cargo.toml",
        "env FOO=1 nice -n 10 env -u NAME /bin/bash",
        "time -o timing.log sh -c pwd",
        "xargs -I placeholder sh -c pwd",
        "npx --package tool sh -c pwd",
        "bunx --package tool cat Cargo.toml",
        "env --unset=NAME cargo test",
        "env -uNAME cargo test",
        "nice -n10 cargo test",
        "nice --adjustment=10 cargo test",
        "env -S cargo",
        "env -vS cargo",
        "env --split-string=cargo",
        "env -- cargo test",
        "nice -- cargo test",
        "env NAME=value -- sh",
        "npx -y jest",
    ] {
        let mut config = ResolvedConfig::default();
        config.commands.insert("test".into(), line.into());
        let error = CommandAdmission::admit(&config, "test", &[]).expect_err(line);
        assert_eq!(error.kind, ErrorKind::RunRedirected, "{line}");
    }
}

#[test]
fn wrapper_options_in_appended_arguments_are_also_refused() {
    for (line, args) in [
        ("env", vec!["-u", "NAME", "sh", "-c", "pwd"]),
        ("nice", vec!["-n", "10", "cat", "Cargo.toml"]),
        ("env FOO=1 nice", vec!["-n", "10", "sh"]),
    ] {
        let mut config = ResolvedConfig::default();
        config.commands.insert("test".into(), line.into());
        let args = args.into_iter().map(str::to_owned).collect::<Vec<_>>();
        assert_eq!(
            CommandAdmission::admit(&config, "test", &args)
                .expect_err(line)
                .kind,
            ErrorKind::RunRedirected,
        );
    }
}

#[test]
fn incomplete_wrappers_and_assignments_outside_env_are_refused() {
    for line in [
        "env",
        "nice",
        "npx",
        "bunx",
        "xargs",
        "time",
        "env NAME=value",
        "env odd-name=value sh -c pwd",
        "env =value cat Cargo.toml",
        "env ./NAME=value sh -c pwd",
        "env NAME=value nice env odd.name=value /bin/sh",
        "env nice env",
        "nice NAME=value cargo test",
        "NAME=value cargo test",
    ] {
        let mut config = ResolvedConfig::default();
        config.commands.insert("test".into(), line.into());
        assert_eq!(
            CommandAdmission::admit(&config, "test", &[])
                .expect_err(line)
                .kind,
            ErrorKind::RunRedirected,
        );
    }
}

#[test]
fn option_free_wrapper_chains_preserve_argv_and_program_arguments() {
    for line in [
        "cargo test",
        "env NAME=value cargo test",
        "nice cargo test",
        "env NAME=value nice env OTHER=value cargo test",
        "time nice cargo test",
        "npx jest",
        "bunx vitest run",
        "xargs cargo test",
    ] {
        let mut config = ResolvedConfig::default();
        config.commands.insert("test".into(), line.into());
        let args = ["--", "cat", "has spaces", "$(literal)", "NAME=value"];
        let args = args.map(str::to_owned);
        let expected = line
            .split_whitespace()
            .map(str::to_owned)
            .chain(args.iter().cloned())
            .collect::<Vec<_>>();
        assert_eq!(
            CommandAdmission::admit(&config, "test", &args).unwrap(),
            expected
        );
    }
}

#[test]
fn operators_remain_refused_after_the_wrapped_program() {
    for operator in OPERATORS {
        let mut config = ResolvedConfig::default();
        config
            .commands
            .insert("test".into(), "env NAME=value nice cargo test".into());
        assert_eq!(
            CommandAdmission::admit(&config, "test", &[(*operator).into()])
                .unwrap_err()
                .kind,
            ErrorKind::RunRedirected,
        );
    }
}
