use super::*;
use crate::application::WorkTarget;
use magistr_cockpit::cockpit::{LoopCmd, RunCommand, WorkPhase, WorkRun};
use magistr_domain::{
    identity::{ProjectId, RunId},
    workflow::RunMode,
};

#[tokio::test]
async fn controls_address_the_resident_run_across_conversation_changes_and_reject_stale_runs() {
    let tmp = projects();
    let log = Log::default();
    let capture = Arc::new(Mutex::new(None));
    let captured = Arc::clone(&capture);
    let mut open = opener(log.clone());
    let registry = SessionRegistry::start_with(
        Box::new(move |opening| {
            *captured.lock().unwrap() = Some((opening.inbox.clone(), opening.observation.clone()));
            open(opening)
        }),
        Box::new(|| Box::pin(async { Ok(()) })),
    );
    let client = registry.client();
    let address = client
        .open(tmp.path().join("first"), SessionOpening::Launch)
        .await
        .unwrap();
    let mut reader = client.subscribe(&address).await.unwrap();
    let (inbox, observation) = capture.lock().unwrap().take().unwrap();
    let id = RunId::new(ProjectId::new("project").unwrap(), "first-run").unwrap();
    let target = WorkTarget {
        address: address.clone(),
        run_id: id.clone(),
    };
    assert!(client.control_work(&target, LoopCmd::Pause).await.is_err());
    let mut work = WorkRun {
        id: id.clone(),
        mode: RunMode::ChunkLoop,
        track: None,
        chunk: None,
        iteration: 1,
        limit: 10,
        phase: WorkPhase::Running,
        requested: None,
    };
    inbox.send(Msg::WorkRun(work.clone()));
    until(&mut reader, |s| {
        s.work.as_ref().is_some_and(|run| run.id == id)
    })
    .await;
    observation.bind_context_for_test(magistr_application::sessions::Conversation {
        id: "next-provider-conversation".into(),
        purpose: magistr_application::sessions::SessionPurpose::Conversation,
        name: None,
        provider: "scripted".into(),
    });
    assert!(client.control_work(&target, LoopCmd::Pause).await.is_ok());
    let stale = WorkTarget {
        run_id: RunId::new(ProjectId::new("project").unwrap(), "other-run").unwrap(),
        ..target.clone()
    };
    assert!(client.control_work(&stale, LoopCmd::Abort).await.is_err());
    assert!(
        client
            .command(
                &mutation_target(&address),
                Cmd::Loop(RunCommand {
                    run_id: id,
                    action: LoopCmd::Abort
                })
            )
            .await
            .is_err()
    );
    work.phase = WorkPhase::Ended {
        reason: "Finished.".into(),
    };
    inbox.send(Msg::WorkRun(work.clone()));
    until(&mut reader, |s| {
        s.work.as_ref().is_some_and(|run| !run.is_active())
    })
    .await;
    assert!(client.control_work(&target, LoopCmd::Resume).await.is_err());
    work.id = RunId::new(ProjectId::new("project").unwrap(), "next-run").unwrap();
    work.phase = WorkPhase::Starting;
    inbox.send(Msg::WorkRun(work));
    until(&mut reader, |s| {
        s.work
            .as_ref()
            .is_some_and(|run| run.id.local() == "next-run")
    })
    .await;
    assert!(client.control_work(&target, LoopCmd::Abort).await.is_err());
    client.close(&address).await.unwrap();
    assert!(client.control_work(&target, LoopCmd::Abort).await.is_err());
    registry.shutdown().await.unwrap();
    assert_eq!(
        log.lines()
            .iter()
            .filter(|s| s.contains(" work "))
            .cloned()
            .collect::<Vec<_>>(),
        ["first work first-run Pause"]
    );
}
