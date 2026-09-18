use super::*;
use magistr_application::sessions::AgentEvent;
use magistr_cockpit::cockpit::{Cmd, Msg};

#[tokio::test]
async fn host_connector_preserves_choices_and_separates_planning_configuration() {
    for (command, planning) in [
        (Cmd::Prompt("Continue".into()), false),
        (Cmd::RoadmapPrompt("Plan".into()), true),
    ] {
        let dir = tempfile::tempdir().unwrap();
        let mut config = AgentConfig::new("old opening", dir.path()).model("chosen");
        config.resume = Some("old-session".into());
        config.continue_conversation = true;
        config.workflow_permissions = true;
        let (observed, mut received) = tokio::sync::mpsc::unbounded_channel();
        let connector = Connector::new(move |config| {
            observed.send(config).unwrap();
            async { Err(crate::error::MagistrError::Acp("connection refused".into())) }
        });
        let (inbox, mut events) = Inbox::channel();
        let (commands, receiver) = magistr_cockpit_runtime::channel::Commands::channel();
        let task = interactive_session(config, connector, inbox, None).spawn(receiver);
        commands.send(command).unwrap();
        let actual = tokio::time::timeout(std::time::Duration::from_secs(2), received.recv())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(actual.model.as_deref(), Some("chosen"));
        assert!(
            actual.prompt.is_empty() && actual.resume.is_none() && !actual.continue_conversation
        );
        assert_eq!(actual.planning_only, planning);
        assert_eq!(actual.workflow_permissions, !planning);
        if planning {
            assert!(
                actual
                    .allowed_tools
                    .iter()
                    .any(|tool| tool == "roadmap_add_chunk")
            );
            assert!(!actual.allowed_tools.iter().any(|tool| tool == "ship_start"));
        }
        assert!(
            matches!(events.recv().await, Some(Msg::Agent(AgentEvent::PromptRejected { error, .. })) if error.contains("connection refused"))
        );
        drop(commands);
        task.await.unwrap().unwrap();
    }
}
