//! The host's verification evidence capability, composed once.
//!
//! The workflow's ship family asks its [`EvidencePort`](magistr_engine::native::EvidencePort)
//! two things: what the workspace's selected source looks like now, and
//! which gates the project requires. Magistr answers both from capabilities
//! it already owns — the workspace crate's lock-free
//! [`SourceSnapshot`](magistr_workspace::snapshot::SourceSnapshot) under its
//! default selection policy (which hashes `.magistr/project.json` as a
//! config input, so a gate-configuration edit moves the revision too), the
//! workspace's [`ConfigDigest`](magistr_workspace::config::ConfigDigest) over
//! captured external policy layers and the shared project's settings. A linked
//! working copy resolves settings from its main project directory; changing
//! them moves its configuration evidence with its source bytes unchanged.
//! Gate contracts use the same settings path and resolver `magistr_gate all` runs. There is
//! no second scanner: a receipt's source revision is the snapshot's and its
//! configuration digest is the layers'.
//!
//! [`writer_config_from_env`] captures production configuration and attaches
//! the capability; [`writer_config`] accepts explicit test inputs. A writer
//! opened without evidence records source-less checks and cannot complete a chunk.

mod snapshot_witness;
mod workspace_evidence;
mod writer_config;
mod writer_config_from_env;

pub use snapshot_witness::SnapshotWitness;
pub use workspace_evidence::WorkspaceEvidence;
pub use writer_config::writer_config;
pub use writer_config_from_env::writer_config_from_env;
