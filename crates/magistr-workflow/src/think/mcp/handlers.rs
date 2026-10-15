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
    BranchStatusArgs, ExportArgs, ImpactArgs, NoArgs, PinArgs, ReviseEstimateArgs, SearchArgs,
    SessionArgs, StatusArgs, StepLookupArgs,
};
use crate::mcp::client_view::ClientView;
use crate::think::domain::ThinkStep;
use crate::think::output_schemas::{
    PinStepOutput, ReviseEstimateOutput, SetBranchStatusOutput, WipeTraceOutput,
};

use super::service::ThinkService;

/// Pub-crate bridge to the `#[tool_router]`-generated `tool_router()`
/// helper. The macro emits it as a private associated function in this
/// module; sibling modules (notably `super::service::ThinkService::new`)
/// can't reach across the privacy boundary on their own. This re-exports
/// it under a stable name without touching the macro.
impl ThinkService {
    pub(crate) fn make_tool_router() -> rmcp::handler::server::router::tool::ToolRouter<Self> {
        Self::tool_router()
    }
}

#[tool_router]
impl ThinkService {
    #[tool(
        name = "think_sessions",
        description = "List the project's saved reasoning sessions, including the default session.\n\nInputs: none.\n\nReturns: summaries and the source read. Use think_session to read a complete session.\n\nPitfalls: a failed read is reported as unavailable; an empty list means no saved sessions.",
        annotations(
            title = "Saved reasoning",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    pub async fn think_sessions(
        &self,
        Parameters(_): Parameters<NoArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        let result = self.use_cases().sessions().map(|read| {
            read.map(|data| {
                data.sessions
                    .into_iter()
                    .map(|history| crate::think::output_schemas::SessionDescriptor {
                        session_id: history.session_id,
                        step_count: history.steps.len(),
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
        name = "think_session",
        description = "Read a complete saved reasoning session.\n\nInputs: optional session_id; omit for the default session.\n\nReturns: authored steps, branch metadata and source.\n\nPitfalls: a missing session returns value:null; an unreadable source returns an error.",
        annotations(
            title = "Saved reasoning",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    pub async fn think_session(
        &self,
        Parameters(args): Parameters<SessionArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        Self::native_result(
            self.use_cases().session(args.session_id.as_deref()),
            "source_unavailable",
        )
    }

    #[tool(
        name = "think_record_step",
        description = "Record reasoning.\n\nInputs: estimated_total>=1; nonblank purpose, context, thought, outcome, next_action, rationale as separate JSON fields. next_action may be an object with nonblank action. Omit step_number, including for branches/revisions; explicit numbers use think_engine_status.next_step_number, never total_steps.\n\nReturns: structuredContent: step_number, total_steps, excerpts, warnings, recent_steps. Fetch pins in recent_steps_omitted with think_get_step. Honor isError.\n\nPitfalls: invalid_args gives field problems and effect:none; correct before retry. Never embed XML such as </thought><outcome> in thought or invent missing text. Read back uncertain writes before retry.\n\nBranches: start with branch_from=earlier step, branch_name=question; continue with returned branch.id as branch_id. Omit branch fields for the main line. Record synthesis with dependencies, then think_set_branch_status(status=merged, merged_into=synthesis step). abandoned drops the investigation. execution_ref links task, action or check.",
        annotations(
            title = "Record reasoning step",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = false,
        )
    )]
    pub async fn think_record_step(
        &self,
        Parameters(step): Parameters<ThinkStep>,
    ) -> Result<CallToolResult, ErrorData> {
        match self.use_cases().record_step(step) {
            Ok(recorded) => Ok(CallToolResult::structured(recorded.report)),
            Err(error) => Self::step_failure(error, "step_rejected"),
        }
    }

    #[tool(
        name = "think_engine_status",
        description = "Inspect saved reasoning, runtime settings and this request's client capabilities.\n\nInputs: optional session_id (omit for default), verbose (include pins and session summaries).\n\nReturns: counts, session_present, source, next_step_number, settings and client capabilities.\n\nPitfalls: total_steps counts one session; never use it to number a new step. Omit step_number when recording, or use next_step_number. A missing session differs from an unreadable source. Settings are captured at startup; client capabilities are read per request. Elicitation requires declared modes as well as support. Numbering resets after an explicit wipe.",
        annotations(
            title = "Engine snapshot",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false,
        )
    )]
    pub async fn think_engine_status(
        &self,
        Parameters(args): Parameters<StatusArgs>,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, ErrorData> {
        let mut v = match self
            .use_cases()
            .status(args.verbose.unwrap_or(false), args.session_id.as_deref())
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
        name = "think_export_trace",
        description = "Export a complete saved reasoning session.\n\nInputs: optional session_id (omit for default), format (markdown, json, console or tree; default markdown).\n\nReturns: text in the selected format; JSON retains authored records and tree shows branches.\n\nPitfalls: console includes ANSI escapes. Missing sessions and unreadable sources return errors. This read never changes stored work.",
        annotations(
            title = "Export trace",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false,
        )
    )]
    pub async fn think_export_trace(
        &self,
        Parameters(args): Parameters<ExportArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        match self.use_cases().export(
            args.format.as_deref().unwrap_or(""),
            args.session_id.as_deref(),
        ) {
            Ok(text) => Ok(CallToolResult::success(vec![ContentBlock::text(text)])),
            Err(error) if error == "engine mutex poisoned" => Err(Self::poisoned()),
            Err(error) => Ok(CallToolResult::error(vec![ContentBlock::text(error)])),
        }
    }

    #[tool(
        name = "think_get_step",
        description = "Read one complete step by its project-global number, across all saved sessions.\n\nInputs: step_number; optional resolve_latest follows the revision chain.\n\nReturns: the authored ThinkStep with its record and session identity.\n\nPitfalls: a missing step returns step_not_found; unreadable data or a broken revision chain returns source_unavailable. Branch steps share the same numbering. No memory-window limit applies.",
        annotations(
            title = "Fetch step",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false,
        )
    )]
    pub async fn think_get_step(
        &self,
        Parameters(args): Parameters<StepLookupArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        match self
            .use_cases()
            .get_step(args.step_number, args.resolve_latest.unwrap_or(false))
        {
            Ok(Some(step)) => Self::native_result(Ok(step), "step_not_found"),
            Err(error) => Ok(Self::structured_err("source_unavailable", error)),
            Ok(None) => Ok(Self::structured_err(
                "step_not_found",
                format!("No step #{} found.", args.step_number),
            )),
        }
    }

    #[tool(
        name = "think_search_trace",
        description = "Search all saved reasoning sessions, including steps outside recent memory.\n\nInputs: query (case-insensitive literal substring), limit (default 10, minimum 1).\n\nReturns: matching step numbers, session and record identities, matched fields and excerpts. Searches thought, outcome, context, purpose, rationale, next action and execution reference.\n\nPitfalls: excerpts are partial; use think_get_step for the full step. Unreadable data returns an error.",
        annotations(
            title = "Search trace",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false,
        )
    )]
    pub async fn think_search_trace(
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
        name = "think_step_impact",
        description = "Inspect dependencies and revisions before changing a reasoning step.\n\nInputs: step_number (project-global).\n\nReturns: upstream and downstream dependencies, relation labels, revision chain and branches from the step.\n\nPitfalls: missing steps return an error. Graph walks stop at 256 nodes; large or cyclic graphs can be partial. Reads use complete saved records across sessions.",
        annotations(
            title = "Step impact graph",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false,
        )
    )]
    pub async fn think_step_impact(
        &self,
        Parameters(args): Parameters<ImpactArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        Self::native_result(self.use_cases().impact(args.step_number), "step_not_found")
    }

    #[tool(
        name = "think_pin_step",
        description = "Pin a step so it stays in recent_steps rollups even after the chronological window moves past it. Use for the original problem statement, confirmed root causes, anchoring conclusions. Pass pinned=false to unpin.\n\nInputs: see inputSchema. Required: `step_number`. Optional: `pinned` (defaults to true).\n\nReturns: structuredContent shaped by outputSchema (PinStepOutput) — step_number, was_pinned, now_pinned.\n\nPitfalls: pinning is local to the trace; pinned status is saved with the history when persistence is enabled.",
        annotations(
            title = "Pin/unpin step",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false,
        )
    )]
    pub async fn think_pin_step(
        &self,
        Parameters(args): Parameters<PinArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        let pinned = args.pinned.unwrap_or(true);
        let result = self
            .use_cases()
            .pin_step(args.step_number, pinned)
            .map(|committed| PinStepOutput {
                step_number: args.step_number,
                was_pinned: committed.value,
                now_pinned: pinned,
                source: committed.source,
            });
        Self::mutation_result(result, "step_not_found")
    }

    #[tool(
        name = "think_revise_estimate",
        description = "Adjust `estimated_total` on the most recently recorded step, in place — no new step is appended. Use when you realize the work will take more (or fewer) steps than you initially declared.\n\nInputs: see inputSchema. Required: `estimated_total` (>= 1). Optional: `reason` (free text, surfaced back in the response).\n\nReturns: structuredContent shaped by outputSchema (ReviseEstimateOutput) — previous, new_estimate, reason.\n\nPitfalls: fails (error_kind=\"no_steps\") if called before any think_record_step. Only the LAST step's estimated_total is touched; older steps keep their historical estimates.",
        annotations(
            title = "Adjust total estimate",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = false,
        )
    )]
    pub async fn think_revise_estimate(
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
        Self::mutation_result(result, "no_steps")
    }

    #[tool(
        name = "think_set_branch_status",
        description = "Mark a branch \"active\", \"merged\", or \"abandoned\". Steps stay; only the branch label changes. When status=\"merged\", optionally pass merged_into=<step_number> to record which synthesis step aggregated the branch back into the main line.\n\nInputs: see inputSchema. Required: `branch_id`, `status` (one of: active, merged, abandoned). Optional: `merged_into` (step number).\n\nReturns: structuredContent shaped by outputSchema (SetBranchStatusOutput) — branch_id, previous_status, new_status, merged_into.\n\nPitfalls: unknown branch_id returns structured error envelope with error_kind=\"unknown_branch\" (and lists known ids). Moving away from \"merged\" clears any prior merged_into pointer so stale pointers don't survive.",
        annotations(
            title = "Set branch status",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false,
        )
    )]
    pub async fn think_set_branch_status(
        &self,
        Parameters(args): Parameters<BranchStatusArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        let result = self
            .use_cases()
            .set_branch_status(&args.branch_id, &args.status, args.merged_into)
            .map(|committed| SetBranchStatusOutput {
                branch_id: args.branch_id,
                previous_status: committed.value.previous,
                new_status: committed.value.current,
                source: committed.source,
                merged_into: committed.value.merged_into,
            });
        Self::mutation_result(result, "unknown_branch")
    }

    #[tool(
        name = "think_trace_checkpoint",
        description = "Review unresolved hypotheses, stale branches, confidence changes, unsupported revisions and refuted dependencies in a saved reasoning session.\n\nInputs: optional session_id; omit for default.\n\nReturns: findings, session_present, source and memory-maintenance proposals.\n\nPitfalls: proposals never change pins or delete steps. Use think_pin_step to accept an unpin proposal. Lists may be bounded; use their total counts. Fewer than two confidence values yields insufficient_data. A missing session is explicit; unreadable data is an error.",
        annotations(
            title = "Trace checkpoint",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false,
        )
    )]
    pub async fn think_trace_checkpoint(
        &self,
        Parameters(args): Parameters<SessionArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        Self::native_result(
            self.use_cases().checkpoint(args.session_id.as_deref()),
            "source_unavailable",
        )
    }

    #[tool(
        name = "think_wipe_trace",
        description = "Wipe the trace: steps, branches, sessions, active-session pointer, and saved histories. DESTRUCTIVE — there is no undo.\n\nInputs: none.\n\nReturns: structuredContent shaped by outputSchema (WipeTraceOutput) — { cleared: true }.\n\nPitfalls: this is intentionally a separate, destructively-named tool. Do not call to \"start fresh\" on a session — use a new session_id instead, which leaves the old trace recoverable.",
        annotations(
            title = "Wipe trace",
            read_only_hint = false,
            destructive_hint = true,
            idempotent_hint = true,
            open_world_hint = false,
        )
    )]
    pub async fn think_wipe_trace(
        &self,
        Parameters(_): Parameters<NoArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        let result = self.use_cases().wipe().map(|committed| WipeTraceOutput {
            cleared: true,
            source: committed.source,
        });
        Self::mutation_result(result, "engine_unavailable")
    }
}
