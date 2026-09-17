use super::{RestoreFuture, WorkerIncarnation};

/// What the composing owner replays into every worker incarnation before the
/// supervisor dispatches any request to it: state the worker process owns and
/// a replacement forgets, such as registered language packs. The supervisor
/// admits a spawned process only after its readiness frame and this restore
/// have both succeeded, so an earlier incarnation's readiness never certifies
/// its replacement. An error leaves the process unadmitted (it is stopped) and
/// fails the request that needed it; the next request starts over.
pub trait WorkerRestore: Send + Sync {
    fn restore<'a>(&'a self, incarnation: &'a mut WorkerIncarnation<'_>) -> RestoreFuture<'a>;
}
