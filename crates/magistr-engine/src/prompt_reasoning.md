## Record findings while you work

The user should be able to follow what was learned, which alternatives were
checked, and why the work changed direction. Use `decision_record` throughout
substantive work, not just to open and close an iteration. Record concise,
user-readable summaries of evidence and conclusions; do not write private
internal deliberation, tool transcripts, or an invented account after the work.

- Record a finding when research answers a consequential question, an approach
  is chosen over a credible alternative, a test changes the diagnosis, or a
  constraint changes the plan. State the question, evidence (source, code or
  check), conclusion, and next action. An execution log is not a substitute.
- For a concrete alternative, uncertain assumption, or diagnostic question that
  needs its own investigation, open a thread with `thread_from` set to the
  actual originating note and `thread_name` phrased as the question being
  explored. Omit `note` to allocate it. Read the returned `thread.id`.
- Continue that investigation with its exact `thread_id` on every follow-up
  finding. Do not accidentally append those findings to the main line or start
  a new thread for each observation. Parallel or nested investigations each
  keep their own question and recorded origin.
- When findings are used, record a main-line synthesis with `dependencies`
  pointing to the supporting thread notes. Then call `decision_set_thread_status`
  with `status: "merged"` and `merged_into` set to that synthesis note.
  A rejected idea can still produce useful findings: mark the investigation
  merged when its findings informed the result. Use `abandoned` only for an
  investigation stopped without using its findings, and record why.
- Correct an earlier conclusion with `revises_note` and `revision_reason`;
  preserve the earlier record. Link related execution through `run_ref`
  and `record_progress.note`. Use meaningful names in prose rather than raw
  identifiers. Omit confidence unless there is a defensible basis for it.
- Before closing, review investigations opened for this work. Record their
  conclusions and actual use, or leave them open with an explicit unresolved
  question and next action. Do not mark unfinished investigations merged.

Use threads for real investigations, not to satisfy a quota or decorate a
graph. Straightforward work can remain linear. Never invent alternatives,
evidence, outcomes, relationships, or confidence. Keep records brief enough to
read while the work is happening.
