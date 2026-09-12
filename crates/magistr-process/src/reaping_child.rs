use std::ops::{Deref, DerefMut};
use std::time::Duration;

use tokio::process::Child;

/// A child whose abandoned owner still kills and explicitly reaps it.
/// Group signalling and registry membership belong to ProcessGroup.
pub struct ReapingChild(Option<Child>);

impl ReapingChild {
    /// Take sole ownership of a spawned child.
    pub fn new(child: Child) -> Self {
        Self(Some(child))
    }
}

impl Deref for ReapingChild {
    type Target = Child;
    fn deref(&self) -> &Child {
        self.0.as_ref().expect("child owned until drop")
    }
}

impl DerefMut for ReapingChild {
    fn deref_mut(&mut self) -> &mut Child {
        self.0.as_mut().expect("child owned until drop")
    }
}

impl Drop for ReapingChild {
    fn drop(&mut self) {
        let Some(mut child) = self.0.take() else {
            return;
        };
        if matches!(child.try_wait(), Ok(Some(_))) {
            return;
        }
        let _ = child.start_kill();
        // try_wait works without an async runtime. Keep the child until it is
        // reaped even if the caller's bounded cleanup already returned a fault.
        let _ = std::thread::Builder::new()
            .name("child-reaper".into())
            .spawn(move || {
                loop {
                    match child.try_wait() {
                        Ok(Some(_)) | Err(_) => break,
                        Ok(None) => std::thread::sleep(Duration::from_millis(10)),
                    }
                }
            });
    }
}
