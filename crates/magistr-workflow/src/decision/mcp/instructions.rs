//! The `instructions` field returned in the MCP `initialize` response.
//!
//! Some clients surface this in the model's system prompt; others log it
//! and move on. Either way it should give a model that has never seen this
//! family before a tight orientation that prevents the most common misuse
//! patterns.

pub const SERVER_INSTRUCTIONS: &str = r#"decision_* records structured, threading, revisable reasoning.

When to call which tool:
  - Recording a new thought             → decision_record
  - Adjusting the total estimate         → decision_revise_estimate
  - Pinning a load-bearing conclusion    → decision_pin
  - Marking a thread open/merged/dead  → decision_set_thread_status
  - Checking log-wide health           → decision_review
  - Fetching a specific note             → decision_get
  - Searching across the log           → decision_search
  - Computing blast radius of revision   → decision_impact
  - Engine introspection                 → decision_status
  - Exporting the log                  → decision_export
  - Wiping everything (destructive)      → decision_wipe

For a distinct question, record a note with thread_from pointing to the
earlier note and thread_name naming the question. Continue with the returned
thread.id as thread_id; its original question and origin are kept. Omit all
thread fields to return to the main line. When findings are used, record a
synthesis note with dependencies on the relevant thread notes, then call
decision_set_thread_status with status merged and merged_into set to that
synthesis note. Use abandoned only when the investigation itself was dropped;
rejecting an idea does not mean abandoning the investigation. A revision uses
revises_note and revision_reason. Omit note_number on every new record,
including thread members and revisions; the server allocates a unique number.

Note numbers are project-global; never reuse. Pin the original problem
statement so later notes keep it in view. Every JSON-returning tool
advertises an outputSchema and emits structuredContent — prefer parsing
that over the text content.

When using the run_* family alongside this one, pass `run_ref` on
decision_record to link reasoning to execution (e.g.
"step:auth-refactor", "progress:42"). Search finds run_ref values."#;
