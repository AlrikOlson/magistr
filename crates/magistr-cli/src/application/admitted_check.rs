//! A check admitted before it runs and settled against that admission.

use std::path::{Path, PathBuf};

use magistr_engine::native::{
    CheckAdmission, CheckExecution, NativeEngines, SourceObservation, TaskAdmission,
};

use super::{CheckEvidenceError, CheckIdentities, SettledCheck};
use crate::utils::gate::nextest_junit_report_path;

/// One gate, admitted by the workflow before its command runs.
///
/// Admission captures the concrete task instance the check belongs to and
/// publishes its durable receipt, and — through the evidence capability
/// the writer was admitted with — observes the workspace's selected source
/// at that moment, so a task started while the gate ran can never collect
/// the result and the receipt names the bytes it certifies. The command
/// then runs with the owner's edits still flowing and nothing waiting on
/// it. Settlement re-observes the source, then settles through the same
/// admitted writer, in that order: the validation is taken as close to the
/// command's end as possible, before the workflow lock is touched.
#[derive(Clone, Debug)]
pub struct AdmittedCheck {
    admission: magistr_engine::Observed<CheckAdmission>,
    root: PathBuf,
}

impl AdmittedCheck {
    /// Admit `gate` on `task_id` (the active task when `None`) through the
    /// admitted writer. The workflow is asked first: a check with no task
    /// to land on is refused before any byte is hashed; a workspace that
    /// cannot be observed refuses the admission too, so an unobservable
    /// gate is never recorded as evidence.
    ///
    /// # Errors
    /// `Admission` when the workflow names no task or refuses the request;
    /// `Source` when the workspace cannot be observed.
    pub fn admit(
        engines: &NativeEngines,
        root: &Path,
        task_id: Option<&str>,
        gate: &str,
        command: String,
        required: bool,
    ) -> Result<Self, CheckEvidenceError> {
        Self::admit_with_options(
            engines,
            root,
            task_id,
            gate,
            command,
            required,
            Default::default(),
        )
    }

    /// Admit with an explicit input scope, rerun request or deadline.
    #[allow(clippy::too_many_arguments)]
    pub fn admit_with_options(
        engines: &NativeEngines,
        root: &Path,
        task_id: Option<&str>,
        gate: &str,
        command: String,
        required: bool,
        options: magistr_engine::native::CheckOptions,
    ) -> Result<Self, CheckEvidenceError> {
        let admission = engines
            .ship_admit_check(magistr_engine::native::CheckRequest {
                options,
                task_id: task_id.map(str::to_owned),
                check_type: magistr_engine::native::check_type_for(gate),
                name: gate.into(),
                passed: None,
                details: String::new(),
                required,
                command: Some(command),
                report: None,
            })
            .map_err(CheckEvidenceError::from_admission)?;
        Ok(Self {
            admission,
            root: root.to_path_buf(),
        })
    }

    /// Reuse matching evidence and settle it under this task, preserving the
    /// original execution and parsed report through the receipt reference.
    pub fn reuse(
        &self,
        engines: &NativeEngines,
    ) -> Result<Option<SettledCheck>, CheckEvidenceError> {
        let outcome = engines
            .ship_reuse_gate(&self.admission.value)
            .map_err(CheckEvidenceError::Settlement)?;
        outcome
            .map(|outcome| {
                self.clone()
                    .settle(engines, outcome.execution, outcome.details)
            })
            .transpose()
    }

    /// The workflow identity the result will be settled against.
    #[must_use]
    pub fn target(&self) -> &TaskAdmission {
        &self.admission.value.target
    }

    /// The full admission: target, request, receipt id, acceptance
    /// revision and the source observed.
    #[must_use]
    pub fn admission(&self) -> &CheckAdmission {
        &self.admission.value
    }

    /// The durable receipt this check runs under.
    #[must_use]
    pub fn receipt_id(&self) -> &str {
        &self.admission.value.receipt_id
    }

    /// The source observed at admission, when the writer carries the
    /// evidence capability.
    #[must_use]
    pub fn source(&self) -> Option<&SourceObservation> {
        self.admission.value.source.as_ref()
    }

    /// Both identities, side by side and never combined.
    #[must_use]
    pub fn identities(&self) -> CheckIdentities {
        CheckIdentities::new(
            &self.admission.value.target,
            self.admission.value.source.as_ref(),
        )
    }

    /// Settle the finished command under its captured identity. A `cargo
    /// nextest` command's junit report is parsed additively from the
    /// project's `target/nextest/<profile>/junit.xml`; only an exit decides
    /// `passed`, and a missing report is "not parsed".
    ///
    /// # Errors
    /// `Settlement` when the workflow refuses. A superseded run or task is
    /// not an error: it is a `Stale` settlement carrying the admission,
    /// the outcome and the settled receipt intact.
    pub fn settle(
        self,
        engines: &NativeEngines,
        execution: CheckExecution,
        details: String,
    ) -> Result<SettledCheck, CheckEvidenceError> {
        let report = self
            .admission
            .value
            .request
            .command
            .as_deref()
            .and_then(nextest_junit_report_path)
            .map(|relative| self.root.join(relative).to_string_lossy().into_owned());
        let settlement = engines
            .ship_settle_gate(
                self.admission.value,
                execution,
                details,
                report.as_deref().map(|path| ("junit", path)),
            )
            .map_err(CheckEvidenceError::Settlement)?;
        Ok(SettledCheck { settlement })
    }
}

#[cfg(test)]
#[path = "admitted_check.tests.rs"]
mod tests;
