use super::{RequestError, RequestUseCases};
use crate::plan::domain::TaskStatus;
use crate::plan::engine::PlanMutationError;
#[cfg(test)]
use crate::request::domain::Request;
use crate::request::domain::{RequestKind, RequestStatus};

pub use magistr_workflow_contracts::request::RequestPromotion;

impl RequestUseCases {
    /// Retry safely using the same work item and reciprocal links.
    pub fn promote(&self, id: &str) -> Result<RequestPromotion, RequestError> {
        let plan = self.plan.as_ref().ok_or(RequestError::NotWired)?;
        let _operation = self.promotion.lock().map_err(|_| RequestError::Poisoned)?;
        let request = self.get(id)?;
        if !matches!(
            request.status,
            RequestStatus::Researched | RequestStatus::Raised | RequestStatus::Promoted
        ) {
            return Err(RequestError::InvalidState(
                "request must be researched or raised before it can be promoted".into(),
            ));
        }
        let task_id = request
            .refs
            .iter()
            .find_map(|reference| reference.strip_prefix("task:"))
            .map(str::to_owned)
            .unwrap_or_else(|| task_id_for(id));
        let task = plan
            .lock()
            .map_err(|_| RequestError::Poisoned)?
            .mutate(|edit| {
                let created = !edit.plan().tasks.iter().any(|task| task.id == task_id);
                if created {
                    edit.add_task(
                        task_id.clone(),
                        promoted_title(request.kind, &request.body),
                        TaskStatus::Idea,
                        0,
                        format!(
                            "Promoted from request {}. Original: {}",
                            request.id, request.body
                        ),
                        Vec::new(),
                        Vec::new(),
                        false,
                    )?;
                }
                let task = edit.link_task(&task_id, &format!("request:{id}"))?.clone();
                Ok((created, task))
            })
            .map_err(|error| match error {
                PlanMutationError::Rejected(message) => RequestError::InvalidState(message),
                PlanMutationError::Store(error) => RequestError::Store(error),
            })?;
        let created = task.value.0;
        let task = task.map(|(_, task)| task);
        let request_result = self
            .engine
            .lock()
            .map_err(|_| RequestError::Poisoned)
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
                            return Err(
                                "The request is already linked to another work item.".into()
                            );
                        }
                        if current.status != RequestStatus::Promoted {
                            edit.promote(id)?;
                        }
                        edit.link(id, &format!("task:{task_id}")).cloned()
                    })
                    .map_err(RequestError::invalid_state)
            });
        let request = request_result.map_err(|cause| RequestError::PartialPromotion {
            task_id: task_id.clone(),
            committed_source: task.source,
            cause: Box::new(cause),
        })?;
        Ok(RequestPromotion {
            request_id: id.to_owned(),
            task_id,
            created,
            request,
            task,
        })
    }
}

fn task_id_for(request_id: &str) -> String {
    format!("request-{request_id}")
}

fn promoted_title(kind: RequestKind, body: &str) -> String {
    let kind = match kind {
        RequestKind::Question => "question",
        RequestKind::Suggestion => "suggestion",
        RequestKind::Concern => "concern",
        RequestKind::Bug => "bug",
        RequestKind::Feedback => "feedback",
    };
    let first_line = body.lines().next().unwrap_or("").trim();
    let snippet: String = first_line.chars().take(80).collect();
    if snippet.is_empty() {
        format!("[{kind}] (request)")
    } else {
        format!("[{kind}] {snippet}")
    }
}

#[cfg(test)]
#[path = "promotion.tests.rs"]
mod tests;
