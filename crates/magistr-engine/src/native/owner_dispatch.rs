//! Carrying an owner command out on the writer.

use super::WorkflowMutationError;
use super::native_engines::NativeEngines;
use super::owner_command::OwnerCommand;
use super::owner_reply::OwnerReply;
use super::plan_command::PlanCommand;
use super::run_command::RunCommand;
use super::signal_command::SignalCommand;
use super::think_command::ThinkCommand;

impl NativeEngines {
    /// Carry `command` out on this writer: the one place an attached
    /// client's request meets an engine, and exactly the typed verb the
    /// in-process caller would have used.
    ///
    /// # Errors
    /// The verb's own refusal, in the engine's words.
    pub fn apply(&self, command: OwnerCommand) -> Result<OwnerReply, WorkflowMutationError> {
        match command {
            OwnerCommand::Plan(command) => self.apply_plan(command),
            OwnerCommand::Think(command) => self.apply_think(command),
            OwnerCommand::Run(command) => self.apply_run(command),
            OwnerCommand::Signal(command) => self.apply_signal(command),
        }
    }

    fn apply_plan(&self, command: PlanCommand) -> Result<OwnerReply, WorkflowMutationError> {
        let task = match command {
            PlanCommand::StartTask { id } => self.plan_start_task(&id),
            PlanCommand::CompleteTask { id, step_ref } => {
                self.plan_complete_task(&id, step_ref.as_deref())
            }
            PlanCommand::AddTask {
                id,
                title,
                name,
                status,
                priority,
                description,
                acceptance,
                deps,
                content,
                feature,
            } => self.plan_add_task(
                id,
                title,
                name,
                status,
                priority,
                description,
                acceptance,
                deps,
                content,
                feature,
            ),
            PlanCommand::UpdateTask {
                id,
                title,
                priority,
                description,
                acceptance,
                deps,
                content,
            } => {
                self.plan_update_task(&id, title, priority, description, acceptance, deps, content)
            }
            PlanCommand::ClearWaitingOn { id } => self.plan_clear_waiting_on(&id),
            PlanCommand::SetWaitingOn {
                id,
                kind,
                reason,
                evidence,
            } => self.plan_set_waiting_on(&id, kind, reason, evidence),
            PlanCommand::SetStatus { id, to } => self.plan_set_status(&id, to),
            PlanCommand::RetireTask { id, reason } => self.plan_retire_task(&id, reason),
            PlanCommand::Reprioritize {
                id,
                suggested_priority,
                reason,
            } => self.plan_reprioritize(&id, suggested_priority, reason),
            PlanCommand::ProposeTitle {
                id,
                suggested_title,
                reason,
                source,
            } => self.plan_propose_title(&id, suggested_title, reason, source),
            PlanCommand::ProposeStatus {
                id,
                suggested_status,
                reason,
                source,
            } => self.plan_propose_status(&id, suggested_status, reason, source),
            PlanCommand::Link { id, cross_ref } => self.plan_link(&id, &cross_ref),
            PlanCommand::SetFeature { id, feature } => self.plan_set_feature(&id, feature),
            PlanCommand::ResolveTitleProposal { id, accept } => {
                self.plan_resolve_title_proposal(&id, accept)
            }
            PlanCommand::ResolveStatusProposal { id, accept } => {
                self.plan_resolve_status_proposal(&id, accept)
            }
            PlanCommand::ResolveReprioritize { id, accept } => {
                self.plan_resolve_reprioritize(&id, accept)
            }
        }?;
        Ok(OwnerReply::Task(Box::new(task)))
    }

    fn apply_think(&self, command: ThinkCommand) -> Result<OwnerReply, WorkflowMutationError> {
        Ok(match command {
            ThinkCommand::RecordStep { step } => {
                OwnerReply::ThinkStep(Box::new(self.think_record_step(*step)?))
            }
            ThinkCommand::PinStep {
                step_number,
                pinned,
            } => OwnerReply::Pinned(self.think_pin_step(step_number, pinned)?),
            ThinkCommand::Checkpoint => OwnerReply::Report(
                self.think_checkpoint()
                    .map_err(WorkflowMutationError::rejected)?,
            ),
        })
    }

    fn apply_run(&self, command: RunCommand) -> Result<OwnerReply, WorkflowMutationError> {
        Ok(match command {
            RunCommand::CompactCheckResults => OwnerReply::Report(
                serde_json::to_value(self.run_compact_checks()?)
                    .map_err(|error| WorkflowMutationError::uncertain(error.to_string()))?,
            ),
            RunCommand::OpenRun {
                description,
                acceptance_criteria,
                constraints,
                scope,
            } => OwnerReply::Cleared(self.run_open(
                description,
                acceptance_criteria,
                constraints,
                scope,
            )?),
            RunCommand::AddStep {
                id,
                title,
                step_type,
                estimate,
            } => OwnerReply::Done(self.add_step(id, title, step_type, estimate)?),
            RunCommand::StartStep { step_id } => OwnerReply::Step(self.start_step(&step_id)?),
            RunCommand::RecordProgress {
                step_id,
                progress_type,
                description,
                files_touched,
                tools_used,
                think_step,
            } => OwnerReply::Progress(self.record_progress(
                step_id.as_deref(),
                progress_type,
                description,
                files_touched,
                tools_used,
                think_step,
            )?),
            RunCommand::RecordGate {
                step_id,
                gate,
                command,
                exit_code,
                details,
                required,
                report,
            } => OwnerReply::Check(
                self.run_record_gate(
                    step_id.as_deref(),
                    &gate,
                    command,
                    exit_code,
                    details,
                    required,
                    report
                        .as_ref()
                        .map(|(format, path)| (format.as_str(), path.as_str())),
                )?,
            ),
            RunCommand::CompleteStep { step_id, artifacts } => {
                OwnerReply::Step(self.finish_step(&step_id, artifacts)?)
            }
            RunCommand::Finalize { artifacts, summary } => OwnerReply::Report(
                serde_json::to_value(self.run_finish(artifacts, summary)?)
                    .map_err(|error| WorkflowMutationError::uncertain(error.to_string()))?,
            ),
        })
    }

    fn apply_signal(&self, command: SignalCommand) -> Result<OwnerReply, WorkflowMutationError> {
        let signal = match command {
            SignalCommand::Capture { kind, from, body } => self.signal_capture(kind, from, body),
            SignalCommand::Research {
                id,
                summary,
                confidence,
                sources,
                think_step,
            } => self.signal_research(&id, summary, confidence, sources, think_step),
            SignalCommand::Promote { id } => {
                return Ok(OwnerReply::Promotion(Box::new(self.signal_promote(&id)?)));
            }
            SignalCommand::Ignore { id } => self.signal_ignore(&id),
            SignalCommand::Snooze { id, minutes } => self.signal_snooze(&id, minutes),
        }?;
        Ok(OwnerReply::Signal(signal))
    }
}
