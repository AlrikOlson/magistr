//! The cockpit and the agent share one live admitted workspace owner.
//! Exercise journal listing, rewind, conflict surfacing and the next agent
//! write through the production ports, with isolated state and real workers.

use std::path::Path;
use std::sync::Arc;

use magistr::application::HostedWorkspace;
use magistr::application::workspace_client::McpWorkbench;
use magistr_acp::{AcpEvent, capabilities::LocalEffectPort};
use magistr_application::effects::{
    EffectAuthorizer, EffectFuture, EffectPort, EffectRequest, EffectResponse, SourceRoute,
};
use magistr_cockpit::{
    cockpit::{Model, Msg, Size, WorkbenchCmd, WorkbenchEvent, update},
    workbench,
};
use magistr_workspace::{service::WorkspaceOpenOptions, snapshot::MutationStatus};
use tokio::sync::mpsc;

struct AllowWrite;

impl EffectAuthorizer for AllowWrite {
    fn authorize(&self, _: EffectRequest) -> EffectFuture<'_, ()> {
        Box::pin(async { Ok(()) })
    }

    fn is_cancelled(&self) -> bool {
        false
    }
}

async fn agent_write(
    port: &LocalEffectPort,
    events: &mut mpsc::UnboundedReceiver<magistr_acp::Result<AcpEvent>>,
    path: &Path,
    content: &str,
) -> i64 {
    assert_eq!(
        port.execute(
            EffectRequest::WriteTextFile {
                session_id: "journal-owner-fixture".into(),
                path: path.display().to_string(),
                content: content.into(),
            },
            Arc::new(AllowWrite),
        )
        .await
        .expect("authorized agent write"),
        EffectResponse::Written
    );
    std::iter::from_fn(|| events.try_recv().ok())
        .find_map(|event| match event {
            Ok(AcpEvent::System { subtype, data }) if subtype == "fs_write" => {
                assert_eq!(data["route"], "workspace");
                Some(data["journal_seq"].as_i64().expect("journaled operation"))
            }
            _ => None,
        })
        .expect("the agent's write event")
}

async fn cockpit_call(mut port: McpWorkbench, cmd: WorkbenchCmd) -> (McpWorkbench, WorkbenchEvent) {
    tokio::task::spawn_blocking(move || {
        let event = workbench::apply(&mut port, &cmd);
        (port, event)
    })
    .await
    .expect("bounded cockpit operation")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn journal_and_undo_share_the_agents_owner_and_preserve_external_edits() {
    let tmp = tempfile::tempdir().expect("isolated workspace");
    let root = tmp.path().join("workspace");
    std::fs::create_dir_all(root.join(".git")).expect("repository marker");
    std::fs::write(root.join("notes.txt"), "original\n").expect("initial content");
    let binary = env!("CARGO_BIN_EXE_magistr");
    let hosted = HostedWorkspace::admit_with(
        WorkspaceOpenOptions::new(&root, binary).with_network_override("0"),
    )
    .await
    .expect("admitted workspace")
    .expect("sole owner");
    let root = hosted.service().root().to_path_buf();
    let generation = hosted.generation();
    let target = root.join("notes.txt");
    let (events, mut received) = mpsc::unbounded_channel();
    let agent = LocalEffectPort::new(
        root.clone(),
        events,
        SourceRoute::Admitted(hosted.source_writer().expect("admitted writer")),
    );
    let first = agent_write(&agent, &mut received, &target, "agent change\n").await;
    let mut port = McpWorkbench::new(
        binary,
        vec!["mcp-launch".into(), "workbench-mcp".into(), ".".into()],
        vec![("WORKBENCH_NETWORK_FS".into(), "0".into())],
    )
    .with_workspace(&root);
    let mut model = Model::new(Size::new(100, 30));

    let event;
    (port, event) = cockpit_call(port, WorkbenchCmd::Journal).await;
    let WorkbenchEvent::Journal { entries } = &event else {
        panic!("owner journal: {event:?}");
    };
    assert_eq!(entries.len(), 1, "{entries:?}");
    assert_eq!(entries[0].id, first.to_string());
    assert_eq!(entries[0].path, "notes.txt");
    assert!(entries[0].diff.contains("+agent change"));
    update(&mut model, Msg::Workbench(event));

    let event;
    (port, event) = cockpit_call(
        port,
        WorkbenchCmd::Undo {
            id: first.to_string(),
        },
    )
    .await;
    assert!(
        matches!(&event, WorkbenchEvent::Undone { id } if id == &first.to_string()),
        "{event:?}"
    );
    update(&mut model, Msg::Workbench(event));
    assert!(model.changes().entries().is_empty());
    assert_eq!(
        std::fs::read_to_string(&target).expect("rewound"),
        "original\n"
    );

    let second = agent_write(&agent, &mut received, &target, "second agent change\n").await;
    let event;
    (port, event) = cockpit_call(port, WorkbenchCmd::Journal).await;
    assert!(
        matches!(&event, WorkbenchEvent::Journal { .. }),
        "{event:?}"
    );
    update(&mut model, Msg::Workbench(event));
    assert!(
        model
            .changes()
            .entries()
            .iter()
            .any(|row| row.id == second.to_string())
    );

    std::fs::write(&target, "external edit\n").expect("an external editor");
    let event;
    (port, event) = cockpit_call(
        port,
        WorkbenchCmd::Undo {
            id: second.to_string(),
        },
    )
    .await;
    assert!(
        matches!(&event, WorkbenchEvent::Failed {
        cmd: WorkbenchCmd::Undo { id }, error
    } if id == &second.to_string() && error.contains("notes.txt")),
        "{event:?}"
    );
    update(&mut model, Msg::Workbench(event));
    assert!(model.changes().refusal(&second.to_string()).is_some());
    assert_eq!(
        std::fs::read_to_string(&target).expect("preserved"),
        "external edit\n"
    );

    let next = agent_write(
        &agent,
        &mut received,
        &root.join("next.txt"),
        "next agent change\n",
    )
    .await;
    for seq in [first, second, next] {
        let receipt = hosted
            .service()
            .mutation_receipt(seq)
            .await
            .expect("receipt read")
            .expect("receipt exists");
        assert_eq!(receipt.status, MutationStatus::Applied);
        assert_eq!(receipt.generation, Some(generation));
    }
    let (port, event) = cockpit_call(port, WorkbenchCmd::Journal).await;
    let WorkbenchEvent::Journal { entries } = event else {
        panic!("journal after conflict: {event:?}");
    };
    assert!(
        entries
            .iter()
            .any(|row| row.id == next.to_string() && row.path == "next.txt")
    );
    assert!(
        !entries
            .iter()
            .any(|row| row.diff.contains("+external edit")),
        "an external edit is not an applied agent operation"
    );
    assert_eq!(hosted.generation(), generation);
    tokio::task::spawn_blocking(move || drop(port))
        .await
        .expect("bridge closed");
    drop(agent);
    hosted.shutdown().await.expect("ordered owner shutdown");
}
