//! The reasoning `#[tool]` handler methods, all in one `#[tool_router]` impl.
//!
//! The macro requires every annotated tool to live in a single impl
//! block. Description strings stay inline — the `#[tool]` macro accepts
//! only string literals (no `const`/`path` expressions), so a separate
//! `descriptions.rs` would require a build script. Inline is cleaner.
//!
//! Description shape:
//!
//!   1. When to use — one short sentence orienting tool selection.
//!   2. Inputs — references the inputSchema; lists required vs optional.
//!   3. Returns — names the outputSchema type and its key fields.
//!   4. Pitfalls — known failure modes and how to avoid them.

use rmcp::{
    ErrorData, RoleServer,
    handler::server::wrapper::Parameters,
    model::{CallToolResult, ContentBlock},
    service::RequestContext,
    tool, tool_router,
};

use super::args::{
    ExportArgs, ImpactArgs, LogArgs, NoArgs, NoteLookupArgs, PinArgs, ReviseEstimateArgs,
    SearchArgs, StatusArgs, ThreadStatusArgs,
};
use crate::decision::domain::Note;
use crate::decision::output_schemas::{
    PinOutput, ReviseEstimateOutput, SetThreadStatusOutput, WipeOutput,
};
use crate::mcp::client_view::ClientView;

use super::service::DecisionService;

/// Pub-crate bridge to the `#[tool_router]`-generated `tool_router()`
/// helper. The macro emits it as a private associated function in this
/// module; sibling modules (notably `super::service::DecisionService::new`)
/// can't reach across the privacy boundary on their own. This re-exports
/// it under a stable name without touching the macro.
impl DecisionService {
    pub(crate) fn make_tool_router() -> rmcp::handler::server::router::tool::ToolRouter<Self> {
        Self::tool_router()
    }
}

#[tool_router]
impl DecisionService {
    #[tool(
        name = "decision_logs",
        description = "List the project's saved reasoning logs, including the default log.\n\nInputs: none.\n\nReturns: summaries and the source read. Use decision_log to read a complete log.\n\nPitfalls: a failed read is reported as unavailable; an empty list means no saved logs.",
        annotations(
            title = "Saved reasoning",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    pub async fn decision_logs(
        &self,
        Parameters(_): Parameters<NoArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        let result = self.use_cases().logs().map(|read| {
            read.map(|data| {
                data.logs
                    .into_iter()
                    .map(|history| crate::decision::output_schemas::LogDescriptor {
                        log_id: history.log_id,
                        note_count: history.notes.len(),
                        completed: history.completed,
                        created_at: history.created_at,
                        updated_at: history.updated_at,
                    })
                    .collect::<Vec<_>>()
            })
        });
        Self::native_result(result, "source_unavailable")
    }

    #[tool(
        name = "decision_log",
        description = "Read a complete saved reasoning log.\n\nInputs: optional log_id; omit for the default log.\n\nReturns: authored notes, thread metadata and source.\n\nPitfalls: a missing log returns value:null; an unreadable source returns an error.",
        annotations(
            title = "Saved reasoning",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    pub async fn decision_log(
        &self,
        Parameters(args): Parameters<LogArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        Self::native_result(
            self.use_cases().log(args.log_id.as_deref()),
            "source_unavailable",
        )
    }

    #[tool(
        name = "decision_record",
        description = "Record reasoning.\n\nInputs: estimated_total>=1; nonblank purpose, context, thought, outcome, next_action, rationale as separate JSON fields. next_action may be an object with nonblank action. Omit note, including for threads/revisions; explicit numbers use decision_status.next_note_number, never total_notes.\n\nReturns: structuredContent: note, total_notes, excerpts, warnings, recent_notes. Fetch pins in recent_notes_omitted with decision_get. Honor isError.\n\nPitfalls: invalid_args gives field problems and effect:none; correct before retry. Never embed XML such as </thought><outcome> in thought or invent missing text. Read back uncertain writes before retry.\n\nThreads: start with thread_from=earlier note, thread_name=question; continue with returned thread.id as thread_id. Omit thread fields for the main line. Record synthesis with dependencies, then decision_set_thread_status(status=merged, merged_into=synthesis note). abandoned drops the investigation. run_ref links task, action or check.",
        annotations(
            title = "Record reasoning note",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = false,
        )
    )]
    pub async fn decision_record(
        &self,
        Parameters(note): Parameters<Note>,
    ) -> Result<CallToolResult, ErrorData> {
        match self.use_cases().record(note) {
            Ok(recorded) => Ok(CallToolResult::structured(recorded.report)),
            Err(error) => Self::note_failure(error, "note_rejected"),
        }
    }

    #[tool(
        name = "decision_status",
        description = "Inspect saved reasoning, runtime settings and this request's client capabilities.\n\nInputs: optional log_id (omit for default), verbose (include pins and log summaries).\n\nReturns: counts, log_present, source, next_note_number, settings and client capabilities.\n\nPitfalls: total_notes counts one log; never use it to number a new note. Omit note when recording, or use next_note_number. A missing log differs from an unreadable source. Settings are captured at startup; client capabilities are read per request. Elicitation requires declared modes as well as support. Numbering resets after an explicit wipe.",
        annotations(
            title = "Engine snapshot",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false,
        )
    )]
    pub async fn decision_status(
        &self,
        Parameters(args): Parameters<StatusArgs>,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, ErrorData> {
        let mut v = match self
            .use_cases()
            .status(args.verbose.unwrap_or(false), args.log_id.as_deref())
        {
            Ok(value) => value,
            Err(error) => return Ok(Self::structured_err("source_unavailable", error)),
        };
        // Client capability observation is transport state, never native state.
        if let (Some(map), Ok(view)) = (
            v.as_object_mut(),
            serde_json::to_value(ClientView::observe(&context)),
        ) {
            map.insert("client".to_string(), view);
        }
        Ok(CallToolResult::structured(v))
    }

    #[tool(
        name = "decision_export",
        description = "Export a complete saved reasoning log.\n\nInputs: optional log_id (omit for default), format (markdown, json, console or tree; default markdown).\n\nReturns: text in the selected format; JSON retains authored records and tree shows threads.\n\nPitfalls: console includes ANSI escapes. Missing logs and unreadable sources return errors. This read never changes stored work.",
        annotations(
            title = "Export log",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false,
        )
    )]
    pub async fn decision_export(
        &self,
        Parameters(args): Parameters<ExportArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        match self
            .use_cases()
            .export(args.format.as_deref().unwrap_or(""), args.log_id.as_deref())
        {
            Ok(text) => Ok(CallToolResult::success(vec![ContentBlock::text(text)])),
            Err(error) if error == "engine mutex poisoned" => Err(Self::poisoned()),
            Err(error) => Ok(CallToolResult::error(vec![ContentBlock::text(error)])),
        }
    }

    #[tool(
        name = "decision_get",
        description = "Read one complete note by its project-global number, across all saved logs.\n\nInputs: note; optional resolve_latest follows the revision chain.\n\nReturns: the authored Note with its record and log identity.\n\nPitfalls: a missing note returns note_not_found; unreadable data or a broken revision chain returns source_unavailable. Thread notes share the same numbering. No memory-window limit applies.",
        annotations(
            title = "Fetch note",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false,
        )
    )]
    pub async fn decision_get(
        &self,
        Parameters(args): Parameters<NoteLookupArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        match self
            .use_cases()
            .get_note(args.note, args.resolve_latest.unwrap_or(false))
        {
            Ok(Some(note)) => Self::native_result(Ok(note), "note_not_found"),
            Err(error) => Ok(Self::structured_err("source_unavailable", error)),
            Ok(None) => Ok(Self::structured_err(
                "note_not_found",
                format!("No note #{} found.", args.note),
            )),
        }
    }

    #[tool(
        name = "decision_search",
        description = "Search all saved reasoning logs, including notes outside recent memory.\n\nInputs: query (case-insensitive literal substring), limit (default 10, minimum 1).\n\nReturns: matching note numbers, log and record identities, matched fields and excerpts. Searches thought, outcome, context, purpose, rationale, next action and execution reference.\n\nPitfalls: excerpts are partial; use decision_get for the full note. Unreadable data returns an error.",
        annotations(
            title = "Search log",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false,
        )
    )]
    pub async fn decision_search(
        &self,
        Parameters(args): Parameters<SearchArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        let hits = match self
            .use_cases()
            .search(&args.query, args.limit.unwrap_or(10).max(1) as usize)
        {
            Ok(value) => value,
            Err(error) => return Ok(Self::structured_err("source_unavailable", error)),
        };
        Ok(CallToolResult::structured(serde_json::json!({
            "query": args.query,
            "match_count": hits.len(),
            "matches": hits,
        })))
    }

    #[tool(
        name = "decision_impact",
        description = "Inspect dependencies and revisions before changing a reasoning note.\n\nInputs: note (project-global).\n\nReturns: upstream and downstream dependencies, relation labels, revision chain and threads from the note.\n\nPitfalls: missing notes return an error. Graph walks stop at 256 nodes; large or cyclic graphs can be partial. Reads use complete saved records across logs.",
        annotations(
            title = "Note impact graph",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false,
        )
    )]
    pub async fn decision_impact(
        &self,
        Parameters(args): Parameters<ImpactArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        Self::native_result(self.use_cases().impact(args.note), "note_not_found")
    }

    #[tool(
        name = "decision_pin",
        description = "Pin a note so it stays in recent_notes rollups even after the chronological window moves past it. Use for the original problem statement, confirmed root causes, anchoring conclusions. Pass pinned=false to unpin.\n\nInputs: see inputSchema. Required: `note`. Optional: `pinned` (defaults to true).\n\nReturns: structuredContent shaped by outputSchema (PinOutput) — note, was_pinned, now_pinned.\n\nPitfalls: pinning is local to the log; pinned status is saved with the history when persistence is enabled.",
        annotations(
            title = "Pin/unpin note",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false,
        )
    )]
    pub async fn decision_pin(
        &self,
        Parameters(args): Parameters<PinArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        let pinned = args.pinned.unwrap_or(true);
        let result = self
            .use_cases()
            .pin(args.note, pinned)
            .map(|committed| PinOutput {
                note: args.note,
                was_pinned: committed.value,
                now_pinned: pinned,
                source: committed.source,
            });
        Self::mutation_result(result, "note_not_found")
    }

    #[tool(
        name = "decision_revise_estimate",
        description = "Adjust `estimated_total` on the most recently recorded note, in place — no new note is appended. Use when you realize the work will take more (or fewer) notes than you initially declared.\n\nInputs: see inputSchema. Required: `estimated_total` (>= 1). Optional: `reason` (free text, surfaced back in the response).\n\nReturns: structuredContent shaped by outputSchema (ReviseEstimateOutput) — previous, new_estimate, reason.\n\nPitfalls: fails (error_kind=\"no_notes\") if called before any decision_record. Only the LAST note's estimated_total is touched; older notes keep their historical estimates.",
        annotations(
            title = "Adjust total estimate",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = false,
        )
    )]
    pub async fn decision_revise_estimate(
        &self,
        Parameters(args): Parameters<ReviseEstimateArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        let result = self
            .use_cases()
            .revise_estimate(args.estimated_total)
            .map(|committed| ReviseEstimateOutput {
                previous: committed.value.0,
                new_estimate: committed.value.1,
                source: committed.source,
                reason: args
                    .reason
                    .as_deref()
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(str::to_owned),
            });
        Self::mutation_result(result, "no_notes")
    }

    #[tool(
        name = "decision_set_thread_status",
        description = "Mark a thread \"open\", \"merged\", or \"abandoned\". Notes stay; only the thread label changes. When status=\"merged\", optionally pass merged_into=<note> to record which synthesis note aggregated the thread back into the main line.\n\nInputs: see inputSchema. Required: `thread_id`, `status` (one of: open, merged, abandoned). Optional: `merged_into` (note number).\n\nReturns: structuredContent shaped by outputSchema (SetThreadStatusOutput) — thread_id, previous_status, new_status, merged_into.\n\nPitfalls: unknown thread_id returns structured error envelope with error_kind=\"unknown_thread\" (and lists known ids). Moving away from \"merged\" clears any prior merged_into pointer so stale pointers don't survive.",
        annotations(
            title = "Set thread status",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false,
        )
    )]
    pub async fn decision_set_thread_status(
        &self,
        Parameters(args): Parameters<ThreadStatusArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        let result = self
            .use_cases()
            .set_thread_status(&args.thread_id, &args.status, args.merged_into)
            .map(|committed| SetThreadStatusOutput {
                thread_id: args.thread_id,
                previous_status: committed.value.previous,
                new_status: committed.value.current,
                source: committed.source,
                merged_into: committed.value.merged_into,
            });
        Self::mutation_result(result, "unknown_thread")
    }

    #[tool(
        name = "decision_review",
        description = "Review unresolved hypotheses, stale threads, confidence changes, unsupported revisions and refuted dependencies in a saved reasoning log.\n\nInputs: optional log_id; omit for default.\n\nReturns: findings, log_present, source and memory-maintenance proposals.\n\nPitfalls: proposals never change pins or delete notes. Use decision_pin to accept an unpin proposal. Lists may be bounded; use their total counts. Fewer than two confidence values yields insufficient_data. A missing log is explicit; unreadable data is an error.",
        annotations(
            title = "Log checkpoint",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false,
        )
    )]
    pub async fn decision_review(
        &self,
        Parameters(args): Parameters<LogArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        Self::native_result(
            self.use_cases().checkpoint(args.log_id.as_deref()),
            "source_unavailable",
        )
    }

    #[tool(
        name = "decision_wipe",
        description = "Wipe the log: notes, threads, logs, active-log pointer, and saved histories. DESTRUCTIVE — there is no undo.\n\nInputs: none.\n\nReturns: structuredContent shaped by outputSchema (WipeOutput) — { cleared: true }.\n\nPitfalls: this is intentionally a separate, destructively-named tool. Do not call to \"start fresh\" on a log — use a new log_id instead, which leaves the old log recoverable.",
        annotations(
            title = "Wipe log",
            read_only_hint = false,
            destructive_hint = true,
            idempotent_hint = true,
            open_world_hint = false,
        )
    )]
    pub async fn decision_wipe(
        &self,
        Parameters(_): Parameters<NoArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        let result = self.use_cases().wipe().map(|committed| WipeOutput {
            cleared: true,
            source: committed.source,
        });
        Self::mutation_result(result, "engine_unavailable")
    }
}
