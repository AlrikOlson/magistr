//! The native goal loop's judge — the pure core (goal-judge-core).
//!
//! After an iteration `magistr next` asks a *fresh* model whether the
//! chunk is done. What the model sees is a [`JudgeInput`]: the chunk's
//! whole acceptance list, unabridged, and the real gate outcomes as
//! [`GateEvidence`] — never a transcript's retelling of them. What it
//! answers is a [`Verdict`] in Claude Code's own three shapes, so a
//! delegated `/goal` mode can report through the same type. And what
//! the loop *does* is a [`Decision`], reconciled from the verdict and
//! the gates in one pure function: a judge saying "met" over a red gate
//! is a disagreement to report, never a completion — `ship_check`'s
//! exit codes are the authority, on every mode.
//!
//! [`SessionJudge`] is the one transport: a fresh, tool-less ACP session
//! per verdict on the shared session owner, its permission asks refused
//! by [`NoTools`], its words read as a verdict only when the owner's
//! [`JudgeEvidence`] — a confirmed `EndTurn` and a settled cleanup —
//! admits them. The driver that acts on the decision is the next slice.

pub mod agent_goal;
pub mod decision;
pub mod delegation;
pub mod delivery;
pub mod delivery_kind;
pub mod gate_evidence;
pub mod goal_mode;
pub mod judge;
pub mod judge_evidence;
pub mod judge_input;
pub mod no_tools;
pub mod session_judge;
pub mod verdict;

pub use agent_goal::AgentGoal;
pub use decision::Decision;
pub use delegation::Delegation;
pub use delivery::Delivery;
pub use delivery_kind::DeliveryKind;
pub use gate_evidence::GateEvidence;
pub use goal_mode::GoalMode;
pub use judge::Judge;
pub use judge_evidence::JudgeEvidence;
pub use judge_input::JudgeInput;
pub use no_tools::NoTools;
pub use session_judge::SessionJudge;
pub use verdict::Verdict;
