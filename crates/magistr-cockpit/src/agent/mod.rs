//! The agent's side of the cockpit: what an ACP session says, in the
//! cockpit's own words, and what the cockpit says back.
//!
//! [`AgentEvent`] is the vocabulary `update` sees — text and thought
//! deltas, tool calls and their updates, the plan, usage, the end of a
//! turn, a permission request — translated from magistr-acp's events at
//! the boundary so no protocol type reaches the core. [`spawn_session`]
//! relays an application session handle's observations and carries the core's
//! commands (prompts, cancels, permission answers) back to its owner; the
//! harness's fake agent scripts the same events directly.

pub mod ask_everything;
pub mod ask_plan;
pub mod edit_diff;
pub mod event;
pub mod image_block;
pub mod permission_choice;
pub mod permission_kind;
pub mod policy_decision;
pub mod session;
pub mod session_jobs;
pub mod session_options;
pub mod session_policy;
pub mod tool_record;
pub mod tool_status;
pub mod tool_title;
pub mod tool_verb;
pub mod translate;
pub mod turn_end;

pub use ask_everything::AskEverything;
pub use ask_plan::AskPlan;
pub use edit_diff::EditDiff;
pub use event::AgentEvent;
pub use image_block::ImageBlock;
pub use permission_choice::PermissionChoice;
pub use permission_kind::PermissionKind;
pub use policy_decision::PolicyDecision;
pub use session::spawn_session;
pub use session_jobs::SessionJobs;
pub use session_options::SessionOptions;
pub use session_policy::SessionPolicy;
pub use tool_record::ToolRecord;
pub use tool_status::ToolStatus;
pub use tool_title::ToolTitle;
pub use tool_verb::ToolVerb;
pub use translate::translate;
pub use turn_end::TurnEnd;
