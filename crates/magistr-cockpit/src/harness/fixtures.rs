//! Typed store records for scenes and tests, and the wire line they ride.
//!
//! A scene or a test needs a task, a signal, a step, a goal,
//! a progress record or a check without saying every field the store
//! records; these constructors say the rest. [`frame`] is the wire-shaped
//! way in — a family, a kind and the payload a socket line would carry —
//! for scripts that read best as what the engines announce: it fills the
//! fields a scripted record leaves out with the typed default and then
//! decodes through the one edge decoder, so a scene is fed exactly what a
//! live feed would deliver. The leniency is the fixture's, not the
//! wire's: a live line missing a field is dropped by that same decoder.

use magistr_workflow_contracts::{
    Check, CheckType, Goal, GoalStatus, NativeFrame, Note, Progress, ProgressType, Signal,
    SignalKind, SignalStatus, Step, StepStatus, StepType, Task, TaskStatus, VerificationMode,
};
use magistr_workflow_contracts::{NativeEvent, NativeSnapshot};
use serde_json::{Value, json};

use crate::engine::EngineEvent;

/// A pending task with nothing but its id — no title, no name (the row
/// falls back to the id), priority zero, no deps.
#[must_use]
pub fn task(id: &str) -> Task {
    Task {
        id: id.to_owned(),
        title: String::new(),
        name: String::new(),
        status: TaskStatus::ToDo,
        priority: 0,
        description: String::new(),
        content: None,
        notes: String::new(),
        feature: None,
        acceptance: Vec::new(),
        deps: Vec::new(),
        tier: None,
        refs: Vec::new(),
        shared: false,
        reprioritize: None,
        status_proposal: None,
        title_proposal: None,
        retired_reason: None,
        waiting_on: None,
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
        refs: Vec::new(),
        surfaced_at: None,
        snooze_until: None,
        project_id: None,
    }
}

/// A main-line reasoning step with nothing but its number.
#[must_use]
pub fn step(number: u32) -> Note {
    Note {
        note: number,
        estimated_total: 0,
        purpose: String::new(),
        context: String::new(),
        thought: String::new(),
        outcome: String::new(),
        next_action: magistr_workflow_contracts::NextAction::default(),
        rationale: String::new(),
        confidence: None,
        uncertainty_notes: None,
        revises_note: None,
        revision_reason: None,
        revised_by: None,
        is_final_note: None,
        thread_from: None,
        thread_id: None,
        thread_name: None,
        tools_used: None,
        dependencies: None,
        timestamp: None,
        duration_ms: None,
        log_id: None,
        pinned: None,
        cwd: None,
        run_ref: None,
        record_id: None,
    }
}

/// An active goal with `description`, unscoped.
#[must_use]
pub fn goal(description: &str) -> Goal {
    Goal {
        description: description.to_owned(),
        acceptance_criteria: Vec::new(),
        constraints: Vec::new(),
        scope: String::new(),
        verification: VerificationMode::Full,
        status: GoalStatus::Active,
        project_id: String::new(),
        created_at: None,
        completed_at: None,
    }
}

/// A planned implement step, nothing recorded on it yet.
#[must_use]
pub fn run_step(id: &str, title: &str) -> Step {
    Step {
        id: id.to_owned(),
        title: title.to_owned(),
        step_type: StepType::Implement,
        status: StepStatus::Planned,
        estimate: None,
        started_at: None,
        completed_at: None,
        artifacts: Vec::new(),
        checks: Vec::new(),
        progress: Vec::new(),
        waiting_reason: None,
        decision_thread: None,
    }
}

/// A code progress numbered `id` with `description`.
#[must_use]
pub fn progress_record(id: u32, description: &str) -> Progress {
    Progress {
        id,
        step_id: String::new(),
        timestamp: String::new(),
        progress_type: ProgressType::Code,
        description: description.to_owned(),
        files_touched: Vec::new(),
        tools_used: Vec::new(),
        result: String::new(),
        note: None,
    }
}

/// A verified test check named `name` that `passed` (or did not), with
/// no exit code and no report.
#[must_use]
pub fn check(name: &str, passed: bool) -> Check {
    Check {
        check_result_id: None,
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

/// A snapshot holding `tasks` and `signals` and nothing else — what a
/// plan scene seeds from.
#[must_use]
pub fn snapshot(tasks: Vec<Task>, signals: Vec<Signal>) -> NativeSnapshot {
    use magistr_workflow_contracts::{Observed, PlanData, ReadSource, ReadState};
    NativeSnapshot {
        plan: ReadState::Current(Observed {
            value: PlanData {
                tasks,
                ..PlanData::default()
            },
            source: ReadSource::Memory,
        }),
        signals: ReadState::Current(Observed {
            value: signals,
            source: ReadSource::Memory,
        }),
        ..NativeSnapshot::default()
    }
}

/// The plan's progress as `done` of `total` tasks.
#[must_use]
pub fn progress(done: u32, total: u32) -> magistr_workflow_contracts::PlanProgress {
    magistr_workflow_contracts::PlanProgress {
        total_tasks: total,
        checked_tasks: done,
        ..magistr_workflow_contracts::PlanProgress::default()
    }
}

/// The tasks a list of partial records describes — [`task_from`] over
/// each.
#[must_use]
pub fn tasks_from(partials: Vec<Value>) -> Vec<Task> {
    partials.into_iter().map(task_from).collect()
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
pub fn steps_from(partials: Vec<Value>) -> Vec<Note> {
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
        // A script may name the step by its record where the wire names
        // it by id, and may leave the id off a progress record or a check.
        if let Some(id) = object.get("step").and_then(|step| step["id"].as_str()) {
            let id = id.to_owned();
            object.entry("step_id").or_insert(json!(id));
        }
        if matches!(kind, "progress_recorded" | "check_recorded") {
            object.entry("step_id").or_insert(json!(""));
        }
        if kind == "run_finished" {
            object.entry("warnings").or_insert(json!([]));
        }
        let ended = match kind {
            "task_completed" => Some("done"),
            "task_retired" => Some("retired"),
            _ => None,
        };
        fill_record(object, "task", |partial| {
            let mut default = task(id_of(partial, "id"));
            if let Some(status) = ended {
                default.status = serde_json::from_value(json!(status)).expect("a status");
            }
            fill_task(&default, partial)
        });
        fill_record(object, "signal", |partial| {
            fill_signal(&signal(id_of(partial, "id")), partial)
        });
        fill_record(object, "step", |partial| {
            let number = partial["note"].as_u64().unwrap_or(0);
            let number = u32::try_from(number).unwrap_or(u32::MAX);
            fill(&step(number), partial)
        });
        fill_record(object, "goal", |partial| fill(&goal(""), partial));
        fill_record(object, "progress", |partial| {
            let id = u32::try_from(partial["id"].as_u64().unwrap_or(0)).unwrap_or(u32::MAX);
            fill(&progress_record(id, ""), partial)
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

/// The task a partial record describes, the rest filled in from
/// [`task`].
#[must_use]
pub fn task_from(partial: Value) -> Task {
    let filled = fill_task(&task(id_of(&partial, "id")), &partial);
    serde_json::from_value(filled).expect("a filled task decodes")
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
pub fn step_from(partial: Value) -> Note {
    let number = u32::try_from(partial["note"].as_u64().unwrap_or(0)).unwrap_or(u32::MAX);
    serde_json::from_value(fill(&step(number), &partial)).expect("a filled step decodes")
}

/// The step a partial record describes, the rest filled in from [`run_step`];
/// its `checks` and `progress` are filled the same way.
#[must_use]
pub fn run_step_from(partial: Value) -> Step {
    let mut filled = fill(&run_step(id_of(&partial, "id"), ""), &partial);
    if let Some(checks) = filled["checks"].as_array_mut() {
        for check in checks.iter_mut() {
            *check = fill_check(check);
        }
    }
    if let Some(progress) = filled["progress"].as_array_mut() {
        for entry in progress.iter_mut() {
            let id = u32::try_from(entry["id"].as_u64().unwrap_or(0)).unwrap_or(u32::MAX);
            *entry = fill(&progress_record(id, ""), entry);
        }
    }
    serde_json::from_value(filled).expect("a filled step decodes")
}

/// The goal a partial record describes, the rest filled in from
/// [`goal`].
#[must_use]
pub fn goal_from(partial: Value) -> Goal {
    serde_json::from_value(fill(&goal(""), &partial)).expect("a filled goal decodes")
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

to_value_via_serde!(Task, Signal, Note, Goal, Step, Progress, Check);

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

fn fill_task(default: &Task, partial: &Value) -> Value {
    let mut value = fill(default, partial);
    // The state word the row spells as `stuck` is two fields on the
    // record: work in progress that carries a blocker.
    if value["status"] == json!("stuck") {
        value["status"] = json!("in_progress");
        if value.get("waiting_on").is_none_or(Value::is_null) {
            value["waiting_on"] = json!({"kind": "person", "reason": "needs a decision"});
        }
    }
    for (key, extra) in [
        ("reprioritize", json!({"proposed_at": ""})),
        ("status_proposal", json!({"proposed_at": "", "source": ""})),
        ("title_proposal", json!({"proposed_at": "", "source": ""})),
        ("waiting_on", json!({"waiting_since": ""})),
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
