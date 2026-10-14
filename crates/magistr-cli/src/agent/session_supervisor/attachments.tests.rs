use super::*;
use magistr_application::sessions::PromptAttachment;

fn attached(text: &str, content: &str) -> PromptInput {
    PromptInput {
        text: text.into(),
        attachments: vec![
            PromptAttachment::from_bytes(
                "guide ü.md",
                "original-hash",
                content.as_bytes().to_vec(),
            )
            .unwrap(),
        ],
    }
}

#[tokio::test]
async fn queued_attachments_reach_the_provider_with_the_exact_admitted_bytes() {
    let mut fixture = Fixture::start("hold first").await;
    fixture.next_started().await;
    let mut input = attached("Review", "tenant A\r\n日本語\n");
    fixture.supervisor.send_followup(&input).unwrap();
    assert_eq!(fixture.supervisor.snapshot().queued_turns, 1);
    input.text = "changed draft".into();
    input.attachments = attached("", "tenant B").attachments;
    fixture.observed.finish.add_permits(1);
    fixture.next_terminal().await.unwrap();
    assert_eq!(
        fixture.next_started().await.0,
        "Review\nAttached file: guide ü.md\n\ntenant A\r\n日本語\n"
    );
    fixture.next_terminal().await.unwrap();
    assert_eq!(fixture.observed.maximum_active.load(Ordering::SeqCst), 1);
    fixture.close().await;
}

#[tokio::test]
async fn cancellation_discards_queued_file_bytes_before_the_next_turn() {
    let mut fixture = Fixture::start("hold first").await;
    fixture.next_started().await;
    fixture
        .supervisor
        .send_followup(&attached("Never send", "private queued bytes"))
        .unwrap();
    fixture.supervisor.cancel_turn().unwrap();
    fixture.next_terminal().await.unwrap();
    assert_eq!(fixture.supervisor.snapshot().queued_turns, 0);
    assert!(fixture.started.try_recv().is_err());
    fixture
        .supervisor
        .send_followup(&"Fresh turn".into())
        .unwrap();
    assert_eq!(fixture.next_started().await.0, "Fresh turn");
    fixture.next_terminal().await.unwrap();
    fixture.close().await;
}
