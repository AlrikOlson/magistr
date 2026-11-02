use super::{
    Observed, ReadSource, RequestEngine, RequestFrame, RequestMutationError, Requests,
    editing::Edit, validate,
};
use crate::infra::transaction::{TransactionTarget, transact};

impl RequestEngine {
    pub(crate) fn mutate<R>(
        &mut self,
        apply: impl FnOnce(&mut Edit<'_>) -> Result<R, String>,
    ) -> Result<Observed<R>, RequestMutationError> {
        let path = self
            .persistence
            .as_ref()
            .filter(|p| p.enabled())
            .map(|p| p.path_for(&self.project_id));
        let lock_path = path.as_ref().map(|path| path.with_extension("json.lock"));
        let target = match (&path, &lock_path) {
            (Some(path), Some(lock_path)) => TransactionTarget::File { path, lock_path },
            _ => TransactionTarget::Memory(&self.requests),
        };
        let result = transact(
            target,
            || Requests {
                schema_version: magistr_workflow_contracts::REQUESTS_SCHEMA_VERSION,
                project_id: self.project_id.clone(),
                requests: Vec::new(),
            },
            |state| validate(state, &self.project_id),
            |state| {
                let mut edit = Edit {
                    requests: state,
                    project_id: self.project_id.clone(),
                    events: Vec::new(),
                };
                let value = apply(&mut edit)?;
                Ok((value, edit.events))
            },
        );
        match result {
            Ok(committed) => {
                self.requests = committed.state;
                self.stamp = match committed.source {
                    ReadSource::File(stamp) => Some(stamp),
                    _ => None,
                };
                self.publish_committed(committed.value.1);
                Ok(Observed {
                    value: committed.value.0,
                    source: committed.source,
                })
            }
            Err(error) => {
                if matches!(error, crate::infra::transaction::TransactionError::Store(_)) {
                    self.stamp = None;
                }
                Err(error.into())
            }
        }
    }

    fn publish_committed(&self, frames: Vec<RequestFrame>) {
        let captured: std::collections::HashSet<String> = frames
            .iter()
            .filter_map(|frame| match frame {
                RequestFrame::RequestCaptured { request } => Some(request.id.clone()),
                _ => None,
            })
            .collect();
        let mut seen = std::collections::HashSet::new();
        let mut committed = Vec::new();
        for frame in frames.into_iter().rev() {
            let old = match &frame {
                RequestFrame::RequestCaptured { request }
                | RequestFrame::RequestChanged { request }
                | RequestFrame::RequestDismissed { request } => request,
            };
            if !seen.insert(old.id.clone()) {
                continue;
            }
            let Some(request) = self
                .requests
                .requests
                .iter()
                .find(|request| request.id == old.id)
                .cloned()
            else {
                continue;
            };
            if captured.contains(&request.id) {
                committed.push(RequestFrame::RequestCaptured { request });
                continue;
            }
            committed.push(match frame {
                RequestFrame::RequestCaptured { .. } => RequestFrame::RequestCaptured { request },
                RequestFrame::RequestDismissed { .. } => RequestFrame::RequestDismissed { request },
                RequestFrame::RequestChanged { .. } => RequestFrame::RequestChanged { request },
            });
        }
        for frame in committed.into_iter().rev() {
            self.record_event(frame);
        }
    }
}
