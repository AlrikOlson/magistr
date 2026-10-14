//! Real provider and workspace child-process proof for the resident application owner.
//! The local HTTP peer and temporary projects are supplied by the recorded test driver.
use color_eyre::eyre::{self, ensure};
use magistr::{
    agent::AgentConfig,
    application::{SessionRegistry, SessionSubscription},
};
use magistr_application::sessions::AgentEvent;
use magistr_cockpit::cockpit::{Cmd, Msg};
use std::{
    io::Write,
    path::{Path, PathBuf},
    time::Duration,
};

fn configuration(root: &Path) -> eyre::Result<AgentConfig> {
    let provider = if root.file_name().is_some_and(|name| name == "failed") {
        "unavailable-provider"
    } else {
        "ollama"
    };
    Ok(AgentConfig::new("", root)
        .provider(provider)
        .model("desktop-test"))
}
async fn text(reader: &mut SessionSubscription, expected: &str) -> eyre::Result<()> {
    tokio::time::timeout(Duration::from_secs(15), async {
        loop {
            let snapshot = reader.snapshot();
            let joined: String = snapshot
                .messages
                .iter()
                .filter_map(|record| match &record.message {
                    Msg::Agent(AgentEvent::Text(text)) => Some(text.as_str()),
                    _ => None,
                })
                .collect();
            if joined.contains(expected) {
                return Ok::<_, eyre::Report>(());
            }
            reader
                .changed()
                .await
                .ok_or_else(|| eyre::eyre!("session observation closed"))?;
        }
    })
    .await??;
    Ok(())
}
async fn exercise(parent: PathBuf) -> eyre::Result<()> {
    let first = parent.join("first");
    let registry = SessionRegistry::start(configuration);
    let client = registry.client();
    let a = client
        .open(&first, magistr::application::SessionOpening::Launch)
        .await
        .map_err(|error| eyre::eyre!(error))?;
    let b = client
        .open(
            parent.join("second"),
            magistr::application::SessionOpening::Launch,
        )
        .await
        .map_err(|error| eyre::eyre!(error))?;
    let c = client
        .open(&first, magistr::application::SessionOpening::Launch)
        .await
        .map_err(|error| eyre::eyre!(error))?;
    ensure!(
        a.session != c.session && a.workspace == c.workspace,
        "independent same-copy sessions"
    );
    let mut stream = client
        .subscribe(&a)
        .await
        .map_err(|error| eyre::eyre!(error))?;
    client
        .command(&a, Cmd::Prompt("first streaming".into()))
        .await
        .map_err(|error| eyre::eyre!(error))?;
    text(&mut stream, "first streaming").await?;
    drop(stream);
    client
        .draft(&a, "unsent first".into())
        .await
        .map_err(|error| eyre::eyre!(error))?;
    let mut other = client
        .subscribe(&b)
        .await
        .map_err(|error| eyre::eyre!(error))?;
    client
        .command(&b, Cmd::Prompt("second complete".into()))
        .await
        .map_err(|error| eyre::eyre!(error))?;
    text(&mut other, "second complete").await?;
    client
        .command(&a, Cmd::Cancel)
        .await
        .map_err(|error| eyre::eyre!(error))?;
    let mut restored = client
        .subscribe(&a)
        .await
        .map_err(|error| eyre::eyre!(error))?;
    let snapshot = restored.snapshot();
    ensure!(
        snapshot.seed.draft.as_deref() == Some("unsent first"),
        "retained own draft"
    );
    ensure!(
        snapshot.replay().iter().any(
            |message| matches!(message, Msg::InputAccepted(input) if input.text == "first streaming")
        ),
        "the session owns the accepted user message"
    );
    ensure!(
        client
            .open(
                parent.join("failed"),
                magistr::application::SessionOpening::Launch
            )
            .await
            .is_err(),
        "provider failure must be explicit"
    );
    ensure!(
        client
            .list()
            .await
            .map_err(|error| eyre::eyre!(error))?
            .len()
            == 3,
        "failed provider must not replace resident sessions"
    );
    let mut third = client
        .subscribe(&c)
        .await
        .map_err(|error| eyre::eyre!(error))?;
    client
        .command(&c, Cmd::Prompt("third active".into()))
        .await
        .map_err(|error| eyre::eyre!(error))?;
    text(&mut third, "third active").await?;
    client.close(&b).await.map_err(|error| eyre::eyre!(error))?;
    let inputs = magistr::config::inputs::ProjectInputs::from_env(&first);
    let history_path = magistr::session::storage::sessions_db_path(&inputs)
        .ok_or_else(|| eyre::eyre!("isolated history directory"))?;
    let history = magistr::session::storage::open_db(&history_path)?;
    ensure!(
        magistr::session::storage::count_sessions(&history)? == 1,
        "closing the actual second provider session must save its history"
    );
    ensure!(
        client.command(&b, Cmd::Cancel).await.is_err(),
        "removed session must reject controls"
    );
    drop(other);
    drop(restored);
    drop(third);
    registry
        .shutdown()
        .await
        .map_err(|error| eyre::eyre!(error))?;
    for name in ["first", "second", "failed"] {
        let inputs = magistr::config::inputs::ProjectInputs::from_env(parent.join(name));
        let owner = magistr::application::WorkspaceAdmission::admit(&inputs).await;
        ensure!(
            owner.hosted().is_some(),
            "working copy {name} released: {}",
            owner.report()
        );
        owner.shutdown().await?;
    }
    std::io::stdout().write_all(b"{\"sessions\":3,\"workspaces\":2,\"stream_switch\":true,\"cancel\":true,\"reconnect\":true,\"startup_failure\":true,\"quit\":true,\"source_owners_released\":true}\n")?;
    Ok(())
}
fn main() -> eyre::Result<()> {
    let args: Vec<_> = std::env::args().collect();
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    if args.get(1).map(String::as_str) == Some("--workspace-worker") {
        return runtime
            .block_on(magistr_workspace::worker::serve_worker())
            .map_err(Into::into);
    }
    if args.get(1).map(String::as_str) == Some("acp-agent") {
        let provider = args
            .get(3)
            .ok_or_else(|| eyre::eyre!("provider argument"))?;
        if let Ok(directory) = std::env::var("MAGISTR_RESIDENT_TEST_DIR") {
            let trace = std::fs::File::create(
                PathBuf::from(directory).join(format!("provider-{}.json", std::process::id())),
            )?;
            serde_json::to_writer(
                trace,
                &serde_json::json!({ "execution": std::env::var("MAGISTR_EXECUTION_ID")?, "workspace": std::env::current_dir()? }),
            )?;
        }
        return runtime.block_on(magistr::commands::misc::cmd_acp_agent(provider, &[]));
    }
    let parent = args
        .get(1)
        .map(PathBuf::from)
        .ok_or_else(|| eyre::eyre!("temporary project directory"))?;
    runtime.block_on(exercise(parent))
}
