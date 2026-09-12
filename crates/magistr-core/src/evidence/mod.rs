//! The host's verification evidence capability, composed once.
//!
//! The workflow's ship family asks its [`EvidencePort`](magistr_engine::native::EvidencePort)
//! two things: what the workspace's selected source looks like now, and
//! which gates the project requires. Magistr answers both from capabilities
//! it already owns — the workspace crate's lock-free
//! [`SourceSnapshot`](magistr_workspace::snapshot::SourceSnapshot) under its
//! default selection policy (which hashes `.magistr/project.json` as a
//! config input, so a gate-configuration edit moves the revision too) and
//! the same gate resolver `magistr_gate all` runs. There is no second
//! scanner: a receipt's source revision is the snapshot's.
//!
//! [`writer_config`] is how every production writer admission attaches the
//! capability; a writer opened without it records source-less checks and
//! cannot complete a chunk.

mod snapshot_witness;
mod workspace_evidence;
mod writer_config;

pub use snapshot_witness::SnapshotWitness;
pub use workspace_evidence::WorkspaceEvidence;
pub use writer_config::writer_config;
