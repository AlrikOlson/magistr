//! The one Magistr composition boundary over the native workflow use cases
//! and the workspace's source evidence (chunks arch-14f,
//! arch-14-verification-receipts and arch-14f2).
//!
//! Every real consumer of both capabilities lives in this crate — the
//! `magistr_gate` tool behind `magistr mcp-serve`, the loop driver's
//! between-iteration gates, the workspace bridge and the cockpit launcher —
//! so the boundary is composed here, in the entry-point crate that already
//! knows every concrete type, rather than in a crate with one consumer.
//!
//! What it binds: a workflow [`CheckAdmission`](magistr_engine::native::CheckAdmission)
//! — project, run, task and task instance captured *before* a gate runs,
//! under a durable [`CheckReceipt`](magistr_engine::native::CheckReceipt) —
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
//! The boundary also composes the host's side of the workspace: a
//! [`WorkspaceAdmission`] is what every source-writing host gets when it
//! tries to own the checkout for its run — a [`HostedWorkspace`], the one
//! native owner, with a [`WorkspaceSourceWriter`] over it as the route the
//! session's authorized source writes take (journaled intents and truthful
//! receipts, the same pipeline every workspace client writes through), or
//! a typed refusal under which those writes are refused rather than
//! written directly (chunk audit-260912-04). A workspace generation, a
//! source revision and a mutation receipt's revision remain three distinct
//! facts.
//!
//! Durability and completion policy live in the workflow: the receipt is
//! the workflow's record, and `roadmap_complete_chunk` refuses without a
//! current receipt for every required gate. Nothing here observes the tree
//! itself; the boundary is the consumer's typed shape over what the
//! workflow admitted. Serialization stays in the MCP and CLI adapters.

mod admitted_check;
mod check_evidence_error;
mod check_identities;
mod hosted_workspace;
mod interactive_host;
mod project_window;
mod settled_check;
mod workspace_admission;
mod workspace_source_writer;

pub use admitted_check::AdmittedCheck;
pub use check_evidence_error::CheckEvidenceError;
pub use check_identities::CheckIdentities;
pub use hosted_workspace::HostedWorkspace;
pub use interactive_host::InteractiveHost;
pub use project_window::ProjectWindow;
pub use settled_check::SettledCheck;
pub use workspace_admission::WorkspaceAdmission;
pub use workspace_source_writer::WorkspaceSourceWriter;

/// Workflow store clients and feeds for interface entry points.
pub mod workflow_client;
/// Workspace clients for interface entry points.
pub mod workspace_client;
