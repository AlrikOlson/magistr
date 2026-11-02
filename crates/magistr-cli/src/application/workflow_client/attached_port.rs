//! The engine port over an owner this process did not open.

use std::sync::{Arc, Mutex, PoisonError};

use magistr_engine::Observed;
use magistr_engine::native::{Signal, SignalPromotion, Task, WorkflowMutationError};
use magistr_engine::{
    OwnerClient, OwnerCommand, OwnerGeneration, OwnerReply, PlanCommand, SignalCommand,
    ThinkCommand,
};

use magistr_cockpit::cockpit::ProposalKind;
use magistr_cockpit::engine::EnginePort;

/// The [`EnginePort`] a second cockpit holds: every verb is sent to the
/// project's running host over its owner endpoint and carried out by
/// that host's writer — the same typed verb
/// [`NativeEnginePort`](super::NativeEnginePort) would have called on an
/// in-process writer, in the one process that has one. This port never
/// admits anything.
///
/// A dropped connection is re-attached on the next verb. When the owner
/// that answers is a newer generation than the one this port was welcomed
/// under, the verb is refused once with the reason: the project changed
/// hands, and what the cockpit shows was read from the previous owner.
/// The cockpit's re-seed catches up on its own; the next verb goes
/// through under the new generation.
pub struct AttachedEnginePort {
    client: Arc<Mutex<OwnerClient>>,
    generation: OwnerGeneration,
}

impl AttachedEnginePort {
    /// A port over `client`, welcomed under its current generation.
    #[must_use]
    pub fn new(client: Arc<Mutex<OwnerClient>>) -> Self {
        let generation = client
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .generation();
        Self { client, generation }
    }

    /// The generation the port last acted under.
    #[must_use]
    pub const fn generation(&self) -> OwnerGeneration {
        self.generation
    }

    fn run(&mut self, command: OwnerCommand) -> Result<OwnerReply, WorkflowMutationError> {
        let mut client = self.client.lock().unwrap_or_else(PoisonError::into_inner);
        if !client.is_attached() {
            let generation = client
                .reconnect()
                .map_err(|error| WorkflowMutationError::rejected(error.to_string()))?;
            if generation != self.generation {
                let previous = self.generation;
                self.generation = generation;
                return Err(WorkflowMutationError::rejected(format!(
                    "The project changed while you were working ({previous} → {generation}). Refresh and try again."
                )));
            }
        }
        client.command(command).map_err(|error| match error {
            magistr_engine::native::AttachError::Engine(error) => *error,
            magistr_engine::native::AttachError::StaleGeneration { .. }
            | magistr_engine::native::AttachError::Detached
            | magistr_engine::native::AttachError::Refused(_) => {
                WorkflowMutationError::rejected(error.to_string())
            }
            error => WorkflowMutationError::uncertain(format!(
                "The result could not be confirmed: {error}. Read the saved work before retrying."
            )),
        })
    }

    fn task(&mut self, command: PlanCommand) -> Result<Observed<Task>, WorkflowMutationError> {
        match self.run(OwnerCommand::Plan(command))? {
            OwnerReply::Task(task) => Ok(*task),
            other => Err(WorkflowMutationError::uncertain(format!(
                "The result could not be confirmed: {other:?}"
            ))),
        }
    }

    fn signal(
        &mut self,
        command: SignalCommand,
    ) -> Result<Observed<Signal>, WorkflowMutationError> {
        match self.run(OwnerCommand::Signal(command))? {
            OwnerReply::Signal(signal) => Ok(signal),
            other => Err(WorkflowMutationError::uncertain(format!(
                "The result could not be confirmed: {other:?}"
            ))),
        }
    }
}

impl EnginePort for AttachedEnginePort {
    fn pin(
        &mut self,
        step_number: u32,
        pinned: bool,
    ) -> Result<Observed<bool>, WorkflowMutationError> {
        match self.run(OwnerCommand::Think(ThinkCommand::PinStep {
            step_number,
            pinned,
        }))? {
            OwnerReply::Pinned(previous) => Ok(previous),
            other => Err(WorkflowMutationError::uncertain(format!(
                "The result could not be confirmed: {other:?}"
            ))),
        }
    }

    fn checkpoint(&mut self) -> Result<serde_json::Value, String> {
        match self
            .run(OwnerCommand::Think(ThinkCommand::Checkpoint))
            .map_err(|error| error.to_string())?
        {
            OwnerReply::Report(report) => Ok(report),
            other => Err(format!("the owner answered {other:?} instead of a report")),
        }
    }

    fn start_task(&mut self, id: &str) -> Result<Observed<Task>, WorkflowMutationError> {
        self.task(PlanCommand::StartTask { id: id.to_owned() })
    }

    fn complete_task(&mut self, id: &str) -> Result<Observed<Task>, WorkflowMutationError> {
        self.task(PlanCommand::CompleteTask {
            id: id.to_owned(),
            step_ref: None,
        })
    }

    fn resolve_proposal(
        &mut self,
        id: &str,
        kind: ProposalKind,
        accept: bool,
    ) -> Result<Observed<Task>, WorkflowMutationError> {
        let id = id.to_owned();
        self.task(match kind {
            ProposalKind::Title => PlanCommand::ResolveTitleProposal { id, accept },
            ProposalKind::Status => PlanCommand::ResolveStatusProposal { id, accept },
            ProposalKind::Reprioritize => PlanCommand::ResolveReprioritize { id, accept },
        })
    }

    fn clear_blocker(&mut self, id: &str) -> Result<Observed<Task>, WorkflowMutationError> {
        self.task(PlanCommand::ClearWaitingOn { id: id.to_owned() })
    }

    fn promote_signal(&mut self, id: &str) -> Result<SignalPromotion, WorkflowMutationError> {
        match self.run(OwnerCommand::Signal(SignalCommand::Promote {
            id: id.to_owned(),
        }))? {
            OwnerReply::Promotion(promotion) => Ok(*promotion),
            other => Err(WorkflowMutationError::uncertain(format!(
                "The result could not be confirmed: {other:?}"
            ))),
        }
    }

    fn ignore_signal(&mut self, id: &str) -> Result<Observed<Signal>, WorkflowMutationError> {
        self.signal(SignalCommand::Ignore { id: id.to_owned() })
    }

    fn snooze_signal(
        &mut self,
        id: &str,
        minutes: u32,
    ) -> Result<Observed<Signal>, WorkflowMutationError> {
        self.signal(SignalCommand::Snooze {
            id: id.to_owned(),
            minutes: i64::from(minutes),
        })
    }
}
