//! The 11 `#[tool]` handler methods, all in one `#[tool_router]` impl.
//!
//! The macro requires every annotated tool to live in a single impl
//! block. Description strings stay inline — the `#[tool]` macro accepts
//! only string literals (no `const`/`path` expressions), so a separate
//! `descriptions.rs` would require a build script. Inline is cleaner.
//!
//! Description shape (all 11 follow the same template):
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
    StatusArgs, StepLookupArgs,
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
            Err(error) if error.error == "engine mutex poisoned" => Err(Self::poisoned()),
            Err(error) => {
                // Keep the native diagnostic intact alongside the wire recovery contract.
                let mut result = crate::infra::tool_failure::ToolFailure::uncertain(
                    "step_rejected",
                    &error.error,
                );
                if let Some(body) = result
                    .structured_content
                    .as_mut()
                    .and_then(serde_json::Value::as_object_mut)
                {
                    body.insert("error".into(), serde_json::json!(error.error));
                    body.insert("status".into(), serde_json::json!(error.status));
                    body.insert("hint".into(), serde_json::json!(error.hint));
                }
                result.content = vec![ContentBlock::text(
                    result
                        .structured_content
                        .as_ref()
                        .expect("structured failure")
                        .to_string(),
                )];
                Ok(result)
            }
        }
    }

    #[tool(
        name = "think_engine_status",
        description = "Inspect engine state — config, counts, version, and what the MCP client on the other end of this very request declared. Use to check whether persistence is on, which session is active, how big the trace is, and — via `client` — whether a capability like elicitation or the SEP-2663 tasks extension is actually live on this connection.\n\nInputs: see inputSchema. Optional `verbose: true` folds in pinned[] and sessions[] arrays.\n\nReturns: structuredContent shaped by outputSchema (EngineStatusOutput). `next_step_number` is the next safe `step_number` for think_record_step. The `client` block carries name/version, `capability_source` (`request_meta` | `initialize` | `none`), `declares_elicitation`, `elicitation_modes`, `declares_tasks`, `extensions[]` and the server's own `asking_enabled` gate.\n\nPitfalls: `total_steps` is a COUNT of retained steps, NOT the highest step number used — the numbering is sparse, so it can be hundreds below the head. Numbering a new step from `total_steps` writes an orphan disconnected from the trace. Use `next_step_number`, or simply omit `step_number` on think_record_step and let the engine append. `next_step_number` is monotonic across trimming and restarts but resets after think_wipe_trace. Engine config reflects the live process — env-var changes won't show until the server restarts. The `client` block does NOT: it is read fresh from this request. `declares_elicitation` can be true while `elicitation_modes` is empty, because rmcp reads modes from the initialize handshake only — the modes are the gate that decides whether you are actually asked.",
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
        let mut v = self
            .use_cases()
            .status(args.verbose.unwrap_or(false))
            .map_err(|_| Self::poisoned())?;
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
        description = "Export the trace in a human-readable format. Pick `markdown` (default, badged steps) for review, `json` for downstream tooling, `console` for ANSI-styled terminal output, `tree` for branch structure.\n\nInputs: see inputSchema. Optional `format`: \"markdown\" | \"json\" | \"console\" | \"tree\".\n\nReturns: text content. NOTE: this tool intentionally has no outputSchema because the shape depends on the chosen format.\n\nPitfalls: `console` includes ANSI escapes — only useful if the consumer renders them. For programmatic consumers, prefer `json`.",
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
        match self
            .use_cases()
            .export(args.format.as_deref().unwrap_or(""))
        {
            Ok(text) => Ok(CallToolResult::success(vec![ContentBlock::text(text)])),
            Err(error) if error == "engine mutex poisoned" => Err(Self::poisoned()),
            Err(error) => Ok(CallToolResult::error(vec![ContentBlock::text(error)])),
        }
    }

    #[tool(
        name = "think_get_step",
        description = "Fetch one step by its project-global step_number. Use to inspect a specific step, including branch steps.\n\nInputs: see inputSchema. Required: `step_number`. Optional: `resolve_latest: true` walks revised_by forward to return the live canonical revision instead of the historical original.\n\nReturns: structuredContent — a ThinkStep object (see outputSchema).\n\nPitfalls: returns a structured error envelope (error_kind=\"step_not_found\") when no step exists. Branch steps share the project-global numbering — there's no separate index.",
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
            .map_err(|_| Self::poisoned())?
        {
            Some(step) => Self::native_result(Ok(step), "step_not_found"),
            None => Ok(Self::structured_err(
                "step_not_found",
                format!("No step #{} found.", args.step_number),
            )),
        }
    }

    #[tool(
        name = "think_search_trace",
        description = "Case-insensitive substring search across every step's thought, outcome, context, purpose, rationale, and next_action. Use when the trace has grown past the recent_steps window and you need to locate an earlier claim.\n\nInputs: see inputSchema. Required: `query`. Optional: `limit` (default 10, min 1).\n\nReturns: structuredContent shaped by outputSchema (SearchTraceOutput) — query, match_count, matches[] with per-hit excerpt and matched_field.\n\nPitfalls: substring search is literal, not semantic. Lowercase the needle yourself if your query mixes case sensitivity expectations. Excerpts are ~60 chars around the match — use think_get_step for full text.",
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
        let hits = self
            .use_cases()
            .search(&args.query, args.limit.unwrap_or(10).max(1) as usize)
            .map_err(|_| Self::poisoned())?;
        Ok(CallToolResult::structured(serde_json::json!({
            "query": args.query,
            "match_count": hits.len(),
            "matches": hits,
        })))
    }

    #[tool(
        name = "think_step_impact",
        description = "Dependency + revision graph around a step. Use BEFORE revising a load-bearing step — answers \"if I change this, what else re-breaks?\".\n\nInputs: see inputSchema. Required: `step_number`.\n\nReturns: structuredContent shaped by outputSchema (StepImpactOutput) — upstream (direct + transitive deps), downstream (direct + transitive dependents, partitioned by relation label), revision_chain (original → revised-by → ...), branches_from.\n\nPitfalls: returns structured error envelope (error_kind=\"step_not_found\") when no step exists. Transitive walks are capped at 256 nodes — pathological cycles get truncated, not infinite-looped.",
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
        description = "Pin a step so it stays in recent_steps rollups even after the chronological window moves past it. Use for the original problem statement, confirmed root causes, anchoring conclusions. Pass pinned=false to unpin.\n\nInputs: see inputSchema. Required: `step_number`. Optional: `pinned` (defaults to true).\n\nReturns: structuredContent shaped by outputSchema (PinStepOutput) — step_number, was_pinned, now_pinned.\n\nPitfalls: pinning is local to the trace; pinned status persists with the session file when persistence is enabled.",
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
            .map(|previous| PinStepOutput {
                step_number: args.step_number,
                was_pinned: previous,
                now_pinned: pinned,
            });
        Self::native_result(result, "step_not_found")
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
        let result = self.use_cases().revise_estimate(args.estimated_total).map(
            |(previous, new_estimate)| ReviseEstimateOutput {
                previous,
                new_estimate,
                reason: args
                    .reason
                    .as_deref()
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(str::to_owned),
            },
        );
        Self::native_result(result, "no_steps")
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
            .map(|(previous, new)| SetBranchStatusOutput {
                branch_id: args.branch_id,
                previous_status: previous.to_string(),
                new_status: new.to_string(),
                merged_into: if args.status.trim().eq_ignore_ascii_case("merged") {
                    args.merged_into
                } else {
                    None
                },
            });
        Self::native_result(result, "unknown_branch")
    }

    #[tool(
        name = "think_trace_checkpoint",
        description = "Trace-wide metacognitive snapshot. Aggregates per-step warnings into patterns that only show up across the whole history — open hypotheses with no validation downstream, stale branches, the confidence trend, revisions whose dependents didn't acknowledge them, and steps that transitively depend on something refuted later. Also reports MEMORY MAINTENANCE health: how big the pinned set has grown, how hard it is pressing on the rollup budget, and which pins have stopped earning their slot.\n\nInputs: none.\n\nReturns: structuredContent shaped by outputSchema (TraceCheckpointOutput) — open_hypotheses, stale_branches, confidence_trend, revised_but_undefended, refuted_chain_alerts, maintenance.\n\nThe `maintenance` block is a PROPOSAL and nothing else: it names `decay_candidates` (pins outside the rollup budget that nothing depends on and no revision touches) and the verb that disposes of one — `think_pin_step(step_number, pinned: false)`. The server never clears a pin by itself, and unpinning never deletes a step; the step stays in the trace and stays fetchable with think_get_step.\n\nPitfalls: confidence_trend reports `insufficient_data` when fewer than 2 confidence values exist; not an error. `decay_candidates` is capped — read `decay_candidates_total` for the real count. Call this periodically on long traces — per-step warnings can't see these whole-trace patterns.",
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
        Parameters(_): Parameters<NoArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        Self::native_result(self.use_cases().checkpoint(), "engine_unavailable")
    }

    #[tool(
        name = "think_wipe_trace",
        description = "Wipe the trace: steps, branches, sessions, active-session pointer, and persisted files. DESTRUCTIVE — there is no undo, and on-disk session files are deleted too.\n\nInputs: none.\n\nReturns: structuredContent shaped by outputSchema (WipeTraceOutput) — { cleared: true }.\n\nPitfalls: this is intentionally a separate, destructively-named tool. Do not call to \"start fresh\" on a session — use a new session_id instead, which leaves the old trace recoverable.",
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
        self.use_cases().wipe().map_err(|_| Self::poisoned())?;
        Self::native_result(Ok(WipeTraceOutput { cleared: true }), "engine_unavailable")
    }
}
