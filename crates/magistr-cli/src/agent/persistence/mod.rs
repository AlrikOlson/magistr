//! Session intelligence — SQLite-backed session storage and querying.
//!
//! Provides persistent session history across runs:
//! - **Types** — `SessionRecord`, `SessionFilter`, `SessionSummary`
//! - **Storage** — SQLite operations (insert, query, purge, tag)
//! - **Resume** — Smart session resume context with git change detection
//!
//! Storage location: `~/.local/share/magistr/sessions.db`

pub mod commit_info;
pub mod resume;
pub mod retention_policy;
pub mod session_filter;
pub mod session_record;
pub mod session_summary;
pub mod session_turn;
pub mod storage;

#[allow(unused_imports)]
pub(crate) use commit_info::CommitInfo;
#[allow(unused_imports)] // Public API — used by TUI resume screen and agent session setup
pub(crate) use resume::ResumeContext;
#[allow(unused_imports)]
pub(crate) use retention_policy::RetentionPolicy;
#[allow(unused_imports)]
pub(crate) use session_filter::SessionFilter;
#[allow(unused_imports)]
pub(crate) use session_record::SessionRecord;
#[allow(unused_imports)]
pub(crate) use session_summary::SessionSummary;
#[allow(unused_imports)]
pub(crate) use session_turn::{SessionTurn, TurnRole};
#[allow(unused_imports)] // Public API — some used by tests and future agent integration
pub(crate) use storage::{
    count_sessions, count_turns, default_db_path, delete_turns, get_session, insert_session,
    insert_turn, open_db, purge_old_sessions, query_sessions, query_turns, set_session_owner,
    tag_session,
};
