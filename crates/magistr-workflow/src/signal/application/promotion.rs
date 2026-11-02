use super::{SignalError, SignalUseCases};
use crate::plan::domain::TaskStatus;
use crate::plan::engine::PlanMutationError;
#[cfg(test)]
use crate::signal::domain::Signal;
use crate::signal::domain::{SignalKind, SignalStatus};

pub use magistr_workflow_contracts::signal::SignalPromotion;

impl SignalUseCases {
    /// Retry safely using the same work item and reciprocal links.
    pub fn promote(&self, id: &str) -> Result<SignalPromotion, SignalError> {
        let plan = self.plan.as_ref().ok_or(SignalError::NotWired)?;
        let _operation = self.promotion.lock().map_err(|_| SignalError::Poisoned)?;
        let signal = self.get(id)?;
        if !matches!(
            signal.status,
            SignalStatus::Researched | SignalStatus::Surfaced | SignalStatus::Promoted
        ) {
            return Err(SignalError::InvalidState(
                "signal must be researched or surfaced before it can be promoted".into(),
            ));
        }
        let task_id = signal
            .refs
            .iter()
            .find_map(|reference| reference.strip_prefix("task:"))
            .map(str::to_owned)
            .unwrap_or_else(|| task_id_for(id));
        let task = plan
            .lock()
            .map_err(|_| SignalError::Poisoned)?
            .mutate(|edit| {
                let created = !edit.plan().tasks.iter().any(|task| task.id == task_id);
                if created {
                    edit.add_task(
                        task_id.clone(),
                        promoted_title(signal.kind, &signal.body),
                        TaskStatus::Idea,
                        0,
                        format!(
                            "Promoted from signal {}. Original: {}",
                            signal.id, signal.body
                        ),
                        Vec::new(),
                        Vec::new(),
                        false,
                    )?;
                }
                let task = edit.link_task(&task_id, &format!("signal:{id}"))?.clone();
                Ok((created, task))
            })
            .map_err(|error| match error {
                PlanMutationError::Rejected(message) => SignalError::InvalidState(message),
                PlanMutationError::Store(error) => SignalError::Store(error),
            })?;
        let created = task.value.0;
        let task = task.map(|(_, task)| task);
        let signal_result = self
            .engine
            .lock()
            .map_err(|_| SignalError::Poisoned)
            .and_then(|mut engine| {
                engine
                    .mutate(|edit| {
                        let current = edit.get(id)?;
                        if current
                            .refs
                            .iter()
                            .filter_map(|reference| reference.strip_prefix("task:"))
                            .any(|linked| linked != task_id)
                        {
                            return Err("The signal is already linked to another work item.".into());
                        }
                        if current.status != SignalStatus::Promoted {
                            edit.promote(id)?;
                        }
                        edit.link(id, &format!("task:{task_id}")).cloned()
                    })
                    .map_err(SignalError::invalid_state)
            });
        let signal = signal_result.map_err(|cause| SignalError::PartialPromotion {
            task_id: task_id.clone(),
            committed_source: task.source,
            cause: Box::new(cause),
        })?;
        Ok(SignalPromotion {
            signal_id: id.to_owned(),
            task_id,
            created,
            signal,
            task,
        })
    }
}

fn task_id_for(signal_id: &str) -> String {
    format!("signal-{signal_id}")
}

fn promoted_title(kind: SignalKind, body: &str) -> String {
    let kind = match kind {
        SignalKind::Question => "question",
        SignalKind::Idea => "idea",
        SignalKind::Concern => "concern",
        SignalKind::Bug => "bug",
        SignalKind::Feedback => "feedback",
    };
    let first_line = body.lines().next().unwrap_or("").trim();
    let snippet: String = first_line.chars().take(80).collect();
    if snippet.is_empty() {
        format!("[{kind}] (signal)")
    } else {
        format!("[{kind}] {snippet}")
    }
}

#[cfg(test)]
#[path = "promotion.tests.rs"]
mod tests;
