use crate::worker::WorkerGuestEnd;

/// One `(string) -> string` component invoke as the worker answered it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginInvokeOutcome {
    pub end: WorkerGuestEnd,
    pub fuel_used: u64,
    /// The export's result text, present iff the run ended ok.
    pub result: Option<String>,
}

impl PluginInvokeOutcome {
    #[must_use]
    pub fn ok(fuel_used: u64, result: impl Into<String>) -> Self {
        Self {
            end: WorkerGuestEnd::Ok,
            fuel_used,
            result: Some(result.into()),
        }
    }

    #[must_use]
    pub fn ended(end: WorkerGuestEnd, fuel_used: u64) -> Self {
        Self {
            end,
            fuel_used,
            result: None,
        }
    }

    /// Stable outcome tag ("ok", "out-of-fuel", "memory-cap", "cap-denied",
    /// "guest-trap", "guest-exit", "invalid-guest").
    #[must_use]
    pub const fn kind(&self) -> &'static str {
        match self.end {
            WorkerGuestEnd::Ok => "ok",
            WorkerGuestEnd::OutOfFuel => "out-of-fuel",
            WorkerGuestEnd::MemoryCap => "memory-cap",
            WorkerGuestEnd::CapDenied(_) => "cap-denied",
            WorkerGuestEnd::GuestTrap(_) => "guest-trap",
            WorkerGuestEnd::GuestExit(_) => "guest-exit",
            WorkerGuestEnd::InvalidGuest(_) => "invalid-guest",
        }
    }

    /// Human-readable detail (import name, trap message, exit code, …).
    #[must_use]
    pub fn detail(&self) -> String {
        match &self.end {
            WorkerGuestEnd::Ok | WorkerGuestEnd::OutOfFuel | WorkerGuestEnd::MemoryCap => {
                String::new()
            }
            WorkerGuestEnd::CapDenied(name) => name.clone(),
            WorkerGuestEnd::GuestTrap(message) | WorkerGuestEnd::InvalidGuest(message) => {
                message.clone()
            }
            WorkerGuestEnd::GuestExit(code) => code.to_string(),
        }
    }

    /// `kind` plus detail, the breach reason vocabulary.
    #[must_use]
    pub fn reason(&self) -> String {
        let detail = self.detail();
        if detail.is_empty() {
            self.kind().to_owned()
        } else {
            format!("{}: {detail}", self.kind())
        }
    }
}
