use super::*;

fn saved() -> SessionSelection {
    SessionSelection {
        agent: "codex".into(),
        model: Some("saved-model".into()),
        effort: Some("high".into()),
        sources: Default::default(),
    }
}

#[test]
fn recovery_requires_matching_explicit_options() {
    let matching = WorkflowAgentOptions {
        agent: Some("codex".into()),
        model: Some("saved-model".into()),
        effort: Some("high".into()),
        show_config: false,
    };
    assert!(matching.check_recovery(&saved()).is_ok());
    assert!(
        WorkflowAgentOptions::default()
            .check_recovery(&saved())
            .is_ok()
    );
    for options in [
        WorkflowAgentOptions {
            agent: Some("claude".into()),
            ..Default::default()
        },
        WorkflowAgentOptions {
            model: Some("other-model".into()),
            ..Default::default()
        },
        WorkflowAgentOptions {
            effort: Some("default".into()),
            ..Default::default()
        },
    ] {
        assert!(options.check_recovery(&saved()).is_err());
    }
}

#[test]
fn all_native_commands_parse_the_same_options_and_provider_alias() {
    use crate::cli::{Cli, Command};
    use clap::Parser;
    for command in ["next", "next-loop", "run"] {
        let mut args = vec!["magistr", command];
        if command == "run" {
            args.push("hello");
        }
        args.extend([
            "--provider",
            "codex",
            "-m",
            "chosen",
            "--effort",
            "high",
            "--show-config",
        ]);
        let parsed = Cli::try_parse_from(args).unwrap();
        let options = match parsed.command.unwrap() {
            Command::Next { agent, .. }
            | Command::NextLoop { agent, .. }
            | Command::Run { agent, .. } => agent,
            _ => panic!("wrong command"),
        };
        assert_eq!(options.agent.as_deref(), Some("codex"));
        assert_eq!(options.model.as_deref(), Some("chosen"));
        assert_eq!(options.effort.as_deref(), Some("high"));
        assert!(options.show_config);
    }
}
