use crate::ship::domain::check::Check;

use super::{
    CheckAdmission, CheckOutcome, CheckRequest, CheckSettlement, ShipError, ShipUseCases,
    TaskAdmission,
};

impl ShipUseCases {
    pub fn admit_task(&self, task_id: Option<&str>) -> Result<TaskAdmission, String> {
        self.snapshot()?.admit_task(task_id)
    }

    pub fn admit_check(&self, request: CheckRequest) -> Result<CheckAdmission, ShipError> {
        if request.command.is_none() && request.passed.is_none() {
            return Err(ShipError::invalid_args(
                "`passed` is required when no `command` is given (or pass a `command` to run and verify the check).",
            ));
        }
        let target = self.admit_task(request.task_id.as_deref())?;
        Ok(CheckAdmission { target, request })
    }

    pub fn settle_check(
        &self,
        admission: CheckAdmission,
        outcome: CheckOutcome,
    ) -> Result<CheckSettlement, String> {
        self.lock()?.settle_admitted_check(admission, outcome)
    }

    /// Immediate recording for a host whose command has already completed.
    /// Hosts that may span task/run changes should retain `admit_check` instead.
    pub fn record_check(
        &self,
        request: CheckRequest,
        outcome: CheckOutcome,
    ) -> Result<Check, String> {
        self.lock()?
            .record_check_full(
                request.task_id.as_deref(),
                request.check_type,
                request.name,
                outcome.passed,
                outcome.details,
                request.required,
                outcome.verified,
                outcome.command,
                outcome.exit_code,
                outcome.report,
                outcome.results,
            )
            .cloned()
    }
}
