//! Encode application-owned prompt bytes without opening a filesystem path.
use agent_client_protocol::schema::v1::{ContentBlock, ImageContent, TextContent};
use base64::Engine as _;
use magistr_application::sessions::PromptInput;

pub(crate) fn encode(input: &PromptInput) -> Vec<ContentBlock> {
    let mut blocks = vec![ContentBlock::Text(TextContent::new(&input.text))];
    for file in &input.attachments {
        if let Some(mime) = file.mime_type() {
            blocks.push(ContentBlock::Text(TextContent::new(format!(
                "Attached image: {}",
                file.name()
            ))));
            blocks.push(ContentBlock::Image(ImageContent::new(
                base64::engine::general_purpose::STANDARD.encode(file.bytes()),
                mime,
            )));
        } else {
            let body = file
                .text()
                .expect("text attachments contain validated UTF-8");
            blocks.push(ContentBlock::Text(TextContent::new(format!(
                "Attached file: {}\n\n{body}",
                file.name()
            ))));
        }
    }
    blocks
}

#[cfg(test)]
#[path = "prompt_content.tests.rs"]
mod tests;
