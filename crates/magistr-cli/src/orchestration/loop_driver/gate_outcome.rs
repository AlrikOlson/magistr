//! What a gate job told the driver.

/// One gate's run, sent to the driver from inside the job that ran it —
/// the cockpit learns the same thing from the job's own result.
///
/// This is the shared [`GateRun`](crate::utils::gate::GateRun) and nothing
/// else. It used to be a three-field record of its own — `{name, passed,
/// duration}` — which threw away `stdout`, `stderr`, the exit code and the
/// difference between a timeout and a red test at the source, leaving the
/// cockpit one sentence for a run that took two and a half minutes. The
/// driver and `magistr_gate` now carry the same evidence because they run
/// the same runner.
pub(crate) type GateOutcome = crate::utils::gate::GateRun;
