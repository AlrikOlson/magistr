//! Carrying an owner command out on the writer.

use super::WorkflowMutationError;
use super::native_engines::NativeEngines;
use super::owner_command::OwnerCommand;
use super::owner_reply::OwnerReply;
use super::roadmap_command::RoadmapCommand;
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
            OwnerCommand::Roadmap(command) => self.apply_roadmap(command),
            OwnerCommand::Think(command) => self.apply_think(command),
            OwnerCommand::Run(command) => self.apply_run(command),
            OwnerCommand::Signal(command) => self.apply_signal(command),
        }
    }

    fn apply_roadmap(&self, command: RoadmapCommand) -> Result<OwnerReply, WorkflowMutationError> {
        let chunk = match command {
            RoadmapCommand::StartChunk { id } => self.roadmap_start_chunk(&id),
            RoadmapCommand::CompleteChunk { id, step_ref } => {
                self.roadmap_complete_chunk(&id, step_ref.as_deref())
            }
            RoadmapCommand::AddChunk {
                id,
                title,
                name,
                status,
                priority,
                description,
                acceptance,
                deps,
                content,
                group,
            } => self.roadmap_add_chunk(
                id,
                title,
                name,
                status,
                priority,
                description,
                acceptance,
                deps,
                content,
                group,
            ),
            RoadmapCommand::UpdateChunk {
                id,
                title,
                priority,
                description,
                acceptance,
                deps,
                content,
            } => self.roadmap_update_chunk(
                &id,
                title,
                priority,
                description,
                acceptance,
                deps,
                content,
            ),
            RoadmapCommand::ClearBlockedBy { id } => self.roadmap_clear_blocked_by(&id),
            RoadmapCommand::SetBlockedBy {
                id,
                kind,
                reason,
                evidence,
            } => self.roadmap_set_blocked_by(&id, kind, reason, evidence),
            RoadmapCommand::SetStatus { id, to } => self.roadmap_set_status(&id, to),
            RoadmapCommand::ObsoleteChunk { id, reason } => {
                self.roadmap_obsolete_chunk(&id, reason)
            }
            RoadmapCommand::Reprioritize {
                id,
                suggested_priority,
                reason,
            } => self.roadmap_reprioritize(&id, suggested_priority, reason),
            RoadmapCommand::ProposeTitle {
                id,
                suggested_title,
                reason,
                source,
            } => self.roadmap_propose_title(&id, suggested_title, reason, source),
            RoadmapCommand::ProposeStatus {
                id,
                suggested_status,
                reason,
                source,
            } => self.roadmap_propose_status(&id, suggested_status, reason, source),
            RoadmapCommand::Link { id, cross_ref } => self.roadmap_link(&id, &cross_ref),
            RoadmapCommand::SetGroup { id, group } => self.roadmap_set_group(&id, group),
            RoadmapCommand::ResolveTitleProposal { id, accept } => {
                self.roadmap_resolve_title_proposal(&id, accept)
            }
            RoadmapCommand::ResolveStatusProposal { id, accept } => {
                self.roadmap_resolve_status_proposal(&id, accept)
            }
            RoadmapCommand::ResolveReprioritize { id, accept } => {
                self.roadmap_resolve_reprioritize(&id, accept)
            }
        }?;
        Ok(OwnerReply::Chunk(Box::new(chunk)))
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
