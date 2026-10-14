//! The current command surface and dependency boundary.
use clap::Parser;
use magistr::cli::Cli;

#[test]
fn retired_terminal_flags_are_refused() {
    for flag in [
        "--no-tui",
        "--no-mouse",
        "--screen-reader",
        "--splash",
        "--theme",
        "--cvd",
        "--reduce-motion",
    ] {
        assert!(Cli::try_parse_from(["magistr", flag]).is_err(), "{flag}");
    }
}

#[test]
fn command_help_keeps_the_supported_process_roles() {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_magistr"))
        .arg("--help")
        .output()
        .expect("CLI help");
    assert!(output.status.success());
    let help = String::from_utf8(output.stdout).expect("help text");
    for command in ["run", "next", "next-loop", "mcp-launch", "workspace"] {
        assert!(help.contains(command), "{command}");
    }
    assert!(!help.to_lowercase().contains("tui"), "{help}");
}
