//! A check admitted before it runs and settled against that admission.

use std::path::{Path, PathBuf};

use magistr_engine::tas::{CheckAdmission, NativeEngines, TaskAdmission};
use magistr_workspace::snapshot::{SelectionPolicy, SourceSnapshot};

use super::{CheckEvidenceError, CheckIdentities, CheckSource, SettledCheck};
use crate::utils::gate::nextest_junit_report_path;

/// One gate, admitted by the workflow before its command runs and — when
/// the consumer asked — bound to the workspace's source as observed at
/// that moment.
///
/// Admission captures the concrete task instance the check belongs to, so
/// a task started while the gate ran can never collect its result; the
/// optional snapshot captures the selected bytes lock-free, so the command
/// runs with the owner's edits still flowing and nothing waits on it.
/// Settlement re-observes the same root, then settles through the same
/// admitted writer, in that order: the source validation is taken as close
/// to the command's end as possible, before the workflow lock is touched.
#[derive(Clone, Debug)]
pub struct AdmittedCheck {
    admission: CheckAdmission,
    root: PathBuf,
    source: Option<SourceSnapshot>,
}

impl AdmittedCheck {
    /// Admit `gate` on `task_id` (the active task when `None`) through the
    /// admitted writer, then observe `root` under `source` when a policy is
    /// given. The workflow is asked first: a check with no task to land on
    /// is refused before any bytes are hashed.
    ///
    /// # Errors
    /// `Admission` when the workflow names no task or refuses the request;
    /// `Source` when the workspace cannot be observed under the policy.
    pub fn admit(
        engines: &NativeEngines,
        root: &Path,
        task_id: Option<&str>,
        gate: &str,
        command: String,
        required: bool,
        source: Option<SelectionPolicy>,
    ) -> Result<Self, CheckEvidenceError> {
        let admission = engines
            .ship_admit_gate(task_id, gate, command, required)
            .map_err(CheckEvidenceError::Admission)?;
        let source = source
            .map(|policy| SourceSnapshot::capture(root, policy))
            .transpose()?;
        Ok(Self {
            admission,
            root: root.to_path_buf(),
            source,
        })
    }

    /// The workflow identity the result will be settled against.
    #[must_use]
    pub fn target(&self) -> &TaskAdmission {
        &self.admission.target
    }

    /// The full admission: target and the request as admitted.
    #[must_use]
    pub fn admission(&self) -> &CheckAdmission {
        &self.admission
    }

    /// The source observed at admission, when one was asked for.
    #[must_use]
    pub fn source(&self) -> Option<&SourceSnapshot> {
        self.source.as_ref()
    }

    /// Both identities, side by side and never combined.
    #[must_use]
    pub fn identities(&self) -> CheckIdentities {
        CheckIdentities::new(
            &self.admission.target,
            self.source.as_ref().map(|s| s.identity.clone()),
        )
    }

    /// Settle the finished command: re-observe the source it was admitted
    /// with, then settle the check under its captured identity. A `cargo
    /// nextest` command's junit report is parsed additively from the
    /// project's `target/nextest/<profile>/junit.xml`; the exit code alone
    /// decides `passed`, and a missing report is "not parsed".
    ///
    /// # Errors
    /// `Source` when the workspace cannot be re-observed; `Settlement` when
    /// the workflow refuses. A superseded run or task is not an error: it
    /// is a `Stale` settlement carrying the admission and outcome intact.
    pub fn settle(
        self,
        engines: &NativeEngines,
        exit_code: i32,
        details: String,
    ) -> Result<SettledCheck, CheckEvidenceError> {
        let source = match self.source {
            Some(before) => {
                let validation = before.validate(&self.root)?;
                Some(CheckSource { before, validation })
            }
            None => None,
        };
        let report = self
            .admission
            .request
            .command
            .as_deref()
            .and_then(nextest_junit_report_path)
            .map(|relative| self.root.join(relative).to_string_lossy().into_owned());
        let settlement = engines
            .ship_settle_gate(
                self.admission,
                exit_code,
                details,
                report.as_deref().map(|path| ("junit", path)),
            )
            .map_err(CheckEvidenceError::Settlement)?;
        Ok(SettledCheck { settlement, source })
    }
}

#[cfg(test)]
#[path = "admitted_check.tests.rs"]
mod tests;
