//! The driver's single control state, shared with its temporary conversation phases.
use crate::workflow::iteration::IterationLoop;
use magistr_cockpit::cockpit::{LoopCmd, Msg, RunCommand, WorkPhase, WorkRun};
use magistr_cockpit_runtime::channel::Inbox;
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub(super) struct Control {
    state: Arc<Mutex<WorkRun>>,
    inbox: Inbox,
}
impl Control {
    pub(super) fn new(run: &IterationLoop, inbox: Inbox) -> Self {
        Self {
            state: Arc::new(Mutex::new(WorkRun {
                id: run.state().run_id.clone(),
                mode: run.run_state().mode,
                track: run.track().map(str::to_owned),
                chunk: run.state().native_chunk_id.clone(),
                iteration: run.display_iteration(),
                limit: run.max_iterations(),
                phase: WorkPhase::Starting,
                requested: None,
            })),
            inbox,
        }
    }
    pub(super) fn publish(&self) {
        self.inbox.send(Msg::WorkRun(self.snapshot()));
    }
    pub(super) fn snapshot(&self) -> WorkRun {
        self.state
            .lock()
            .expect("work control lock poisoned")
            .clone()
    }
    fn update<T>(&self, change: impl FnOnce(&mut WorkRun) -> T) -> T {
        let mut state = self.state.lock().expect("work control lock poisoned");
        let before = state.clone();
        let result = change(&mut state);
        if *state != before {
            self.inbox.send(Msg::WorkRun(state.clone()));
        }
        result
    }
    pub(super) fn request(&self, command: RunCommand) -> Result<(), String> {
        self.update(|state| {
            if state.id != command.run_id || !state.is_active() {
                return Err(
                    "That work run has ended. Review the current work before trying again.".into(),
                );
            }
            if state.requested == Some(LoopCmd::Abort) && command.action != LoopCmd::Abort {
                return Err("Work will stop after the current item. Wait for it to finish.".into());
            }
            match command.action {
                LoopCmd::Resume => {
                    state.requested = None;
                    if state.phase == WorkPhase::Paused {
                        state.phase = WorkPhase::Starting;
                    }
                }
                LoopCmd::Step => {
                    state.requested = Some(LoopCmd::Step);
                    if state.phase == WorkPhase::Paused {
                        state.phase = WorkPhase::Starting;
                    }
                }
                LoopCmd::Pause if state.phase == WorkPhase::Paused => {}
                action => state.requested = Some(action),
            }
            Ok(())
        })
    }
    /// Arrive after an iteration (or before the first), without admitting work.
    pub(super) fn boundary(&self) {
        self.update(|state| {
            if state.requested == Some(LoopCmd::Pause)
                || (state.phase == WorkPhase::Running && state.requested == Some(LoopCmd::Step))
            {
                state.phase = WorkPhase::Paused;
                state.requested = None;
            }
        });
    }
    pub(super) fn pause(&self) {
        self.update(|state| {
            if state.requested != Some(LoopCmd::Abort) {
                state.requested = Some(LoopCmd::Pause);
            }
        });
    }
    pub(super) fn take_skip(&self) {
        self.update(|state| {
            state.requested = None;
            state.phase = WorkPhase::Starting;
        });
    }
    pub(super) fn begin(&self, run: &IterationLoop, chunk: Option<String>) {
        self.update(|state| {
            state.iteration = run.display_iteration();
            state.track = run.track().map(str::to_owned);
            state.chunk = chunk;
            state.phase = WorkPhase::Running;
        });
    }
    pub(super) fn end(&self, reason: String) {
        self.update(|state| {
            state.phase = WorkPhase::Ended { reason };
            state.requested = None;
        });
    }
}

#[cfg(test)]
#[path = "control.tests.rs"]
mod tests;
