use super::{
    Observed, ReadSource, SignalEngine, SignalFrame, SignalMutationError, Signals, editing::Edit,
    validate,
};
use crate::infra::transaction::{TransactionTarget, transact};

impl SignalEngine {
    pub(crate) fn mutate<R>(
        &mut self,
        apply: impl FnOnce(&mut Edit<'_>) -> Result<R, String>,
    ) -> Result<Observed<R>, SignalMutationError> {
        let path = self
            .persistence
            .as_ref()
            .filter(|p| p.enabled())
            .map(|p| p.path_for(&self.project_id));
        let lock_path = path.as_ref().map(|path| path.with_extension("json.lock"));
        let target = match (&path, &lock_path) {
            (Some(path), Some(lock_path)) => TransactionTarget::File { path, lock_path },
            _ => TransactionTarget::Memory(&self.signals),
        };
        let result = transact(
            target,
            || Signals {
                project_id: self.project_id.clone(),
                signals: Vec::new(),
            },
            |state| validate(state, &self.project_id),
            |state| {
                let mut edit = Edit {
                    signals: state,
                    project_id: self.project_id.clone(),
                    events: Vec::new(),
                };
                let value = apply(&mut edit)?;
                Ok((value, edit.events))
            },
        );
        match result {
            Ok(committed) => {
                self.signals = committed.state;
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

    fn publish_committed(&self, frames: Vec<SignalFrame>) {
        let captured: std::collections::HashSet<String> = frames
            .iter()
            .filter_map(|frame| match frame {
                SignalFrame::SignalCaptured { signal } => Some(signal.id.clone()),
                _ => None,
            })
            .collect();
        let mut seen = std::collections::HashSet::new();
        let mut committed = Vec::new();
        for frame in frames.into_iter().rev() {
            let old = match &frame {
                SignalFrame::SignalCaptured { signal }
                | SignalFrame::SignalChanged { signal }
                | SignalFrame::SignalDismissed { signal } => signal,
            };
            if !seen.insert(old.id.clone()) {
                continue;
            }
            let Some(signal) = self
                .signals
                .signals
                .iter()
                .find(|signal| signal.id == old.id)
                .cloned()
            else {
                continue;
            };
            if captured.contains(&signal.id) {
                committed.push(SignalFrame::SignalCaptured { signal });
                continue;
            }
            committed.push(match frame {
                SignalFrame::SignalCaptured { .. } => SignalFrame::SignalCaptured { signal },
                SignalFrame::SignalDismissed { .. } => SignalFrame::SignalDismissed { signal },
                SignalFrame::SignalChanged { .. } => SignalFrame::SignalChanged { signal },
            });
        }
        for frame in committed.into_iter().rev() {
            self.record_event(frame);
        }
    }
}
