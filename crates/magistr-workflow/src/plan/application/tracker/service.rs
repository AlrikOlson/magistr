use super::{
    SetupRequest, TargetPhase, TrackerSetupError, TrackerSetupReport, TrackerStatus, setup_local,
    setup_probe,
};
use crate::plan::application::{PlanError, PlanUseCases};
use crate::tracker::{TrackerConfig, TrackerPort};
use std::path::Path;

#[cfg(test)]
#[path = "service.tests.rs"]
mod tests;

impl PlanUseCases {
    pub fn tracker_status(&self, config: &TrackerConfig) -> Result<TrackerStatus, PlanError> {
        let engine = self.lock()?;
        let provider = config.provider.as_deref().unwrap_or_default();
        let (included, not_included) = if provider.is_empty() {
            (0, 0)
        } else {
            (
                engine.tasks_opted_in(provider).len(),
                engine.tasks_not_opted_in(provider).len(),
            )
        };
        Ok(TrackerStatus {
            project_id: engine.project_id().to_owned(),
            enabled: crate::tracker::should_project(config),
            provider: config.provider.clone(),
            target: config.target.clone(),
            included,
            not_included,
            inherits_new_tasks: crate::tracker::inherited_opt_in(config).is_some(),
        })
    }

    /// The adapter supplies its credential-resolved network port and startup
    /// paths. No mutation lock spans the network request and no stdin is read.
    pub async fn setup_tracker(
        &self,
        port: &dyn TrackerPort,
        req: &SetupRequest,
        create_missing: bool,
        data_dir: &Path,
    ) -> Result<TrackerSetupReport, TrackerSetupError> {
        let project_id = self
            .lock()
            .map_err(TrackerSetupError::plan)?
            .project_id()
            .to_owned();
        let display = req.name.as_deref().unwrap_or(req.into.trim());
        let phase = setup_probe(port, display, create_missing && !req.dry_run)
            .await
            .map_err(|error| TrackerSetupError::new("tracker_error", error).partial())?;
        if phase == TargetPhase::MissingAndNotCreated && !req.dry_run {
            return Err(TrackerSetupError::new(
                "target_missing",
                format!(
                    "'{}' does not exist on {}. Nothing was written. Ask the human whether to create it, then call again with create_missing: true — that writes to their tracker and cannot be undone from here.",
                    req.into.trim(),
                    req.provider.trim(),
                ),
            ));
        }
        let outcome = {
            let mut engine = self.lock().map_err(|error| {
                let error = TrackerSetupError::plan(error);
                if phase == TargetPhase::Created {
                    error.partial()
                } else {
                    error
                }
            })?;
            setup_local(&mut engine, req, data_dir, &project_id).map_err(|error| {
                if phase == TargetPhase::Created {
                    error.partial()
                } else {
                    error
                }
            })?
        };
        Ok(TrackerSetupReport {
            project_id,
            provider: req.provider.clone(),
            target: req.into.clone(),
            dry_run: req.dry_run,
            phase,
            outcome,
        })
    }
}
