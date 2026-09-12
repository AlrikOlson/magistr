//! The one Magistr composition boundary over the native workflow use cases
//! and the workspace's source evidence (chunk arch-14f).
//!
//! Every real consumer of both capabilities lives in this crate — the
//! `magistr_gate` tool behind `magistr mcp-serve`, the loop driver's
//! between-iteration gates, the workspace bridge and the cockpit launcher —
//! so the boundary is composed here, in the entry-point crate that already
//! knows every concrete type, rather than in a crate with one consumer.
//!
//! What it binds: a workflow [`CheckAdmission`](magistr_engine::tas::CheckAdmission)
//! — project, run, task and task instance captured *before* a gate runs —
//! and, when the consumer asks for it, a lock-free
//! [`SourceSnapshot`](magistr_workspace::snapshot::SourceSnapshot) of the
//! workspace's selected bytes taken at the same moment. Settlement re-observes
//! the tree and settles the check against the captured admission, so a
//! result is never reassigned to a task started while the gate ran and the
//! source it certifies is named by content, not by clock. The two identities
//! stay side by side ([`CheckIdentities`]): a workflow run and a workspace
//! generation are different authorities and are never folded into one
//! revision.
//!
//! Nothing here is durable. A [`SettledCheck`] is the in-memory answer the
//! consumer renders or records through the ordinary check; durable
//! receipts, freshness policy at completion and the decision of which
//! checks capture source belong to `arch-14-verification-receipts`.
//! Serialization stays in the MCP and CLI adapters.

mod admitted_check;
mod check_evidence_error;
mod check_identities;
mod check_source;
mod settled_check;

pub use admitted_check::AdmittedCheck;
pub use check_evidence_error::CheckEvidenceError;
pub use check_identities::CheckIdentities;
pub use check_source::CheckSource;
pub use settled_check::SettledCheck;
