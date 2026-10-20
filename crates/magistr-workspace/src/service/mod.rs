//! The native workspace composition root (chunk arch-14c10): one admitted
//! workspace per process, every typed use case behind one owner, and no
//! transport. Hosts — the MCP adapter today, the CLI verbs next — call these
//! methods; serialization stays in `response` and at the host's own edge.
//! Admission here is single-owner-per-process (an OS-held lock); the
//! multi-client owner protocol remains the following native integration.
mod index_build_state;
mod index_build_task;
mod journal_list;
mod journal_list_input;
mod lifecycle_lock;
mod lock_record;
mod open_error;
mod open_options;
mod owner_generation;
mod server_lock;
mod session_identity;
mod status_row;
mod tool_schemas;
mod workspace_service;
mod workspace_session;
mod workspace_upgrade;

pub use index_build_state::IndexBuildState;
pub use index_build_task::IndexBuildTask;
pub use journal_list_input::JournalListInput;
pub use lock_record::LockRecord;
pub use open_error::WorkspaceOpenError;
pub use open_options::WorkspaceOpenOptions;
pub use owner_generation::WorkspaceGeneration;
pub use server_lock::ServerLock;
pub use session_identity::mint_session_identity;
pub use status_row::StatusRow;
pub use tool_schemas::ToolSchemas;
pub use workspace_service::WorkspaceService;
pub use workspace_session::WorkspaceSession;
