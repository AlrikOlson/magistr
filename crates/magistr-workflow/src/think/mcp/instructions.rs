//! The `instructions` field returned in the MCP `initialize` response.
//!
//! Some clients surface this in the model's system prompt; others log it
//! and move on. Either way it should give a model that has never seen this
//! family before a tight orientation that prevents the most common misuse
//! patterns.

pub const SERVER_INSTRUCTIONS: &str = r#"think_* records structured, branching, revisable reasoning.

When to call which tool:
  - Recording a new thought             → think_record_step
  - Adjusting the total estimate         → think_revise_estimate
  - Pinning a load-bearing conclusion    → think_pin_step
  - Marking a branch active/merged/dead  → think_set_branch_status
  - Checking trace-wide health           → think_trace_checkpoint
  - Fetching a specific step             → think_get_step
  - Searching across the trace           → think_search_trace
  - Computing blast radius of revision   → think_step_impact
  - Engine introspection                 → think_engine_status
  - Exporting the trace                  → think_export_trace
  - Wiping everything (destructive)      → think_wipe_trace

For a distinct question, record a step with branch_from pointing to the
earlier step and branch_name naming the question. Continue with the returned
branch.id as branch_id; its original question and origin are kept. Omit all
branch fields to return to the main line. When findings are used, record a
synthesis step with dependencies on the relevant branch steps, then call
think_set_branch_status with status merged and merged_into set to that
synthesis step. Use abandoned only when the investigation itself was dropped;
rejecting an idea does not mean abandoning the investigation. A revision uses
revises_step and revision_reason. Omit step_number on every new record,
including branch members and revisions; the server allocates a unique number.

Step numbers are project-global; never reuse. Pin the original problem
statement so later steps keep it in view. Every JSON-returning tool
advertises an outputSchema and emits structuredContent — prefer parsing
that over the text content.

When using the ship_* family alongside this one, pass `execution_ref` on
think_record_step to link reasoning to execution (e.g.
"task:auth-refactor", "action:42"). Search finds execution_ref values."#;
