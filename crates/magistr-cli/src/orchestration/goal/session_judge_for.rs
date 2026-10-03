//! Who judges a loop an entry point or a host starts.

use std::path::Path;
use std::sync::Arc;

use magistr_application::effects::{SourceRefusal, SourceRoute};

use super::{GoalMode, Judge, SessionJudge};
use crate::agent::Backend;
use crate::orchestration::loop_driver::Connector;

/// Who judges an iteration: magistr's own fresh session ([`SessionJudge`])
/// on a roadmap-driven project under the native mode — and under the
/// delegated modes too, as what an unconfirmed delivery falls back to;
/// nobody under `off`, and nobody on a project with no chunk to judge.
/// The judge is constrained: it holds no source-write authority, so its
/// session advertises no write capability and refuses any write.
pub(crate) fn session_judge_for(
    goal: GoalMode,
    native: bool,
    dir: &Path,
    backend: Backend,
    inputs: &crate::config::inputs::ProjectInputs,
) -> Option<Arc<dyn Judge>> {
    match goal {
        GoalMode::Off => None,
        GoalMode::Native | GoalMode::Extension | GoalMode::Command | GoalMode::Auto if native => {
            Some(Arc::new(
                SessionJudge::new(
                    Connector::live(SourceRoute::Refused(SourceRefusal::NoAuthority)),
                    dir,
                    backend,
                )
                .with_inputs(inputs),
            ))
        }
        _ => None,
    }
}

#[cfg(test)]
#[path = "session_judge_for.tests.rs"]
mod tests;
