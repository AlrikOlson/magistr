use magistr_domain::identity::RunId;

use crate::PortFuture;

use super::RunProjection;

/// Reads an admitted run projection without leaking native snapshot/store types.
/// The host enforces `ReadProjection` and returns the requested exact run,
/// retaining unknown freshness when it cannot prove a current authority read.
pub trait ProjectionPort: Send {
    fn read_run(&mut self, run: &RunId) -> PortFuture<'_, RunProjection>;
}
