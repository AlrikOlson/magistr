//! The one Magistr composition boundary over the native workflow use cases
//! and the workspace's source evidence (chunks arch-14f and
//! arch-14-verification-receipts).
//!
//! Every real consumer of both capabilities lives in this crate — the
//! `magistr_gate` tool behind `magistr mcp-serve`, the loop driver's
//! between-iteration gates, the workspace bridge and the cockpit launcher —
//! so the boundary is composed here, in the entry-point crate that already
//! knows every concrete type, rather than in a crate with one consumer.
//!
//! What it binds: a workflow [`CheckAdmission`](magistr_engine::tas::CheckAdmission)
//! — project, run, task and task instance captured *before* a gate runs,
//! under a durable [`CheckReceipt`](magistr_engine::tas::CheckReceipt) —
//! whose source the workflow observed through the evidence capability the
//! writer was admitted with ([`WorkspaceEvidence`](magistr_core::evidence::WorkspaceEvidence),
//! composed by [`writer_config`](magistr_core::evidence::writer_config)).
//! Settlement re-observes the tree and settles the check against the
//! captured admission, so a result is never reassigned to a task started
//! while the gate ran and the source it certifies is named by content, not
//! by clock. The two identities stay side by side ([`CheckIdentities`]): a
//! workflow run and a workspace generation are different authorities and
//! are never folded into one revision.
//!
//! Durability and completion policy live in the workflow: the receipt is
//! the workflow's record, and `roadmap_complete_chunk` refuses without a
//! current receipt for every required gate. Nothing here observes the tree
//! itself; the boundary is the consumer's typed shape over what the
//! workflow admitted. Serialization stays in the MCP and CLI adapters.

mod admitted_check;
mod check_evidence_error;
mod check_identities;
mod settled_check;

pub use admitted_check::AdmittedCheck;
pub use check_evidence_error::CheckEvidenceError;
pub use check_identities::CheckIdentities;
pub use settled_check::SettledCheck;
