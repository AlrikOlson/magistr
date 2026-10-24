//! The workspace-owned source certification capability (arch-14e): a
//! [`SourceSnapshot`] names exactly which workspace, which owner generation
//! and which selected bytes an observation covered, and a
//! [`MutationReceipt`] states what a journaled mutation truthfully did to
//! the filesystem. Neither an index generation, a watcher event, a journal
//! sequence nor a file's mtime/size is a source revision here: the manifest
//! is content-addressed, the receipt is read from the resolved durable intent.
//!
//! Capture and validation read the tree without the journal writer or any
//! mutation lock, so a check may run between them. An [`Consistency::Optimistic`]
//! snapshot whose validation finds no change proves only that two
//! observations agreed; an isolated [`SourceExport`] is the one path to
//! [`Consistency::Immutable`] inputs. Consumption by workflow checks belongs
//! to `arch-14-verification-receipts`.
mod capture;
mod change_kind;
mod consistency;
mod digest_cache;
mod entry_kind;
mod manifest_entry;
mod mutation_receipt;
mod mutation_status;
mod receipt_file;
mod scheme;
mod selection_policy;
mod snapshot_archive;
mod snapshot_error;
mod snapshot_validation;
mod source_change;
mod source_export;
mod source_manifest;
mod source_snapshot;
mod workspace_identity;

pub use change_kind::ChangeKind;
pub use consistency::Consistency;
pub use entry_kind::EntryKind;
pub use manifest_entry::ManifestEntry;
pub use mutation_receipt::MutationReceipt;
pub use mutation_status::MutationStatus;
pub use receipt_file::ReceiptFile;
pub use scheme::SCHEME;
pub use selection_policy::SelectionPolicy;
pub use snapshot_error::SnapshotError;
pub use snapshot_validation::SnapshotValidation;
pub use source_change::SourceChange;
pub use source_export::SourceExport;
pub use source_manifest::SourceManifest;
pub use source_snapshot::SourceSnapshot;
pub use workspace_identity::WorkspaceIdentity;
pub(crate) use workspace_identity::frame;
