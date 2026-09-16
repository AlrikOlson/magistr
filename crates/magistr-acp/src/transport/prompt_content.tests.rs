use super::*;

#[tokio::test]
async fn images_use_acp_image_blocks_and_respect_provider_capability() {
    let folder = tempfile::tempdir().expect("project");
    // The public capture's bytes supply a real decoded format rather than a filename guess.
    let png = base64::engine::general_purpose::STANDARD
        .decode("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+j8ioAAAAASUVORK5CYII=")
        .expect("png");
    tokio::fs::write(folder.path().join("picture.dat"), &png)
        .await
        .expect("image");
    assert!(
        PromptContent::resolve("@picture.dat", folder.path(), false)
            .await
            .expect_err("capability")
            .contains("does not accept images")
    );
    let blocks = PromptContent::resolve("@picture.dat", folder.path(), true)
        .await
        .expect("image content");
    let ContentBlock::Image(image) = &blocks[2] else {
        panic!("actual image block");
    };
    assert_eq!(image.mime_type, "image/png");
    assert_eq!(
        base64::engine::general_purpose::STANDARD
            .decode(&image.data)
            .expect("base64"),
        png
    );
    assert_eq!(image_mime(&[0xff, 0xd8, 0xff, 0xe0]), Some("image/jpeg"));
    assert_eq!(image_mime(b"RIFFxxxxWEBP"), Some("image/webp"));
}

use agent_client_protocol::schema::v1::{PromptRequest, SessionId};

#[tokio::test]
async fn actual_prompt_contains_selected_files_in_separate_blocks() {
    let folder = tempfile::tempdir().expect("project");
    tokio::fs::write(
        folder.path().join("retry guide.md"),
        "Keep each tenant separate.",
    )
    .await
    .expect("file");
    let draft = "Read @\"retry guide.md\" and check recovery.";
    let blocks = PromptContent::resolve(draft, folder.path(), false)
        .await
        .expect("attachments");
    let request = PromptRequest::new(SessionId::new("session"), blocks);
    let json = serde_json::to_value(request).expect("wire request");
    assert_eq!(json["prompt"][0]["text"], draft);
    assert_eq!(
        json["prompt"][1]["text"],
        "Attached file: retry guide.md\n\nKeep each tenant separate."
    );
}

#[tokio::test]
async fn missing_or_large_files_refuse_the_whole_message_with_a_reason() {
    let folder = tempfile::tempdir().expect("project");
    assert!(
        PromptContent::resolve("Read @missing.rs", folder.path(), false)
            .await
            .expect_err("missing")
            .contains("missing")
    );
    tokio::fs::write(
        folder.path().join("large.txt"),
        vec![b'x'; FILE_BYTES as usize + 1],
    )
    .await
    .expect("file");
    assert!(
        PromptContent::resolve("@large.txt", folder.path(), false)
            .await
            .expect_err("large")
            .contains("256 KB")
    );
}

#[tokio::test]
async fn text_without_files_needs_no_project_lookup() {
    let content =
        PromptContent::resolve("Contact person@example.com", Path::new("/missing"), false)
            .await
            .expect("ordinary message");
    assert_eq!(content.len(), 1);
}

#[tokio::test]
async fn attachment_paths_cannot_leave_the_project() {
    let folder = tempfile::tempdir().expect("project");
    assert!(
        PromptContent::resolve("@../private.txt", folder.path(), false)
            .await
            .expect_err("parent")
            .contains("inside this project")
    );
    assert!(
        PromptContent::resolve("@/private.txt", folder.path(), false)
            .await
            .expect_err("absolute")
            .contains("inside this project")
    );
    #[cfg(unix)]
    {
        let outside = tempfile::NamedTempFile::new().expect("outside");
        std::os::unix::fs::symlink(outside.path(), folder.path().join("link.txt")).expect("link");
        assert!(
            PromptContent::resolve("@link.txt", folder.path(), false)
                .await
                .expect_err("symlink")
                .contains("outside this project")
        );
    }
}

#[test]
fn quoted_unicode_and_escaped_names_round_trip_and_duplicates_are_sent_once() {
    assert_eq!(
        references("Read @\"résumé draft.md\" @a.rs @a.rs").expect("paths"),
        vec!["résumé draft.md", "a.rs"]
    );
    let path = "a \"quoted\" name.txt";
    let message = format!("@{}", serde_json::to_string(path).expect("json"));
    assert_eq!(references(&message).expect("escaped"), vec![path]);
    assert!(references("@\"unfinished").is_err());
}
