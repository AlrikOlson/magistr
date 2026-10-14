use super::*;
use agent_client_protocol::schema::v1::{PromptRequest, SessionId};
use magistr_application::sessions::PromptAttachment;

#[test]
fn exact_text_and_image_bytes_are_distinct_prompt_blocks() {
    let text = "Keep each tenant separate.\r\n日本語\n";
    let png = base64::engine::general_purpose::STANDARD
        .decode("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+j8ioAAAAASUVORK5CYII=")
        .expect("png");
    let input = PromptInput {
        text: "Read the files and check recovery.".into(),
        attachments: vec![
            PromptAttachment::from_bytes("retry guide.md", "text-hash", text.as_bytes().to_vec())
                .unwrap(),
            PromptAttachment::from_bytes("picture.dat", "image-hash", png.clone()).unwrap(),
        ],
    };
    let request = PromptRequest::new(SessionId::new("session"), encode(&input));
    let json = serde_json::to_value(request).expect("wire request");
    assert_eq!(json["prompt"][0]["text"], input.text);
    assert_eq!(
        json["prompt"][1]["text"],
        format!("Attached file: retry guide.md\n\n{text}")
    );
    assert_eq!(json["prompt"][2]["text"], "Attached image: picture.dat");
    assert_eq!(json["prompt"][3]["mimeType"], "image/png");
    assert_eq!(
        base64::engine::general_purpose::STANDARD
            .decode(json["prompt"][3]["data"].as_str().unwrap())
            .unwrap(),
        png
    );
}

#[test]
fn file_mentions_remain_authored_text_without_implicit_reads() {
    let input: PromptInput = "Contact person@example.com about @missing.txt".into();
    let blocks = encode(&input);
    assert_eq!(blocks.len(), 1);
    let ContentBlock::Text(text) = &blocks[0] else {
        panic!("text");
    };
    assert_eq!(text.text, input.text);
}
