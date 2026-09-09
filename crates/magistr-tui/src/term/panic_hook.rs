//! Leaving the screen before a panic message prints.

use std::panic::{self, PanicHookInfo};
use std::sync::{Arc, Mutex, PoisonError};

use super::restorer::Restorer;
use super::session::Session;

type Hook = Box<dyn Fn(&PanicHookInfo<'_>) + Send + Sync + 'static>;

/// The session's restorer, taken by whichever panic comes first.
static SLOT: Mutex<Option<(Session, Restorer)>> = Mutex::new(None);

/// The panic hook for a session: while it lives, a panic on any thread
/// writes the session's leave sequence and puts the terminal back (raw
/// mode off, stderr restored and replayed) *before* the previous hook
/// prints the message — so the message lands on a normal screen and never
/// inside the alternate one. Dropping it (the normal end of a run) takes
/// the restorer out and puts the previous hook back.
///
/// A panic anywhere in the cockpit process restores the terminal, an
/// input or job thread included: whatever thread it is, the message is a
/// bug report worth reading, and the runtime is winding down either way.
pub struct PanicHook {
    previous: Arc<Hook>,
}

impl std::fmt::Debug for PanicHook {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PanicHook")
            .field("installed", &PanicHook::is_installed())
            .finish()
    }
}

impl PanicHook {
    /// Install the hook for `session`, wrapping whatever hook was set.
    #[must_use]
    pub fn install(session: Session, restorer: Restorer) -> Self {
        *slot() = Some((session, restorer));
        let previous: Arc<Hook> = Arc::new(panic::take_hook());
        let chained = Arc::clone(&previous);
        panic::set_hook(Box::new(move |info| {
            restore_terminal();
            chained(info);
        }));
        Self { previous }
    }

    /// Whether a session's restorer is armed.
    #[must_use]
    pub fn is_installed() -> bool {
        slot().is_some()
    }
}

impl Drop for PanicHook {
    fn drop(&mut self) {
        slot().take();
        // The hook cannot be swapped from a panicking thread (`take_hook`
        // would panic again and abort the process). Unwinding out of a run
        // leaves the wrapper installed and disarmed: with an empty slot it
        // only chains to the previous hook.
        if std::thread::panicking() {
            return;
        }
        let _ours = panic::take_hook();
        let previous = Arc::clone(&self.previous);
        panic::set_hook(Box::new(move |info| previous(info)));
    }
}

fn slot() -> std::sync::MutexGuard<'static, Option<(Session, Restorer)>> {
    SLOT.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Take the restorer, if any, and run it with the leave sequence. A
/// `try_lock`: a panic raised while the slot is held must not deadlock
/// inside the hook — it just prints on whatever screen there is.
fn restore_terminal() {
    let taken = SLOT.try_lock().ok().and_then(|mut slot| slot.take());
    if let Some((session, restorer)) = taken {
        let mut out = Vec::new();
        session.leave(&mut out);
        restorer(&out);
    }
}

#[cfg(test)]
#[path = "panic_hook.tests.rs"]
mod tests;
