//! Carrying an owner command out on the writer.

use super::native_engines::NativeEngines;
use super::owner_command::OwnerCommand;
use super::owner_reply::OwnerReply;
use super::roadmap_command::RoadmapCommand;
use super::ship_command::ShipCommand;
use super::signal_command::SignalCommand;
use super::think_command::ThinkCommand;

impl NativeEngines {
    /// Carry `command` out on this writer: the one place an attached
    /// client's request meets an engine, and exactly the typed verb the
    /// in-process caller would have used.
    ///
    /// # Errors
    /// The verb's own refusal, in the engine's words.
    pub fn apply(&self, command: OwnerCommand) -> Result<OwnerReply, String> {
        match command {
            OwnerCommand::Roadmap(command) => self.apply_roadmap(command),
            OwnerCommand::Think(command) => self.apply_think(command),
            OwnerCommand::Ship(command) => self.apply_ship(command),
            OwnerCommand::Signal(command) => self.apply_signal(command),
        }
    }

    fn apply_roadmap(&self, command: RoadmapCommand) -> Result<OwnerReply, String> {
        let chunk = match command {
            RoadmapCommand::StartChunk { id } => self.roadmap_start_chunk(&id),
            RoadmapCommand::CompleteChunk { id, ship_ref } => {
                self.roadmap_complete_chunk(&id, ship_ref.as_deref())
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

    fn apply_think(&self, command: ThinkCommand) -> Result<OwnerReply, String> {
        Ok(match command {
            ThinkCommand::RecordStep { step } => OwnerReply::Step(
                self.think_record_step(*step)
                    .map(|ok| ok.text)
                    .map_err(|err| err.text),
            ),
            ThinkCommand::PinStep {
                step_number,
                pinned,
            } => OwnerReply::Pinned(self.think_pin_step(step_number, pinned)?),
            ThinkCommand::Checkpoint => OwnerReply::Report(self.think_checkpoint()?),
        })
    }

    fn apply_ship(&self, command: ShipCommand) -> Result<OwnerReply, String> {
        Ok(match command {
            ShipCommand::SetObjective {
                description,
                acceptance_criteria,
                constraints,
                scope,
            } => OwnerReply::Cleared(self.ship_set_objective(
                description,
                acceptance_criteria,
                constraints,
                scope,
            )?),
            ShipCommand::AddTask {
                id,
                title,
                task_type,
                estimate,
            } => {
                self.ship_add_task(id, title, task_type, estimate)?;
                OwnerReply::Done
            }
            ShipCommand::StartTask { task_id } => OwnerReply::Task(self.ship_start_task(&task_id)?),
            ShipCommand::RecordAction {
                task_id,
                action_type,
                description,
                files_touched,
                tools_used,
                think_step,
            } => OwnerReply::Action(self.ship_record_action(
                task_id.as_deref(),
                action_type,
                description,
                files_touched,
                tools_used,
                think_step,
            )?),
            ShipCommand::RecordGate {
                task_id,
                gate,
                command,
                exit_code,
                details,
                required,
                report,
            } => OwnerReply::Check(
                self.ship_record_gate(
                    task_id.as_deref(),
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
            ShipCommand::CompleteTask { task_id, artifacts } => {
                OwnerReply::Task(self.ship_complete_task(&task_id, artifacts)?)
            }
            ShipCommand::Finalize { artifacts, summary } => {
                OwnerReply::Report(self.ship_finalize(artifacts, summary)?)
            }
        })
    }

    fn apply_signal(&self, command: SignalCommand) -> Result<OwnerReply, String> {
        let signal = match command {
            SignalCommand::Capture { kind, from, body } => self.signal_capture(kind, from, body),
            SignalCommand::Research {
                id,
                summary,
                confidence,
                sources,
                think_step,
            } => self.signal_research(&id, summary, confidence, sources, think_step),
            SignalCommand::Promote { id } => self.signal_promote(&id),
            SignalCommand::Ignore { id } => self.signal_ignore(&id),
            SignalCommand::Snooze { id, minutes } => self.signal_snooze(&id, minutes),
        }?;
        Ok(OwnerReply::Signal(signal))
    }
}
