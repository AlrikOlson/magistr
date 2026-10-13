use super::*;
use crate::agent::persistence::{SessionHistory, session_outcome::SessionOutcome};

#[tokio::test]
async fn a_saved_session_updates_an_open_native_projection_after_refused_delivery() {
    let directory = tempfile::tempdir().unwrap();
    let first = directory.path().join("first");
    let second = directory.path().join("second");
    for root in [&first, &second] {
        std::fs::create_dir_all(root.join(".magistr")).unwrap();
        std::fs::write(root.join(".magistr/project.json"), "{}").unwrap();
    }
    let first = first.canonicalize().unwrap();
    let second = second.canonicalize().unwrap();
    let inputs = ProjectInputs::from_lookup(&first, |key| {
        (key == "XDG_DATA_HOME").then(|| directory.path().join("data").display().to_string())
    });
    let (inbox, mut updates) = Inbox::bounded(1);
    let feed = follow(first.clone(), inputs.clone(), inbox);
    tokio::time::sleep(Duration::from_millis(40)).await;
    let history = SessionHistory::new();
    history
        .begin(&inputs.for_project(&second), &second, "scripted", None)
        .await
        .unwrap();
    history.settle(SessionOutcome::Completed).await.unwrap();
    // The first snapshot occupies the queue when the next changed one is read.
    tokio::time::sleep(Duration::from_millis(1100)).await;
    let result = tokio::time::timeout(Duration::from_secs(3), async {
        while let Some(Msg::Job {
            output: JobOutput::Galaxy(data),
            ..
        }) = updates.recv().await
        {
            if data.projects.iter().any(|project| {
                project.dir.as_deref() == Some(second.display().to_string().as_str())
            }) {
                return;
            }
        }
        panic!("history projection closed before the saved project appeared");
    })
    .await;
    feed.abort();
    let _ = feed.await;
    result.expect("refused delivery retries the current projection");
}
