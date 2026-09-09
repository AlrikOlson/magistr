use core::{
    future::Future,
    task::{Context, Poll, Waker},
};

use crate::{CommandContext, Revision};
use magistr_domain::identity::{AttemptId, CommandId, ProjectId, RunId};

pub(crate) fn run(project: &str, key: &str) -> RunId {
    RunId::new(ProjectId::new(project).expect("project fixture"), key).expect("run fixture")
}

pub(crate) fn command(run: &RunId) -> CommandContext {
    let command = CommandId::new(run.clone(), "command").expect("command fixture");
    CommandContext {
        attempt: AttemptId::new(command, core::num::NonZeroU32::MIN),
        expected_revision: Revision(7),
    }
}

/// Poll only deterministic immediately-ready fake ports; never run a real executor.
pub(crate) fn ready<F: Future>(future: F) -> F::Output {
    let mut future = core::pin::pin!(future);
    let mut context = Context::from_waker(Waker::noop());
    match future.as_mut().poll(&mut context) {
        Poll::Ready(value) => value,
        Poll::Pending => panic!("test port must be immediately ready"),
    }
}
