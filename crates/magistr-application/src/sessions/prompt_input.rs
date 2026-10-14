//! Immutable prompt content and its shared admission bounds. Attachments are
//! validated once from source-owner bytes and keep those bytes through queuing.
use alloc::{format, string::String, sync::Arc, vec::Vec};
use core::{
    fmt,
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT_ATTACHMENT: AtomicU64 = AtomicU64::new(1);

pub const MAX_PROMPT_BYTES: usize = 1024 * 1024;
pub const MAX_ATTACHMENT_BYTES: usize = 1024 * 1024;
pub const MAX_ATTACHMENT_NAME_BYTES: usize = 4096;
pub const MAX_ATTACHMENT_HASH_BYTES: usize = 128;
pub const MAX_ATTACHMENTS: usize = 8;
pub const MAX_ATTACHMENT_TOTAL_BYTES: usize = 8 * MAX_ATTACHMENT_BYTES;

#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct PromptInput {
    pub text: String,
    pub attachments: Vec<PromptAttachment>,
}

impl PromptInput {
    #[must_use]
    pub fn receipt(&self) -> PromptReceipt {
        PromptReceipt {
            text: self.text.clone(),
            attachments: self
                .attachments
                .iter()
                .map(|file| AttachmentReceipt {
                    id: file.id.clone(),
                    name: file.name.clone(),
                    content_hash: file.content_hash.clone(),
                    kind: file.kind(),
                    byte_len: file.bytes.len(),
                })
                .collect(),
        }
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.text.trim().is_empty() && self.attachments.is_empty()
    }

    #[must_use]
    pub fn byte_len(&self) -> usize {
        self.text.len()
            + self
                .attachments
                .iter()
                .map(|file| file.bytes.len())
                .sum::<usize>()
    }

    pub fn validate(&self, images: bool) -> Result<(), String> {
        if self.text.len() > MAX_PROMPT_BYTES {
            return Err("This message is too long. Send a smaller part.".into());
        }
        if self.attachments.len() > MAX_ATTACHMENTS {
            return Err(format!("Add up to {MAX_ATTACHMENTS} files to one message."));
        }
        if self
            .attachments
            .iter()
            .map(|file| file.bytes.len())
            .sum::<usize>()
            > MAX_ATTACHMENT_TOTAL_BYTES
        {
            return Err("The files exceed 8 MB. Remove a file and send again.".into());
        }
        if !images
            && let Some(file) = self
                .attachments
                .iter()
                .find(|file| file.mime_type.is_some())
        {
            return Err(format!(
                "Cannot attach {}: this agent does not accept images.",
                file.name
            ));
        }
        Ok(())
    }
}

impl From<String> for PromptInput {
    fn from(text: String) -> Self {
        Self {
            text,
            attachments: Vec::new(),
        }
    }
}

impl From<&str> for PromptInput {
    fn from(text: &str) -> Self {
        String::from(text).into()
    }
}

/// Accepted input identity without retained file payloads.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct PromptReceipt {
    pub text: String,
    pub attachments: Vec<AttachmentReceipt>,
}

/// The ordered file identity a surface uses to acknowledge its submitted draft.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct AttachmentReceipt {
    pub id: String,
    pub name: String,
    pub content_hash: String,
    pub kind: PromptAttachmentKind,
    pub byte_len: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PromptAttachmentKind {
    Text,
    Image,
}

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct PromptAttachment {
    id: String,
    name: String,
    content_hash: String,
    bytes: Arc<[u8]>,
    mime_type: Option<&'static str>,
}

impl PromptAttachment {
    pub fn from_bytes(
        name: impl Into<String>,
        content_hash: impl Into<String>,
        bytes: Vec<u8>,
    ) -> Result<Self, String> {
        let name = name.into();
        let content_hash = content_hash.into();
        if name.len() > MAX_ATTACHMENT_NAME_BYTES {
            return Err("This file name is too long. Choose a shorter name.".into());
        }
        if content_hash.is_empty() || content_hash.len() > MAX_ATTACHMENT_HASH_BYTES {
            return Err("This file could not be verified. Choose it again.".into());
        }
        if name.is_empty() {
            return Err("Choose a named file to attach.".into());
        }
        if bytes.len() > MAX_ATTACHMENT_BYTES {
            return Err(format!(
                "Cannot attach {name}: files must be 1 MB or smaller."
            ));
        }
        let mime_type = image_mime(&bytes);
        if mime_type.is_none() && (core::str::from_utf8(&bytes).is_err() || bytes.contains(&0)) {
            return Err(format!(
                "Cannot attach {name}: choose a text file or a PNG, JPEG or WebP image."
            ));
        }
        Ok(Self {
            id: format!(
                "attachment-{}",
                NEXT_ATTACHMENT.fetch_add(1, Ordering::Relaxed)
            ),
            name,
            content_hash,
            bytes: bytes.into(),
            mime_type,
        })
    }

    /// Identity of this selection, preserved by clones and acceptance receipts.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub fn content_hash(&self) -> &str {
        &self.content_hash
    }

    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    #[must_use]
    pub fn text(&self) -> Option<&str> {
        if self.mime_type.is_some() {
            None
        } else {
            core::str::from_utf8(&self.bytes).ok()
        }
    }

    #[must_use]
    pub fn mime_type(&self) -> Option<&'static str> {
        self.mime_type
    }

    #[must_use]
    pub fn kind(&self) -> PromptAttachmentKind {
        if self.mime_type.is_some() {
            PromptAttachmentKind::Image
        } else {
            PromptAttachmentKind::Text
        }
    }
}

impl fmt::Debug for PromptAttachment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PromptAttachment")
            .field("id", &self.id)
            .field("name", &self.name)
            .field("content_hash", &self.content_hash)
            .field("bytes", &self.bytes.len())
            .field("kind", &self.kind())
            .finish()
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

#[cfg(test)]
#[path = "prompt_input.tests.rs"]
mod tests;
