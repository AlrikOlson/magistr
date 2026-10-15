//! MCP service for Magistr and native workflow tools.
//!
//! [`super::build_server`] supplies the `magistr_*` handlers alongside the
//! think-and-ship tool families served by [`UnifiedService`]. Other tools,
//! resources, and task methods delegate with the original request context.
//!
//! `magistr_gate` runs every gate on Magistr's own pty runner, streaming its
//! output to the feed, and records the run through the composition
//! boundary's [`AdmittedCheck`] — the same admission and settlement the loop
//! driver uses for the gates it runs — on the task named as active at the
//! time. Without a task the gate still runs and the answer says
//! `recorded: false` and why.
//!
//! `magistr_set_phase` validates ship state before transitioning, announces
//! the phase typed on the feed's `magistr` family and records an action
//! afterward. Execute and verify require an objective and task; done
//! requires a recorded check.
//!
//! Native engines open only through an admitted [`ProjectAuthority`] in the
//! service role: identity, the cooperative owner lease and the live feed are
//! settled before any engine opens, and a competing MCP service fails there.
//! The transport's own mutations act through the same [`NativeEngines`] its
//! [`UnifiedService`] serves — one handler per verb, whichever side asks —
//! and never through its own wire. The cockpit or loop host holds the other
//! role beside this one. External binaries that bypass the lease still share
//! the underlying persistence.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use magistr_engine::native::{
    ActionType, Check, CheckSettlement, NativeConfig, NativeEngines, ProjectAuthority, WriterRole,
};

use crate::application::AdmittedCheck;
use magistr_engine::{GateOutput, MagistrFrame, ShipGuardState, WorkflowPhase};
use magistr_mcp::{SdkMcpServer, ToolContent, ToolResult};
use magistr_workflow::infra::Family;
use magistr_workflow::mcp::UnifiedService;
use rmcp::model::{
    CallToolRequestParams, CallToolResponse, CallToolResult, CancelTaskParams, ContentBlock,
    GetTaskParams, GetTaskResult, Implementation, JsonObject, ListResourceTemplatesResult,
    ListResourcesResult, ListToolsResult, PaginatedRequestParams, ReadResourceRequestParams,
    ReadResourceResponse, ServerInfo, Tool, UpdateTaskParams,
};
use rmcp::service::{RequestContext, RoleServer};
use rmcp::{ErrorData, ServerHandler};
use serde_json::{Value, json};

use super::gate::{GateSpec, gate_json, resolve_gates};
use super::roadmap_export::RoadmapExport;

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
    /// The admitted writer over the very engines `unified` serves: what this
    /// server's own mutations — a gate it ran, a phase it recorded — go
    /// through, so they are the same handler on the same engines as a tool
    /// call from the agent. Cloning a service shares it; the lease releases
    /// with the last clone.
    pub(super) engines: NativeEngines,
    roadmap_export: RoadmapExport,
    context: Arc<super::view_context::ContextState>,
    /// The admission this service runs under: the project identity, the
    /// service-role lease, and the live-feed socket the engines broadcast on
    /// (kept so a gate this server runs itself can stream its bytes there
    /// too). The owner endpoint (`owner_attach`) is keyed by it.
    pub(super) authority: Arc<ProjectAuthority>,
}

impl MagistrService {
    /// Open one owner on an explicit configuration. Clones share the same engines
    /// and lease. Independent instances must acquire ownership before loading state.
    /// A configuration without the project's evidence capability is refused
    /// before the lease is taken: every check this service records must be
    /// bound to the source and the external configuration it certifies, and
    /// only a configuration composed through `writer_config` (over the
    /// captured project inputs) knows which external layers apply.
    pub async fn new(project_root: PathBuf, tas: &NativeConfig) -> std::io::Result<Self> {
        let project_root = project_root.canonicalize()?;
        let tas = tas.clone().with_project_root(project_root.clone());
        if tas.evidence().is_none() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "the writer configuration carries no verification evidence capability; compose it with magistr_core::evidence::writer_config so every recorded check is bound to the source and configuration it certifies",
            ));
        }
        let mut authority = ProjectAuthority::admit(&tas, WriterRole::Service)?;
        if let Some(path) = tas.broadcast_path() {
            authority = authority.publish(path).await?;
        }
        let (_, engines) = authority.serve();
        Ok(Self::from_engines(engines))
    }

    pub(crate) fn from_engines(engines: NativeEngines) -> Self {
        let project_root = engines.config().project_root().to_path_buf();
        let unified = ProjectAuthority::mcp_service(&engines);
        Self {
            roadmap_export: RoadmapExport::new(&project_root),
            context: Arc::default(),
            tools: Arc::new(super::build_server(project_root.clone())),
            project_root,
            unified,
            authority: engines.shared_authority(),
            engines,
        }
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
    ) -> Result<CallToolResult, ErrorData> {
        let name = request.name.to_string();
        let args = Value::Object(request.arguments.clone().unwrap_or_default());
        match name.as_str() {
            "magistr_gate" => self.gate(&args).await,
            "magistr_context" => self.view_context(&args),
            "magistr_set_phase" => {
                let Some(phase) = args.get("phase").and_then(Value::as_str) else {
                    return self.invoke(&name, args).await;
                };
                let ship = self
                    .engines
                    .ship_snapshot()
                    .map_err(|error| ErrorData::internal_error(error, None))?;
                if let Some(reason) =
                    ShipGuardState::from_ship(ship.objective(), ship.tasks()).phase_block(phase)
                {
                    return Ok(to_call_tool_result(ToolResult::error(reason)));
                }
                let result = self.invoke(&name, args.clone()).await?;
                if result.is_error != Some(true)
                    && let Some(phase) = WorkflowPhase::from_name(phase)
                {
                    self.record_phase(phase, &ship);
                }
                Ok(result)
            }
            _ => self.invoke(&name, args).await,
        }
    }

    /// A narrow owner query; no command or workflow mutation is performed.
    fn view_context(&self, args: &Value) -> Result<CallToolResult, ErrorData> {
        super::view_context::validate_args(args).map_err(|e| ErrorData::invalid_params(e, None))?;
        let config = crate::validate::load_project_config(&self.project_root)
            .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;
        let choices = super::view_context::check_choices(&config)
            .map_err(|e| ErrorData::internal_error(e, None))?;
        let ship = self
            .engines
            .ship_snapshot()
            .map_err(|error| ErrorData::internal_error(error, None))?;
        let cycle = ship.objective().and_then(|o| o.created_at.clone());
        let task = ship.active_task().map(|task| task.id.clone());
        let detail = args
            .get("task_id")
            .and_then(Value::as_str)
            .map(|id| {
                ship.tasks()
                    .iter()
                    .find(|t| t.id == id)
                    .cloned()
                    .ok_or_else(|| {
                        ErrorData::invalid_params("That task is no longer available.", None)
                    })
                    .map(super::view_context::bounded_task)
            })
            .transpose()?;
        let receipt = args
            .get("receipt_id")
            .and_then(Value::as_str)
            .map(|id| {
                self.engines
                    .ship_receipt(id)
                    .map_err(|e| ErrorData::internal_error(e, None))
            })
            .transpose()?
            .flatten();
        let body = json!({
            "version": 1,
            "task": detail,
            "receipt": receipt,
            "project_id": self.authority.project_id(),
            "project_root": self.project_root.to_string_lossy(),
            "owner_epoch": self.authority.generation().get().to_string(),
            "phase": self.context.phase(&cycle, &task),
            "phase_task": task,
            "phase_cycle": cycle,
            "checks": choices
        });
        let mut result = CallToolResult::success(vec![ContentBlock::text(body.to_string())]);
        result.structured_content = Some(body);
        Ok(result)
    }

    /// `magistr_gate`: resolve the gate set from project.json, run each gate
    /// on the pty runner and record it on the active task when there is one.
    async fn gate(&self, args: &Value) -> Result<CallToolResult, ErrorData> {
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
            results.push(self.run_gate(spec, timeout_secs).await);
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

    /// One gate: run on the pty runner, then recorded — on the task that is
    /// active as the run starts, named explicitly — through the same
    /// `ship_record_gate` the loop driver records its gates with. The
    /// command runs exactly once whether or not there is a task to record
    /// it on; without one the answer says so and why.
    async fn run_gate(&self, spec: &GateSpec, timeout_secs: u64) -> Value {
        // Named *before* the run, so the record lands where the caller
        // stood when it asked — never on a task started while the gate ran
        // — and the receipt observes the source as it is at this moment.
        let admission = AdmittedCheck::admit(
            &self.engines,
            &self.project_root,
            None,
            &spec.name,
            spec.command.clone(),
            spec.required,
        );
        // A gate this server runs is live on the feed socket: every chunk
        // goes out on the host's own family as a typed gate-output frame
        // keyed by the gate, the event the cockpit's engine feed folds into
        // its terminal store. A frame nobody is attached for is dropped,
        // which is the library's documented degraded mode, not an error.
        let feed = self
            .authority
            .feed()
            .cloned()
            .map(|feed| feed.with_execution(self.engines.config().execution_context().key()));
        let gate = spec.name.clone();
        let stream = move |bytes: &[u8]| {
            if let Some(feed) = &feed {
                let output = GateOutput {
                    gate: gate.clone(),
                    bytes: bytes.to_vec(),
                };
                let _ = feed.emit(Family::Magistr, &MagistrFrame::GateOutput(output));
            }
        };
        let run = crate::utils::gate::run_gate(
            &spec.name,
            &spec.command,
            spec.required,
            &self.project_root,
            timeout_secs,
            &tokio_util::sync::CancellationToken::new(),
            stream,
        )
        .await;
        let mut result = gate_json(&run);
        result["gate"] = json!(spec.name);
        result["required"] = json!(spec.required);

        let admission = match admission {
            Ok(admission) => admission,
            Err(why) => {
                result["recorded"] = json!(false);
                result["note"] = json!(unrecorded_note(&why.to_string()));
                result["failure"] = json!(why.mutation_failure());
                return result;
            }
        };
        let task = admission.target().task_id.clone();
        // A nextest junit report is parsed into the record additively by
        // the boundary (a missing report never fails the check).
        match admission
            .settle(&self.engines, run.execution(), run.details())
            .map(|settled| settled.settlement)
        {
            Ok(CheckSettlement::Applied { check, receipt }) => {
                // Rows from the captured output, plus the junit failures the
                // record parsed (the report names tests the panic scan may
                // not).
                let mut rows = run.rows.clone();
                junit_rows(&check.value, &mut rows);
                result["rows"] = json!(rows);
                result["recorded"] = json!(true);
                result["verified"] = json!(check.value.verified);
                result["saved_check"] = json!(check);
                result["saved_receipt"] = json!(receipt);
                result["task_id"] = json!(task);
                result["receipt"] = json!(receipt.value.id);
                result["source_revision"] =
                    json!(receipt.value.source.as_ref().map(|s| &s.revision));
                result["source_freshness"] = json!(receipt.value.freshness);
            }
            Ok(CheckSettlement::Stale {
                admission,
                outcome,
                receipt,
            }) => {
                result["recorded"] = json!(false);
                result["note"] = json!(unrecorded_note(
                    "the admitted workflow run or task was superseded"
                ));
                result["stale_admission"] = json!(admission);
                result["unsettled_outcome"] = json!(outcome);
                result["saved_receipt"] = json!(receipt);
                result["receipt"] = json!(receipt.value.id);
            }
            Err(why) => {
                result["recorded"] = json!(false);
                result["note"] = json!(unrecorded_note(&why.to_string()));
                result["failure"] = json!(why.mutation_failure());
            }
        }
        result
    }

    /// `magistr_set_phase` side effects: the phase announced typed on the
    /// feed's `magistr` family — what the cockpit reads the phase from —
    /// and, when a task is active, a `ship_record` action on it naming the
    /// transition as trace narrative. Both are best effort: a feed nobody
    /// listens to drops the frame, and without an active task there is
    /// nothing to record on.
    fn record_phase(&self, phase: WorkflowPhase, ship: &magistr_engine::ShipData) {
        self.context.observe(
            ship.objective().and_then(|o| o.created_at.clone()),
            ship.active_task().map(|task| task.id.clone()),
            phase,
        );
        if let Some(feed) = self.authority.feed() {
            let _ = feed
                .clone()
                .with_execution(self.engines.config().execution_context().key())
                .emit(Family::Magistr, &MagistrFrame::PhaseSet { phase });
        }
        let Some(task) = ship.active_task() else {
            return;
        };
        let _ = self.engines.ship_record_action(
            Some(&task.id),
            ActionType::Config,
            format!("workflow phase → {}", phase.label()),
            Vec::new(),
            vec!["magistr_set_phase".to_owned()],
            None,
        );
    }
}

/// The junit failures a recorded check parsed, as rows beside the ones the
/// output scan found — each test once, whichever scan named it first.
fn junit_rows(check: &Check, rows: &mut Vec<magistr_core::GateFailureRow>) {
    let Some(results) = check.results.as_ref() else {
        return;
    };
    for failure in &results.failures {
        let name = failure.name.as_str();
        if name.is_empty() || rows.iter().any(|row| row.message.contains(name)) {
            continue;
        }
        let message = match failure.message.as_deref().filter(|m| !m.is_empty()) {
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
        if let Some(tools) = info.capabilities.tools.as_mut() {
            tools.list_changed = Some(true);
        }
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
        tools.push(super::view_context::tool());
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
            return self.call_magistr(request).await.map(Into::into);
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
