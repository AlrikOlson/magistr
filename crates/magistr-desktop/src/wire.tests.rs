use super::*;

#[test]
fn saved_spending_is_visible_without_inventing_context_counts() {
    let usage = serde_json::to_value(Usage::from_report(None, Some(25))).unwrap();
    assert!(usage["used"].is_null());
    assert!(usage["size"].is_null());
    assert_eq!(usage["cost_cents"], 25);
}

#[test]
fn reported_zero_context_and_unknown_spending_stay_distinct() {
    let usage = serde_json::to_value(Usage::from_report(
        Some(magistr_application::sessions::ContextUsage { used: 0, size: 100 }),
        None,
    ))
    .unwrap();
    assert_eq!(usage["used"], 0);
    assert_eq!(usage["size"], 100);
    assert!(usage["cost_cents"].is_null());
}
#[test]
fn catalog_frames_preserve_full_ownership_and_omit_transcript_and_request_details() {
    let summary = magistr::application::SessionSummary {
        address: SessionAddress {
            project: "project".into(),
            workspace: "copy".into(),
            session: "resident".into(),
        },
        name: "Project".into(),
        owned_conversations: vec!["chat-history".into(), "planning-history".into()],
        conversation: Some(magistr_application::sessions::Conversation {
            id: "planning-history".into(),
            purpose: SessionPurpose::Roadmap,
            name: Some("Plan the next release".into()),
            provider: "copilot".into(),
        }),
        status: magistr::application::SessionStatus::Working,
        closed: false,
        model: None,
        work: None,
        usage: Some(Msg::Agent(AgentEvent::Usage {
            context: None,
            cost_cents: Some(0),
            elapsed_secs: None,
        })),
        requests: vec![magistr::application::WaitingRequest {
            id: 7,
            title: "Read".into(),
        }],
        activity: Some(magistr_application::sessions::SessionActivity {
            active: true,
            queued_turns: 2,
            configuration_in_progress: false,
            cancellation_requested: false,
            can_resume: Some(false),
            can_send_images: None,
        }),
    };
    let frame = CatalogFrame::from_catalog(
        9,
        &SessionCatalog {
            sequence: 42,
            sessions: vec![summary],
        },
    );
    let json = serde_json::to_value(frame).unwrap();
    assert_eq!(json["stream"], 9);
    assert_eq!(json["sequence"], 42);
    let session = &json["sessions"][0];
    assert_eq!(session["address"]["project"], "project");
    assert_eq!(session["address"]["workspace"], "copy");
    assert_eq!(session["address"]["session"], "resident");
    assert_eq!(session["status"], "Waiting for you");
    assert_eq!(session["target"]["address"], session["address"]);
    assert_eq!(session["target"]["conversation"], "planning-history");
    assert_eq!(session["conversation"]["purpose"], "roadmap");
    assert_eq!(
        session["owned_conversations"],
        serde_json::json!(["chat-history", "planning-history"])
    );
    assert_eq!(session["pending"][0]["id"], 7);
    assert!(session["pending"][0].get("detail").is_none());
    assert!(session.get("events").is_none());
    assert_eq!(session["usage"]["cost_cents"], 0);
    assert!(session["usage"]["used"].is_null());
    assert_eq!(session["activity"]["can_resume"], false);
    assert!(session["activity"]["can_send_images"].is_null());
}
#[test]
fn admissions_preserve_the_real_cursor_files_and_refusal_from_the_owner() {
    use magistr::application::SubmissionAdmission;
    use magistr_application::sessions::{PromptAttachment, PromptInput};
    let input = PromptInput {
        text: "hello".into(),
        attachments: vec![
            PromptAttachment::from_bytes("日本.txt", "hash", b"private bytes".to_vec()).unwrap(),
        ],
    }
    .receipt();
    let attachment_id = input.attachments[0].id.clone();
    let accepted = Admission::from(&SubmissionAdmission {
        sequence: 4,
        input: input.clone(),
        accepted: true,
    });
    let rejected = Admission::from(&SubmissionAdmission {
        sequence: 5,
        input,
        accepted: false,
    });
    let value = serde_json::to_value(accepted).unwrap();
    assert_eq!(value["sequence"], 4);
    assert_eq!(value["text"], "hello");
    assert_eq!(value["accepted"], true);
    assert_eq!(value["attachments"][0]["id"], attachment_id);
    assert_eq!(value["attachments"][0]["name"], "日本.txt");
    assert_eq!(value["attachments"][0]["content_hash"], "hash");
    assert!(!value.to_string().contains("private bytes"));
    assert!(!rejected.accepted);
}

#[test]
fn attachment_previews_seed_once_and_reseed_for_a_new_subscription() {
    use magistr_application::sessions::PromptAttachment;
    let image = PromptAttachment::from_bytes(
        "sample.png",
        "image-hash",
        b"\x89PNG\r\n\x1a\npreview bytes".to_vec(),
    )
    .unwrap();
    let text = PromptAttachment::from_bytes(
        "notes.txt",
        "text-hash",
        "日本の文章".repeat(100).into_bytes(),
    )
    .unwrap();
    let initial = [
        Attachment::from_content(&image, Some("sample.png"), &[]),
        Attachment::from_content(&text, None, &[]),
    ];
    assert!(
        initial[0]
            .preview
            .as_ref()
            .unwrap()
            .starts_with("data:image/png;base64,")
    );
    assert_eq!(initial[1].excerpt.as_ref().unwrap().chars().count(), 240);
    let seen = initial
        .iter()
        .map(|file| file.id.clone())
        .collect::<Vec<_>>();
    // Transcript frames may advance repeatedly without changing staged files.
    for _ in 0..3 {
        for content in [&image, &text] {
            let next = Attachment::from_content(content, None, &seen);
            assert!(next.preview.is_none());
            assert!(next.excerpt.is_none());
            assert_eq!(next.size, content.bytes().len());
            assert_eq!(next.content_hash, content.content_hash());
        }
    }
    let reseeded = Attachment::from_content(&image, None, &[]);
    assert_eq!(reseeded.preview, initial[0].preview);
    // Re-adding identical bytes creates a new stage identity and needs a new preview.
    let new_image =
        PromptAttachment::from_bytes(image.name(), image.content_hash(), image.bytes().to_vec())
            .unwrap();
    assert_ne!(new_image.id(), image.id());
    let added_again = Attachment::from_content(&new_image, None, &seen);
    assert_eq!(added_again.preview, initial[0].preview);
}
