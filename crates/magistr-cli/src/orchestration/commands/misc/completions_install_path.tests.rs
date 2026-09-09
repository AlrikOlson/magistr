use super::*;
use clap_complete::Shell;
use std::path::PathBuf;

#[test]
fn completion_paths_use_the_supplied_home() {
    for (shell, relative) in [
        (
            Shell::Bash,
            ".local/share/bash-completion/completions/magistr",
        ),
        (Shell::Zsh, ".local/share/zsh/site-functions/_jig"),
        (Shell::Fish, ".config/fish/completions/magistr.fish"),
    ] {
        let path = from_lookup(shell, |key| (key == "HOME").then(|| "/fixture-home".into()));
        assert_eq!(path, Some(PathBuf::from("/fixture-home").join(relative)));
    }
}

#[test]
fn xdg_roots_override_the_home_defaults() {
    let get = |key: &str| match key {
        "HOME" => Some("/fixture-home".into()),
        "XDG_DATA_HOME" => Some("/fixture-data".into()),
        "XDG_CONFIG_HOME" => Some("/fixture-config".into()),
        _ => None,
    };
    for (shell, expected) in [
        (
            Shell::Bash,
            "/fixture-data/bash-completion/completions/magistr",
        ),
        (Shell::Zsh, "/fixture-data/zsh/site-functions/_jig"),
        (Shell::Fish, "/fixture-config/fish/completions/magistr.fish"),
    ] {
        assert_eq!(from_lookup(shell, get), Some(PathBuf::from(expected)));
    }
}

#[test]
fn a_missing_home_or_unsupported_shell_has_no_install_path() {
    assert_eq!(from_lookup(Shell::Bash, |_| None), None);
    assert_eq!(
        from_lookup(Shell::PowerShell, |_| Some("/fixture-home".into())),
        None
    );
}
