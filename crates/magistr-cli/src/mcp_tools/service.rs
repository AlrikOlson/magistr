//! MCP service for Magistr and native workflow tools.
//!
//! [`super::build_server`] supplies the `magistr_*` handlers alongside the
//! think-and-ship tool families served by [`UnifiedService`]. Other tools,
//! resources, and task methods delegate with the original request context.
//!
//! `magistr_gate` forwards checks to `ship_check` when a ship task is active,
//! recording their actual exit codes. Without a task, checks run directly and
//! report `recorded: false`. Both routes return structured output rows. Direct
//! runs stream PTY output to the cockpit; checks run inside the workflow
//! library do not currently stream output.
//!
//! `magistr_set_phase` validates ship state before transitioning and records
//! an action afterward. Execute and verify require an objective and task;
//! done requires a recorded check.
//!
//! Native engines open only through an admitted [`ProjectAuthority`] in the
//! service role: identity, the cooperative owner lease and the live feed are
//! settled before any engine opens, and a competing MCP service fails there.
//! The cockpit or loop host holds the other role beside this one. External
//! binaries that bypass the lease still share the underlying persistence.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use magistr_engine::tas::{ProjectAuthority, TasConfig, WriterRole};
use magistr_engine::{GateOutput, ShipGuardState};
use magistr_mcp::{SdkMcpServer, ToolContent, ToolResult};
use rmcp::model::{
    CallToolRequestParams, CallToolResponse, CallToolResult, CancelTaskParams, ContentBlock,
    GetTaskParams, GetTaskResult, Implementation, JsonObject, ListResourceTemplatesResult,
    ListResourcesResult, ListToolsResult, PaginatedRequestParams, ReadResourceRequestParams,
    ReadResourceResponse, ServerInfo, Tool, UpdateTaskParams,
};
use rmcp::service::{RequestContext, RoleServer};
use rmcp::{ErrorData, ServerHandler};
use serde_json::{Value, json};
use think_and_ship::infra::Family;
use think_and_ship::mcp::UnifiedService;

use super::gate::{GateSpec, resolve_gates, run_gate_command};
use super::roadmap_export::RoadmapExport;
use crate::utils::gate::nextest_junit_report_path;

const INSTRUCTIONS: &str = "magistr — project workflow server. \
`magistr_*` tools run quality gates (magistr_gate, recorded through ship_check), \
formatting, phase transitions, hooks and safe file operations for this project. \
The think_*/ship_*/roadmap_*/signal_* families below are think-and-ship, served \
in-process on the same store as Magistr's own views.";

/// The rmcp server behind `magistr mcp-serve`: the `magistr_*` tools plus the
/// four think-and-ship families, one tool list.
#[derive(Clone)]
pub struct MagistrService {
    project_root: PathBuf,
    tools: Arc<SdkMcpServer>,
    unified: UnifiedService,
    roadmap_export: RoadmapExport,
    /// The admission this service runs under: the project identity, the
    /// service-role lease, and the live-feed socket the engines broadcast on
    /// (kept so a gate this server runs itself can stream its bytes there
    /// too). Cloning a service shares it; the lease releases with the last
    /// clone.
    authority: Arc<ProjectAuthority>,
}

impl MagistrService {
    /// Open the project owner with environment-derived identity and store.
    ///
    /// On Unix, request the project's default live-feed socket. Other platforms
    /// keep their existing no-feed default; an explicit unsupported socket errors.
    /// Returns an error when the project is already owned or a requested feed
    /// cannot be bound. No engines are opened after failed admission.
    pub async fn for_project(project_root: PathBuf) -> std::io::Result<Self> {
        let project_root = project_root.canonicalize()?;
        let tas = TasConfig::from_env(&project_root);
        #[cfg(unix)]
        let tas = tas.with_project_socket_default();
        Self::new(project_root, &tas).await
    }

    /// Open one owner on an explicit configuration. Clones share the same engines
    /// and lease. Independent instances must acquire ownership before loading state.
    pub async fn new(project_root: PathBuf, tas: &TasConfig) -> std::io::Result<Self> {
        let project_root = project_root.canonicalize()?;
        let mut tas = tas.clone();
        tas.project_root = project_root.clone();
        let mut authority = ProjectAuthority::admit(&tas, WriterRole::Service)?;
        if let Some(path) = tas.broadcast_path.as_ref() {
            authority = authority.publish(path).await?;
        }
        let (unified, authority) = authority.serve();

        Ok(Self {
            roadmap_export: RoadmapExport::new(&project_root),
            tools: Arc::new(super::build_server(project_root.clone())),
            project_root,
            unified,
            authority,
        })
    }

    /// The project this server works in.
    #[must_use]
    pub fn project_root(&self) -> &Path {
        &self.project_root
    }

    /// The `magistr_*` tools as rmcp tool descriptors.
    fn magistr_tools(&self) -> Vec<Tool> {
        self.tools
            .list_tools()
            .into_iter()
            .map(|t| {
                let schema: JsonObject = t.input_schema().as_object().cloned().unwrap_or_default();
                Tool::new(
                    t.name().to_owned(),
                    t.description().to_owned(),
                    Arc::new(schema),
                )
            })
            .collect()
    }

    /// Run a `magistr_*` tool through its SdkMcpTool handler.
    async fn invoke(&self, name: &str, args: Value) -> Result<CallToolResult, ErrorData> {
        let tool = self
            .tools
            .get_tool(name)
            .ok_or_else(|| ErrorData::invalid_params(format!("unknown tool '{name}'"), None))?;
        let result = tool
            .invoke(args)
            .await
            .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;
        Ok(to_call_tool_result(result))
    }

    async fn call_magistr(
        &self,
        request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, ErrorData> {
        let name = request.name.to_string();
        let args = Value::Object(request.arguments.clone().unwrap_or_default());
        match name.as_str() {
            "magistr_gate" => self.gate(&args, context).await,
            "magistr_set_phase" => {
                let Some(phase) = args.get("phase").and_then(Value::as_str) else {
                    return self.invoke(&name, args).await;
                };
                if let Some(reason) = self.ship_phase_block(phase, context.clone()).await {
                    return Ok(to_call_tool_result(ToolResult::error(reason)));
                }
                let result = self.invoke(&name, args.clone()).await?;
                if result.is_error != Some(true) {
                    self.record_phase(phase, context).await;
                }
                Ok(result)
            }
            _ => self.invoke(&name, args).await,
        }
    }

    /// `magistr_gate`: resolve the gate set from project.json and run each
    /// gate through `ship_check` when a task is active, directly otherwise.
    async fn gate(
        &self,
        args: &Value,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, ErrorData> {
        let Some(gate) = args.get("gate").and_then(Value::as_str) else {
            return Err(ErrorData::invalid_params("missing 'gate' parameter", None));
        };
        let timeout_secs = args
            .get("timeout_secs")
            .and_then(Value::as_u64)
            .unwrap_or(300);
        let config = crate::validate::load_project_config(&self.project_root).map_err(|e| {
            ErrorData::internal_error(format!("Failed to load project config: {e}"), None)
        })?;
        let specs = match resolve_gates(&config, gate) {
            Ok(specs) => specs,
            Err(message) => return Ok(to_call_tool_result(ToolResult::error(message))),
        };

        let mut results = Vec::with_capacity(specs.len());
        for spec in &specs {
            results.push(self.run_gate(spec, timeout_secs, context.clone()).await?);
        }

        // A polyglot project resolves one standard gate to several specs —
        // any multi-spec answer takes the grouped shape, not only `all`.
        let body = if gate == "all" || results.len() != 1 {
            let all_passed = results.iter().all(|r| {
                let passed = r["passed"].as_bool().unwrap_or(false);
                let required = r["required"].as_bool().unwrap_or(true);
                passed || !required
            });
            // A run the trace did not see must say so at the top, not only
            // on each gate — a green `all` that recorded nothing is the
            // failure this flag exists for.
            let unrecorded: Vec<&Value> = results
                .iter()
                .filter(|g| g["recorded"] == Value::Bool(false))
                .collect();
            let recorded = unrecorded.is_empty();
            let note = unrecorded_all_note(&unrecorded);
            let mut body = json!({ "passed": all_passed, "recorded": recorded, "gates": results });
            if let Some(note) = note {
                body["note"] = json!(note);
            }
            body
        } else {
            results.pop().unwrap_or_else(|| json!({ "passed": false }))
        };
        Ok(to_call_tool_result(ToolResult::text(
            serde_json::to_string_pretty(&body).unwrap_or_default(),
        )))
    }

    async fn run_gate(
        &self,
        spec: &GateSpec,
        timeout_secs: u64,
        context: RequestContext<RoleServer>,
    ) -> Result<Value, ErrorData> {
        let reason = match self.record_via_ship_check(spec, context).await {
            Ok(recorded) => return Ok(recorded),
            Err(reason) => reason,
        };
        // A gate this server runs itself is live on the feed socket: every
        // chunk goes out as a `gate_output` frame keyed by the gate, the
        // frame the cockpit's engine feed folds into its terminal store. A
        // frame nobody is attached for is dropped, which is the library's
        // documented degraded mode, not an error.
        let feed = self.authority.feed().cloned();
        let gate = spec.name.clone();
        let stream = move |bytes: &[u8]| {
            if let Some(feed) = &feed {
                let output = GateOutput {
                    gate: gate.clone(),
                    bytes: bytes.to_vec(),
                };
                let _ = feed.emit(Family::Ship, &output.frame().payload);
            }
        };
        let mut result = run_gate_command(&spec.command, &self.project_root, timeout_secs, stream)
            .await
            .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;
        result["gate"] = json!(spec.name);
        result["required"] = json!(spec.required);
        result["recorded"] = json!(false);
        result["note"] = json!(unrecorded_note(&reason));
        Ok(result)
    }

    /// Forward one gate to `ship_check`. `Err` carries why the check could
    /// not be recorded — `ship_check`'s own refusal (typically "no active
    /// task"), an MCP error, or an answer that was not a check record — so
    /// the caller, which then runs the gate directly, can say so rather than
    /// guess.
    async fn record_via_ship_check(
        &self,
        spec: &GateSpec,
        context: RequestContext<RoleServer>,
    ) -> Result<Value, String> {
        // Asked *before* the command is handed over, because `ship_check`
        // runs it and only then discovers it has nowhere to put the result.
        // Forwarding a gate it is going to refuse therefore costs a whole
        // run — a `cargo nextest`, a clippy, a release build — that is
        // thrown away, and `run_gate` then runs the same command a second
        // time to answer the caller. Two runs of a timing or golden gate is
        // two independent chances to fail with only the second reported,
        // which is what made the gates look flaky. `ship_status` reads the
        // same in-process ship engine `ship_check` would, so the question
        // and the answer cannot disagree.
        if !self.has_active_task(context.clone()).await {
            return Err(NO_ACTIVE_TASK.to_owned());
        }
        let mut params = CallToolRequestParams::new("ship_check");
        let mut args = json!({
            "type": check_type_for(&spec.name),
            "name": spec.name,
            "command": spec.command,
            "required": spec.required,
        });
        // nextest can emit a junit report; ask ship_check to parse it into
        // the record (additive — a missing report never fails the check).
        if let Some(path) = nextest_junit_report_path(&spec.command) {
            args["report"] = json!({"format": "junit", "path": path});
        }
        params.arguments = args.as_object().cloned();
        let result = match self.unified.call_tool(params, context).await {
            Ok(CallToolResponse::Complete(result)) => result,
            Ok(_) => return Err("ship_check answered with a task, not a check record".to_owned()),
            Err(e) => return Err(format!("ship_check failed: {}", e.message)),
        };
        let record = check_record(&result)?;
        let output = record
            .get("details")
            .and_then(Value::as_str)
            .unwrap_or_default();
        // Rows from the captured output, plus the junit failures ship_check
        // parsed for us (the report names tests the panic scan may not).
        let mut rows = magistr_core::parse_gate_rows(&spec.command, output);
        if let Some(failures) = record["results"]["failures"].as_array() {
            for f in failures {
                let name = f["name"].as_str().unwrap_or_default();
                if name.is_empty() || rows.iter().any(|r| r.message.contains(name)) {
                    continue;
                }
                let message = match f["message"].as_str().filter(|m| !m.is_empty()) {
                    Some(m) => format!("{name}: {}", m.lines().next().unwrap_or_default()),
                    None => name.to_owned(),
                };
                rows.push(magistr_core::GateFailureRow {
                    severity: magistr_core::GateRowSeverity::TestFailure,
                    code: None,
                    message,
                    file: None,
                    line: None,
                    column: None,
                });
            }
        }
        Ok(json!({
            "gate": spec.name,
            "required": spec.required,
            "command": spec.command,
            "passed": record.get("passed").cloned().unwrap_or(Value::Bool(false)),
            "exit_code": record.get("exit_code").cloned().unwrap_or(Value::Null),
            "output": output,
            "rows": rows,
            "recorded": true,
            "verified": record.get("verified").cloned().unwrap_or(Value::Bool(true)),
        }))
    }

    /// The plan guard on `magistr_set_phase`: why `phase` may not be entered
    /// given this server's ship state, or `None` when it may. Reads
    /// `ship_status` through the unified service — the same document the
    /// agent sees — so the guard and the agent never disagree on the state.
    async fn ship_phase_block(
        &self,
        phase: &str,
        context: RequestContext<RoleServer>,
    ) -> Option<String> {
        let status = self.ship_status(context).await?;
        ShipGuardState::from_status(&status).phase_block(phase)
    }

    /// This server's `ship_status` document, or `None` when the call failed
    /// or answered something that is not a status.
    ///
    /// One read for both consumers — the phase guard and the gate's
    /// can-this-be-recorded question — so neither can be looking at a
    /// different ship state than the agent is.
    async fn ship_status(&self, context: RequestContext<RoleServer>) -> Option<Value> {
        let params = CallToolRequestParams::new("ship_status");
        let CallToolResponse::Complete(result) =
            self.unified.call_tool(params, context).await.ok()?
        else {
            return None;
        };
        result
            .structured_content
            .clone()
            .or_else(|| first_text(&result).and_then(|t| serde_json::from_str(&t).ok()))
    }

    /// Whether a task is started in this server's ship engine — the one
    /// condition that decides whether `ship_check` has anywhere to record a
    /// gate. A status this server cannot read answers `false`, so an
    /// unreadable state costs a gate its trace entry and never a second run
    /// of the command.
    async fn has_active_task(&self, context: RequestContext<RoleServer>) -> bool {
        self.ship_status(context)
            .await
            .is_some_and(|status| !status["active_task"].is_null())
    }

    /// `magistr_set_phase` side effect: a `ship_record` action on the active
    /// task. Best effort — without an active task there is nothing to record.
    async fn record_phase(&self, phase: &str, context: RequestContext<RoleServer>) {
        let mut params = CallToolRequestParams::new("ship_record");
        params.arguments = json!({
            "type": "config",
            "description": format!("workflow phase → {phase}"),
            "tools_used": ["magistr_set_phase"],
        })
        .as_object()
        .cloned();
        let _ = self.unified.call_tool(params, context).await;
    }
}

/// Why a gate was not forwarded to `ship_check`: there is nowhere to record
/// it. Word for word `ship_check`'s own refusal, because it is the same
/// condition — this server just asks *before* paying for a run of the
/// command rather than after (see [`MagistrService::record_via_ship_check`]),
/// so the note a caller reads is the one it always read.
const NO_ACTIVE_TASK: &str = "no task_id provided and no active task";

/// What `ship_check` answered, read without losing the why: the check
/// record when there is one, otherwise the reason there is not.
///
/// think-and-ship reports a refused call as a soft error envelope
/// (`ok: false`, `error_kind`, `message`) with `is_error: false` — "no
/// active task" arrives this way — so the envelope's message is the reason.
/// An MCP-level error (`is_error: true`) contributes its text. Anything
/// else without an `exit_code` is not a command-run check, whatever it is.
fn check_record(result: &CallToolResult) -> Result<Value, String> {
    if result.is_error == Some(true) {
        let text = first_text(result).unwrap_or_else(|| "no message".to_owned());
        return Err(format!("ship_check errored: {text}"));
    }
    let Some(record) = result
        .structured_content
        .clone()
        .or_else(|| first_text(result).and_then(|t| serde_json::from_str(&t).ok()))
    else {
        return Err("ship_check answered no check record".to_owned());
    };
    if record.get("ok") == Some(&Value::Bool(false)) {
        let kind = record["error_kind"].as_str().unwrap_or("refused");
        return Err(match record["message"].as_str().filter(|m| !m.is_empty()) {
            Some(message) => message.to_owned(),
            None => format!("ship_check refused ({kind})"),
        });
    }
    if record.get("exit_code").is_none() {
        return Err("ship_check answered no check record".to_owned());
    }
    Ok(record)
}

/// The note on a gate that ran but was not recorded: the reason in
/// `ship_check`'s own words, and the remedy. The remedy names *this*
/// server because a task started through another process's engine is
/// invisible here until that process saves (tas-ship-state-two-servers).
fn unrecorded_note(reason: &str) -> String {
    format!(
        "not recorded: {reason} (a check needs an active ship task in this server: \
         ship_set_objective → ship_plan → ship_start, then re-run the gate)"
    )
}

/// The top-level note on a `magistr_gate(all)` run with unrecorded gates:
/// every gate that was not recorded, grouped by reason, so a green run that
/// left no trace is visible without reading each gate. `None` when every
/// gate was recorded.
fn unrecorded_all_note(unrecorded: &[&Value]) -> Option<String> {
    if unrecorded.is_empty() {
        return None;
    }
    // Group by the gate's note, preserving first-seen order.
    let mut groups: Vec<(&str, Vec<&str>)> = Vec::new();
    for gate in unrecorded {
        let note = gate["note"].as_str().unwrap_or("not recorded");
        let name = gate["gate"].as_str().unwrap_or("?");
        match groups.iter_mut().find(|(n, _)| *n == note) {
            Some((_, names)) => names.push(name),
            None => groups.push((note, vec![name])),
        }
    }
    Some(
        groups
            .iter()
            .map(|(note, names)| format!("{}: {note}", names.join(", ")))
            .collect::<Vec<_>>()
            .join("; "),
    )
}

/// Map a gate spec name to a `ship_check` type. Polyglot specs are named
/// `{gate}:{archetype id}`, so the match is on the prefix before the colon.
fn check_type_for(gate: &str) -> &'static str {
    match gate.split(':').next().unwrap_or(gate) {
        "check" => "typecheck",
        "test" => "test",
        _ => "lint",
    }
}

fn first_text(result: &CallToolResult) -> Option<String> {
    result
        .content
        .iter()
        .find_map(|c| c.as_text().map(|t| t.text.clone()))
}

fn to_call_tool_result(result: ToolResult) -> CallToolResult {
    let content = result
        .content
        .into_iter()
        .map(|c| match c {
            ToolContent::Text { text } => ContentBlock::text(text),
            ToolContent::Image { data, mime_type } => ContentBlock::image(data, mime_type),
        })
        .collect();
    if result.is_error == Some(true) {
        CallToolResult::error(content)
    } else {
        CallToolResult::success(content)
    }
}

/// The protocol eras this server will negotiate or advertise — everything
/// below 2026-07-28, whose SEP-2322 result shapes the Claude Agent SDK's
/// bundled CLI negotiates and then rejects (see [`super::protocol_cap`]).
/// Clients learn the ceiling three ways — the `initialize` response, the
/// `server/discover` listing, and the per-request version check — and all
/// three read from here or from [`ServerHandler::get_info`].
const SERVED_PROTOCOL_VERSIONS: &[rmcp::model::ProtocolVersion] = &[
    rmcp::model::ProtocolVersion::V_2024_11_05,
    rmcp::model::ProtocolVersion::V_2025_03_26,
    rmcp::model::ProtocolVersion::V_2025_06_18,
];

impl ServerHandler for MagistrService {
    fn get_info(&self) -> ServerInfo {
        let mut info = self.unified.get_info();
        info.server_info = Implementation::new("magistr", env!("CARGO_PKG_VERSION"));
        // The version a client that requested an unknown era falls back to,
        // and the newest this server owns up to anywhere.
        info.protocol_version = rmcp::model::ProtocolVersion::V_2025_06_18;
        info.instructions = Some(match info.instructions.take() {
            Some(tas) => format!("{INSTRUCTIONS}\n\n{tas}"),
            None => INSTRUCTIONS.to_owned(),
        });
        info
    }

    fn supported_protocol_versions(
        &self,
    ) -> std::borrow::Cow<'static, [rmcp::model::ProtocolVersion]> {
        std::borrow::Cow::Borrowed(SERVED_PROTOCOL_VERSIONS)
    }

    async fn list_tools(
        &self,
        request: Option<PaginatedRequestParams>,
        context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        let mut tools = self.magistr_tools();
        tools.extend(self.unified.list_tools(request, context).await?.tools);
        if let Some(tool) = tools.iter_mut().find(|tool| tool.name == "roadmap_export") {
            *tool = RoadmapExport::tool();
        }
        Ok(ListToolsResult::with_all_items(tools))
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, ErrorData> {
        if request.name.starts_with("magistr_") {
            return self.call_magistr(request, context).await.map(Into::into);
        }
        if request.name == "roadmap_export" {
            return self
                .roadmap_export
                .call(&self.unified, request, context)
                .await;
        }
        self.unified.call_tool(request, context).await
    }

    async fn list_resources(
        &self,
        request: Option<PaginatedRequestParams>,
        context: RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult, ErrorData> {
        self.unified.list_resources(request, context).await
    }

    async fn list_resource_templates(
        &self,
        request: Option<PaginatedRequestParams>,
        context: RequestContext<RoleServer>,
    ) -> Result<ListResourceTemplatesResult, ErrorData> {
        self.unified.list_resource_templates(request, context).await
    }

    async fn read_resource(
        &self,
        request: ReadResourceRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResponse, ErrorData> {
        self.unified.read_resource(request, context).await
    }

    async fn get_task(
        &self,
        request: GetTaskParams,
        context: RequestContext<RoleServer>,
    ) -> Result<GetTaskResult, ErrorData> {
        self.unified.get_task(request, context).await
    }

    async fn update_task(
        &self,
        request: UpdateTaskParams,
        context: RequestContext<RoleServer>,
    ) -> Result<(), ErrorData> {
        self.unified.update_task(request, context).await
    }

    async fn cancel_task(
        &self,
        request: CancelTaskParams,
        context: RequestContext<RoleServer>,
    ) -> Result<(), ErrorData> {
        self.unified.cancel_task(request, context).await
    }
}

#[cfg(test)]
#[path = "service.tests.rs"]
mod tests;
