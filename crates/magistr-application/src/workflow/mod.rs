//! Curated native roadmap operations. Native lifecycle rules and records remain
//! adapter-owned; these references carry identity, never proof of completion.

mod chunk_reference;
mod complete_chunk;
mod service;
mod start_chunk;
mod task_reference;
mod workflow_port;

pub use chunk_reference::ChunkReference;
pub use complete_chunk::CompleteChunk;
pub use service::Workflow;
pub use start_chunk::StartChunk;
pub use task_reference::TaskReference;
pub use workflow_port::WorkflowPort;

fn validate_key(key: &str) -> Result<(), crate::ApplicationError> {
    if key.trim().is_empty() || key.contains(':') || key.chars().any(char::is_control) {
        return Err(crate::ApplicationError::InvalidRequest {
            reason:
                "native reference needs a nonempty local key without a prefix or control characters"
                    .into(),
        });
    }
    Ok(())
}
