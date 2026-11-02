//! The service's read-only phase observation and canonical check choices.
//! These values change together as the admitted service or active step changes.

use std::sync::Mutex;

use magistr_core::types::ProjectConfig;
use magistr_engine::WorkflowPhase;
use rmcp::model::Tool;
use serde_json::{Value, json};

#[derive(Default)]
pub(super) struct ContextState {
    observation: Mutex<Option<Observation>>,
}

struct Observation {
    cycle: Option<String>,
    step: Option<String>,
    phase: WorkflowPhase,
}

impl ContextState {
    pub(super) fn observe(
        &self,
        cycle: Option<String>,
        step: Option<String>,
        phase: WorkflowPhase,
    ) {
        if let Ok(mut observed) = self.observation.lock() {
            *observed = Some(Observation { cycle, step, phase });
        }
    }

    pub(super) fn phase(
        &self,
        cycle: &Option<String>,
        step: &Option<String>,
    ) -> Option<WorkflowPhase> {
        self.observation
            .lock()
            .ok()?
            .as_ref()
            .and_then(|o| (&o.cycle == cycle && &o.step == step).then_some(o.phase))
    }
}

/// Validate even when an MCP client bypasses the advertised schema.
pub(super) fn validate_args(args: &Value) -> Result<(), String> {
    let object = args.as_object().ok_or("Expected query options.")?;
    for (key, value) in object {
        if !matches!(key.as_str(), "step_id" | "check_result_id")
            || !value
                .as_str()
                .is_some_and(|s| !s.is_empty() && s.chars().count() <= 256)
        {
            return Err("Choose a step or a check result from the current records.".to_owned());
        }
    }
    Ok(())
}

pub(super) fn tool() -> Tool {
    Tool::new(
        "magistr_context",
        "Read this connected owner's explicitly announced phase and configured checks. A missing phase is unknown, not inferred. This query runs no commands.",
        std::sync::Arc::new(
            json!({
                "type": "object", "properties": {
                    "step_id": {"type": "string", "maxLength": 256},
                    "check_result_id": {"type": "string", "maxLength": 256}
                }, "additionalProperties": false
            })
            .as_object()
            .expect("object schema")
            .clone(),
        ),
    )
}

pub(super) fn check_choices(config: &ProjectConfig) -> Result<Vec<Value>, String> {
    let mut names = vec!["check".to_owned(), "test".to_owned(), "lint".to_owned()];
    names.extend(config.custom_gates.iter().map(|gate| gate.name.clone()));
    names.sort();
    names.dedup();
    names
        .into_iter()
        .filter(|key| key != "all")
        .map(|key| {
            let specs = super::gate::resolve_gates(config, &key)?;
            Ok(json!({
                "key": key,
                "commands": specs.iter().filter(|s| !s.command.trim().is_empty())
                    .map(|s| json!({"name": s.name, "command": s.command, "required": s.required}))
                    .collect::<Vec<_>>()
            }))
        })
        .collect()
}

/// Serialize the canonical record with explicit evidence display bounds.
pub(super) fn bounded_step(mut step: magistr_engine::native::Step) -> Value {
    let omitted_checks = step.checks.len().saturating_sub(80);
    let omitted_actions = step.progress.len().saturating_sub(80);
    step.checks.drain(..omitted_checks);
    step.progress.drain(..omitted_actions);
    for check in &mut step.checks {
        check.details = check.details.chars().take(6000).collect();
    }
    json!({"record": step, "omitted_checks": omitted_checks, "omitted_progress": omitted_actions,
        "detail_limit": 6000})
}

#[cfg(test)]
#[path = "view_context.tests.rs"]
mod tests;
