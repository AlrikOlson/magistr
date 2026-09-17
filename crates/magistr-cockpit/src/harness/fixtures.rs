//! Typed store records for scenes and tests, and the wire line they ride.
//!
//! A scene or a test needs a chunk, a signal, a step, a task, an
//! objective, an action or a check without saying every field the store
//! records; these constructors say the rest. [`frame`] is the wire-shaped
//! way in — a family, a kind and the payload a socket line would carry —
//! for scripts that read best as what the engines announce: it fills the
//! fields a scripted record leaves out with the typed default and then
//! decodes through the one edge decoder, so a scene is fed exactly what a
//! live feed would deliver. The leniency is the fixture's, not the
//! wire's: a live line missing a field is dropped by that same decoder.

use magistr_workflow_contracts::{
    Action, ActionType, Check, CheckType, Chunk, ChunkStatus, NativeFrame, Objective,
    ObjectiveStatus, Signal, SignalKind, SignalStatus, Task, TaskStatus, TaskType, ThinkStep,
    VerificationMode,
};
use magistr_workflow_contracts::{NativeEvent, NativeSnapshot};
use serde_json::{Value, json};

use crate::engine::EngineEvent;

/// A pending chunk with nothing but its id — no title, no name (the row
/// falls back to the id), priority zero, no deps.
#[must_use]
pub fn chunk(id: &str) -> Chunk {
    Chunk {
        id: id.to_owned(),
        title: String::new(),
        name: String::new(),
        status: ChunkStatus::Pending,
        priority: 0,
        description: String::new(),
        content: None,
        notes: String::new(),
        group: None,
        acceptance: Vec::new(),
        deps: Vec::new(),
        tier: None,
        cross_refs: Vec::new(),
        shared: false,
        reprioritize: None,
        status_proposal: None,
        title_proposal: None,
        obsoleted_reason: None,
        blocked_by: None,
        project_id: None,
        created_at: String::new(),
        updated_at: String::new(),
    }
}

/// A new idea signal with nothing but its id.
#[must_use]
pub fn signal(id: &str) -> Signal {
    Signal {
        id: id.to_owned(),
        kind: SignalKind::Idea,
        from: String::new(),
        body: String::new(),
        content: None,
        created: String::new(),
        status: SignalStatus::New,
        enrichment: Vec::new(),
        cross_refs: Vec::new(),
        surfaced_at: None,
        snooze_until: None,
        project_id: None,
    }
}

/// A main-line reasoning step with nothing but its number.
#[must_use]
pub fn step(number: u32) -> ThinkStep {
    ThinkStep {
        step_number: number,
        estimated_total: 0,
        purpose: String::new(),
        context: String::new(),
        thought: String::new(),
        outcome: String::new(),
        next_action: magistr_workflow_contracts::NextAction::default(),
        rationale: String::new(),
        confidence: None,
        uncertainty_notes: None,
        revises_step: None,
        revision_reason: None,
        revised_by: None,
        is_final_step: None,
        branch_from: None,
        branch_id: None,
        branch_name: None,
        tools_used: None,
        dependencies: None,
        timestamp: None,
        duration_ms: None,
        session_id: None,
        pinned: None,
        cwd: None,
        execution_ref: None,
        record_id: None,
    }
}

/// An active objective with `description`, unscoped.
#[must_use]
pub fn objective(description: &str) -> Objective {
    Objective {
        description: description.to_owned(),
        acceptance_criteria: Vec::new(),
        constraints: Vec::new(),
        scope: String::new(),
        verification: VerificationMode::Full,
        status: ObjectiveStatus::Active,
        project_id: String::new(),
        created_at: None,
        completed_at: None,
    }
}

/// A planned implement task, nothing recorded on it yet.
#[must_use]
pub fn task(id: &str, title: &str) -> Task {
    Task {
        id: id.to_owned(),
        title: title.to_owned(),
        task_type: TaskType::Implement,
        status: TaskStatus::Planned,
        estimate: None,
        started_at: None,
        completed_at: None,
        artifacts: Vec::new(),
        checks: Vec::new(),
        actions: Vec::new(),
        blocked_reason: None,
        think_branch: None,
    }
}

/// A code action numbered `id` with `description`.
#[must_use]
pub fn action(id: u32, description: &str) -> Action {
    Action {
        id,
        task_id: String::new(),
        timestamp: String::new(),
        action_type: ActionType::Code,
        description: description.to_owned(),
        files_touched: Vec::new(),
        tools_used: Vec::new(),
        result: String::new(),
        think_step: None,
    }
}

/// A verified test check named `name` that `passed` (or did not), with
/// no exit code and no report.
#[must_use]
pub fn check(name: &str, passed: bool) -> Check {
    Check {
        check_type: CheckType::Test,
        name: name.to_owned(),
        passed,
        details: String::new(),
        required: true,
        verified: true,
        command: None,
        exit_code: None,
        report: None,
        results: None,
        timestamp: String::new(),
    }
}

/// A snapshot holding `chunks` and `signals` and nothing else — what a
/// roadmap scene seeds from.
#[must_use]
pub fn snapshot(chunks: Vec<Chunk>, signals: Vec<Signal>) -> NativeSnapshot {
    NativeSnapshot {
        chunks,
        signals,
        ..NativeSnapshot::default()
    }
}

/// The roadmap's progress as `done` of `total` chunks.
#[must_use]
pub fn progress(done: u32, total: u32) -> magistr_workflow_contracts::RoadmapProgress {
    magistr_workflow_contracts::RoadmapProgress {
        total_tasks: total,
        checked_tasks: done,
        ..magistr_workflow_contracts::RoadmapProgress::default()
    }
}

/// The chunks a list of partial records describes — [`chunk_from`] over
/// each.
#[must_use]
pub fn chunks_from(partials: Vec<Value>) -> Vec<Chunk> {
    partials.into_iter().map(chunk_from).collect()
}

/// The signals a list of partial records describes — [`signal_from`]
/// over each.
#[must_use]
pub fn signals_from(partials: Vec<Value>) -> Vec<Signal> {
    partials.into_iter().map(signal_from).collect()
}

/// The steps a list of partial records describes — [`step_from`] over
/// each.
#[must_use]
pub fn steps_from(partials: Vec<Value>) -> Vec<ThinkStep> {
    partials.into_iter().map(step_from).collect()
}

/// The typed event a socket line of `family` and `kind` carrying
/// `payload` decodes to, the record it names filled out from the typed
/// default where the script left fields unsaid.
///
/// # Panics
/// When the filled line does not decode: the script named a family, a
/// kind or a shape the engines never emit, which is a scripting error.
#[must_use]
pub fn frame(family: &str, kind: &str, payload: Value) -> NativeEvent {
    let mut payload = payload;
    if let Some(object) = payload.as_object_mut() {
        object.insert("family".to_owned(), json!(family));
        object.insert("type".to_owned(), json!(kind));
        // A script may name the task by its record where the wire names
        // it by id, and may leave the id off an action or a check.
        if let Some(id) = object.get("task").and_then(|task| task["id"].as_str()) {
            let id = id.to_owned();
            object.entry("task_id").or_insert(json!(id));
        }
        if matches!(kind, "action_recorded" | "check_recorded") {
            object.entry("task_id").or_insert(json!(""));
        }
        if kind == "objective_shipped" {
            object.entry("warnings").or_insert(json!([]));
        }
        let ended = match kind {
            "chunk_completed" => Some("done"),
            "chunk_obsoleted" => Some("obsoleted"),
            _ => None,
        };
        fill_record(object, "chunk", |partial| {
            let mut default = chunk(id_of(partial, "id"));
            if let Some(status) = ended {
                default.status = serde_json::from_value(json!(status)).expect("a status");
            }
            fill_chunk(&default, partial)
        });
        fill_record(object, "signal", |partial| {
            fill_signal(&signal(id_of(partial, "id")), partial)
        });
        fill_record(object, "step", |partial| {
            let number = partial["step_number"].as_u64().unwrap_or(0);
            let number = u32::try_from(number).unwrap_or(u32::MAX);
            fill(&step(number), partial)
        });
        fill_record(object, "objective", |partial| fill(&objective(""), partial));
        fill_record(object, "action", |partial| {
            let id = u32::try_from(partial["id"].as_u64().unwrap_or(0)).unwrap_or(u32::MAX);
            fill(&action(id, ""), partial)
        });
        fill_record(object, "check", fill_check);
    }
    let line = serde_json::to_string(&payload).expect("a JSON payload serializes");
    let (event, _) = NativeFrame::parse_line(&line)
        .and_then(NativeFrame::decode)
        .unwrap_or_else(|| panic!("the fixture line does not decode: {line}"));
    event
}

/// [`frame`] as the inbox message a scene folds: this process's own
/// event, delivered locally.
#[must_use]
pub fn event(family: &str, kind: &str, payload: Value) -> EngineEvent {
    EngineEvent::local(frame(family, kind, payload))
}

/// The chunk a partial record describes, the rest filled in from
/// [`chunk`].
#[must_use]
pub fn chunk_from(partial: Value) -> Chunk {
    let filled = fill_chunk(&chunk(id_of(&partial, "id")), &partial);
    serde_json::from_value(filled).expect("a filled chunk decodes")
}

/// The signal a partial record describes, the rest filled in from
/// [`signal`].
#[must_use]
pub fn signal_from(partial: Value) -> Signal {
    let filled = fill_signal(&signal(id_of(&partial, "id")), &partial);
    serde_json::from_value(filled).expect("a filled signal decodes")
}

/// The step a partial record describes, the rest filled in from [`step`].
#[must_use]
pub fn step_from(partial: Value) -> ThinkStep {
    let number = u32::try_from(partial["step_number"].as_u64().unwrap_or(0)).unwrap_or(u32::MAX);
    serde_json::from_value(fill(&step(number), &partial)).expect("a filled step decodes")
}

/// The task a partial record describes, the rest filled in from [`task`];
/// its `checks` and `actions` are filled the same way.
#[must_use]
pub fn task_from(partial: Value) -> Task {
    let mut filled = fill(&task(id_of(&partial, "id"), ""), &partial);
    if let Some(checks) = filled["checks"].as_array_mut() {
        for check in checks.iter_mut() {
            *check = fill_check(check);
        }
    }
    if let Some(actions) = filled["actions"].as_array_mut() {
        for entry in actions.iter_mut() {
            let id = u32::try_from(entry["id"].as_u64().unwrap_or(0)).unwrap_or(u32::MAX);
            *entry = fill(&action(id, ""), entry);
        }
    }
    serde_json::from_value(filled).expect("a filled task decodes")
}

/// The objective a partial record describes, the rest filled in from
/// [`objective`].
#[must_use]
pub fn objective_from(partial: Value) -> Objective {
    serde_json::from_value(fill(&objective(""), &partial)).expect("a filled objective decodes")
}

/// The check a partial record describes, the rest filled in from
/// [`check`].
#[must_use]
pub fn check_from(partial: Value) -> Check {
    serde_json::from_value(fill_check(&partial)).expect("a filled check decodes")
}

/// A record as its serialized form — the shape the partial is laid over.
trait ToValue {
    fn to_value(&self) -> Value;
}

impl ToValue for Value {
    fn to_value(&self) -> Value {
        self.clone()
    }
}

macro_rules! to_value_via_serde {
    ($($record:ty),* $(,)?) => {
        $(impl ToValue for $record {
            fn to_value(&self) -> Value {
                serde_json::to_value(self).expect("a record serializes")
            }
        })*
    };
}

to_value_via_serde!(Chunk, Signal, ThinkStep, Objective, Task, Action, Check);

fn id_of<'a>(partial: &'a Value, key: &str) -> &'a str {
    partial[key].as_str().unwrap_or_default()
}

/// Fill the record under `key` in `object`, when the script gave one.
fn fill_record(
    object: &mut serde_json::Map<String, Value>,
    key: &str,
    filled: impl FnOnce(&Value) -> Value,
) {
    if let Some(partial) = object.get(key).filter(|v| v.is_object()) {
        let value = filled(partial);
        object.insert(key.to_owned(), value);
    }
}

/// `partial` over the serialized `default`: every field the script said
/// wins, every field it left out comes from the default.
fn fill(default: &impl ToValue, partial: &Value) -> Value {
    let mut value = default.to_value();
    if let (Some(base), Some(over)) = (value.as_object_mut(), partial.as_object()) {
        for (key, field) in over {
            base.insert(key.clone(), field.clone());
        }
    }
    value
}

fn fill_chunk(default: &Chunk, partial: &Value) -> Value {
    let mut value = fill(default, partial);
    // The state word the row spells as `stuck` is two fields on the
    // record: work in progress that carries a blocker.
    if value["status"] == json!("stuck") {
        value["status"] = json!("in_progress");
        if value.get("blocked_by").is_none_or(Value::is_null) {
            value["blocked_by"] = json!({"kind": "awaiting_human", "reason": "needs a decision"});
        }
    }
    for (key, extra) in [
        ("reprioritize", json!({"proposed_at": ""})),
        ("status_proposal", json!({"proposed_at": "", "source": ""})),
        ("title_proposal", json!({"proposed_at": "", "source": ""})),
        ("blocked_by", json!({"blocked_at": ""})),
    ] {
        if let Some(nested) = value.get_mut(key).filter(|v| v.is_object()) {
            let filled = fill(&extra, nested);
            *nested = filled;
        }
    }
    value
}

fn fill_signal(default: &Signal, partial: &Value) -> Value {
    let mut value = fill(default, partial);
    if let Some(trail) = value["enrichment"].as_array_mut() {
        for entry in trail.iter_mut() {
            let filled = fill(&json!({"summary": "", "confidence": 0.0, "at": ""}), entry);
            *entry = filled;
        }
    }
    if let Some(content) = value.get_mut("content").filter(|v| v.is_object()) {
        let filled = fill(&json!({"version": 1, "summary": ""}), content);
        *content = filled;
    }
    value
}

fn fill_check(partial: &Value) -> Value {
    let name = partial["name"].as_str().unwrap_or("check");
    let passed = partial["passed"].as_bool().unwrap_or(false);
    let mut value = fill(&check(name, passed), partial);
    // A script may put the runner's duration on the check itself; the
    // record keeps it on the parsed report.
    if let Some(duration) = value
        .as_object_mut()
        .and_then(|object| object.remove("duration_ms"))
    {
        if value.get("results").is_none_or(Value::is_null) {
            value["results"] = json!({});
        }
        value["results"]["duration_ms"] = duration;
    }
    if let Some(results) = value.get_mut("results").filter(|v| v.is_object()) {
        let filled = fill(
            &json!({"total": 0, "passed": 0, "failed": 0, "skipped": 0}),
            results,
        );
        *results = filled;
    }
    value
}
