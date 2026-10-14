//! Staged prompt files and their exact-byte validation belong to the addressed
//! application session. Native surfaces supply selections; providers receive content.
use magistr_application::sessions::{
    MAX_ATTACHMENTS, PromptAttachment, PromptInput, PromptReceipt,
};
use magistr_workspace::{
    contracts::{ErrorKind, WorkspaceError},
    file_content::FileContent,
    service::WorkspaceService,
};
use std::path::PathBuf;

/// A pending attachment. Its source path is revalidated before queue admission;
/// pasted bytes have no filesystem authority.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StagedAttachment {
    content: PromptAttachment,
    path: Option<String>,
}
impl StagedAttachment {
    pub fn id(&self) -> &str {
        self.content.id()
    }
    pub fn content(&self) -> &PromptAttachment {
        &self.content
    }
    pub fn path(&self) -> Option<&str> {
        self.path.as_deref()
    }

    pub(super) fn sent(content: PromptAttachment) -> Self {
        Self::new(content, None)
    }
    pub(crate) fn new(content: PromptAttachment, path: Option<String>) -> Self {
        Self { content, path }
    }
}

pub(super) enum AttachmentSelection {
    Files(Vec<PathBuf>),
    Bytes { name: String, bytes: Vec<u8> },
}

impl AttachmentSelection {
    pub(super) fn prepare(
        self,
        source: Option<&WorkspaceService>,
    ) -> Result<Vec<StagedAttachment>, String> {
        match self {
            Self::Files(paths) => {
                if paths.len() > MAX_ATTACHMENTS {
                    return Err(format!("Add up to {MAX_ATTACHMENTS} files to one message."));
                }
                let source = source.ok_or_else(source_unavailable)?;
                paths
                    .into_iter()
                    .map(|path| {
                        let name = path
                            .to_str()
                            .ok_or_else(|| "This file name cannot be read.".to_owned())?;
                        let observed = source
                            .read_source(name, None)
                            .map_err(|error| explain(name, error))?;
                        let attachment = PromptAttachment::from_bytes(
                            observed.path.clone(),
                            observed.content_hash,
                            observed.content.as_bytes().to_vec(),
                        )?;
                        Ok(StagedAttachment::new(attachment, Some(observed.path)))
                    })
                    .collect()
            }
            Self::Bytes { name, bytes } => {
                let content = FileContent::decode(bytes);
                let hash = content.content_hash();
                let attachment =
                    PromptAttachment::from_bytes(name, hash, content.as_bytes().to_vec())?;
                Ok(vec![StagedAttachment::new(attachment, None)])
            }
        }
    }
}

pub(super) fn prepare_send(
    text: String,
    attachments: &[StagedAttachment],
    source: Option<&WorkspaceService>,
) -> Result<PromptInput, String> {
    for attachment in attachments {
        if let Some(path) = attachment.path() {
            source
                .ok_or_else(source_unavailable)?
                .read_source(path, Some(attachment.content().content_hash()))
                .map_err(|error| explain(path, error))?;
        }
    }
    Ok(PromptInput {
        text,
        attachments: attachments
            .iter()
            .map(|attachment| attachment.content.clone())
            .collect(),
    })
}

pub(super) fn matches_ids(attachments: &[StagedAttachment], ids: &[String]) -> bool {
    attachments.len() == ids.len()
        && attachments
            .iter()
            .zip(ids)
            .all(|(attachment, id)| attachment.id() == id)
}

#[cfg(test)]
pub(super) fn matches_input(attachments: &[StagedAttachment], input: &PromptInput) -> bool {
    attachments.len() == input.attachments.len()
        && attachments
            .iter()
            .zip(&input.attachments)
            .all(|(staged, sent)| staged.content() == sent)
}

pub(super) fn matches_receipt(attachments: &[StagedAttachment], receipt: &PromptReceipt) -> bool {
    attachments.len() == receipt.attachments.len()
        && attachments
            .iter()
            .zip(&receipt.attachments)
            .all(|(staged, sent)| {
                let content = staged.content();
                content.id() == sent.id
                    && content.name() == sent.name
                    && content.content_hash() == sent.content_hash
                    && content.kind() == sent.kind
                    && content.bytes().len() == sent.byte_len
            })
}

fn source_unavailable() -> String {
    "The project files are no longer available. Your message was not sent.".into()
}
fn explain(name: &str, error: WorkspaceError) -> String {
    match error.kind {
        ErrorKind::StaleAnchor { .. } => format!(
            "{name} changed after you attached it. Remove it and choose it again. Your message was not sent."
        ),
        ErrorKind::FileNotFound => {
            format!("{name} is missing or cannot be read. Your message was not sent.")
        }
        ErrorKind::BudgetTooSmall => format!(
            "{name} is too large. Choose a file of 1 MiB or less. Your message was not sent."
        ),
        ErrorKind::PathOutsideWorkspace => {
            "Choose a file inside this project. Your message was not sent.".into()
        }
        _ => format!("Cannot attach {name}: {}", error.message),
    }
}

#[cfg(test)]
#[path = "session_attachments.tests.rs"]
mod tests;
