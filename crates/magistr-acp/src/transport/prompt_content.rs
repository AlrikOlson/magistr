//! Resolve explicitly mentioned workspace files into prompt content.

use agent_client_protocol::schema::v1::{ContentBlock, ImageContent, TextContent};
use base64::Engine as _;
use std::path::Path;
use tokio::io::AsyncReadExt;

const FILE_BYTES: u64 = 256 * 1024;
const TOTAL_BYTES: usize = 8 * 1024 * 1024;
const IMAGE_BYTES: u64 = 4 * 1024 * 1024;
const FILE_COUNT: usize = 8;

/// A message and its bounded text attachments. No filesystem work occurs in the UI model.
pub(crate) struct PromptContent;

impl PromptContent {
    /// Read workspace-relative @paths, preserving the original message as the first block.
    /// Quoted paths use JSON escapes so spaces, quotes and Unicode round-trip.
    pub(crate) async fn resolve(
        text: &str,
        cwd: &Path,
        images: bool,
    ) -> Result<Vec<ContentBlock>, String> {
        let files = references(text)?;
        let mut blocks = vec![ContentBlock::Text(TextContent::new(text))];
        if files.is_empty() {
            return Ok(blocks);
        }
        if files.len() > FILE_COUNT {
            return Err(format!("Add up to {FILE_COUNT} files to one message."));
        }
        let root = tokio::fs::canonicalize(cwd).await.map_err(|_| {
            "The project folder is no longer available. Your message was not sent.".to_owned()
        })?;
        let mut total = 0;
        for name in files {
            let relative = Path::new(&name);
            if relative.is_absolute()
                || relative
                    .components()
                    .any(|part| matches!(part, std::path::Component::ParentDir))
            {
                return Err(format!(
                    "Cannot attach {name}: choose a file inside this project."
                ));
            }
            let path = tokio::fs::canonicalize(root.join(relative))
                .await
                .map_err(|_| {
                    format!("Cannot attach {name}: the file is missing or cannot be read.")
                })?;
            if !path.starts_with(&root) {
                return Err(format!(
                    "Cannot attach {name}: the file points outside this project."
                ));
            }
            let file = tokio::fs::File::open(&path)
                .await
                .map_err(|_| format!("Cannot attach {name}: the file cannot be read."))?;
            let metadata = file
                .metadata()
                .await
                .map_err(|_| format!("Cannot read {name}."))?;
            if !metadata.is_file() {
                return Err(format!(
                    "Cannot attach {name}: choose a file, not a folder."
                ));
            }
            let mut bytes = Vec::new();
            file.take(IMAGE_BYTES + 1)
                .read_to_end(&mut bytes)
                .await
                .map_err(|_| format!("Cannot read {name}."))?;
            total += bytes.len();
            if total > TOTAL_BYTES {
                return Err("The files exceed 8 MB. Remove a file and send again.".into());
            }
            if let Some(mime) = image_mime(&bytes) {
                if !images {
                    return Err(format!(
                        "Cannot attach {name}: this agent does not accept images."
                    ));
                }
                if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > IMAGE_BYTES {
                    return Err(format!(
                        "Cannot attach {name}: images must be smaller than 4 MB."
                    ));
                }
                blocks.push(ContentBlock::Text(TextContent::new(format!(
                    "Attached image: {name}"
                ))));
                blocks.push(ContentBlock::Image(ImageContent::new(
                    base64::engine::general_purpose::STANDARD.encode(&bytes),
                    mime,
                )));
                continue;
            }
            if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > FILE_BYTES {
                return Err(format!(
                    "Cannot attach {name}: text files must be smaller than 256 KB."
                ));
            }
            let body = String::from_utf8(bytes).map_err(|_| {
                format!("Cannot attach {name}: choose a text file or a PNG, JPEG or WebP image.")
            })?;
            if body.contains('\0') {
                return Err(format!(
                    "Cannot attach {name}: choose a text file or a PNG, JPEG or WebP image."
                ));
            }
            // Text is supported by every ACP provider; each file remains a separate
            // content block even when the provider has no embedded-resource support.
            blocks.push(ContentBlock::Text(TextContent::new(format!(
                "Attached file: {name}\n\n{body}"
            ))));
        }
        Ok(blocks)
    }
}

fn image_mime(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("image/png")
    } else if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        Some("image/jpeg")
    } else if bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP") {
        Some("image/webp")
    } else {
        None
    }
}

fn references(text: &str) -> Result<Vec<String>, String> {
    magistr_application::sessions::prompt_file_references(text, |tail| {
        let mut stream = serde_json::Deserializer::from_str(tail).into_iter::<String>();
        let name = stream
            .next()
            .transpose()
            .map_err(|_| "A file name has an unfinished quote. Choose the file again.".to_owned())?
            .ok_or_else(|| "Choose a file after @.".to_owned())?;
        Ok((name, stream.byte_offset()))
    })
}

#[cfg(test)]
#[path = "prompt_content.tests.rs"]
mod tests;
