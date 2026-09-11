use super::PluginManifest;
use crate::worker::WorkerPolicy;

/// The one fuel/memory policy every plugin and script guest runs under.
/// Manifest and request limits clamp downward; nothing widens the ceiling.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PluginGuestPolicy {
    pub fuel: u64,
    pub memory_cap_bytes: usize,
}

impl PluginGuestPolicy {
    /// Host ceiling on fuel per run (also the script lane's default).
    pub const FUEL_CAP: u64 = 1_000_000_000;
    /// Host ceiling on guest linear memory (256 MiB).
    pub const MEMORY_CAP: usize = 256 * 1024 * 1024;

    /// The host ceiling itself.
    #[must_use]
    pub const fn ceiling() -> Self {
        Self {
            fuel: Self::FUEL_CAP,
            memory_cap_bytes: Self::MEMORY_CAP,
        }
    }

    /// Manifest limits clamp downward against the host ceiling.
    #[must_use]
    pub fn for_manifest(manifest: &PluginManifest) -> Self {
        Self::clamped(manifest.limits.fuel, manifest.limits.memory_bytes)
    }

    /// Requested limits (a manifest or a script call) clamped to the ceiling.
    #[must_use]
    pub fn clamped(fuel: Option<u64>, memory_cap_bytes: Option<usize>) -> Self {
        Self {
            fuel: fuel.unwrap_or(Self::FUEL_CAP).min(Self::FUEL_CAP),
            memory_cap_bytes: memory_cap_bytes
                .unwrap_or(Self::MEMORY_CAP)
                .min(Self::MEMORY_CAP),
        }
    }
}

impl From<PluginGuestPolicy> for WorkerPolicy {
    fn from(policy: PluginGuestPolicy) -> Self {
        Self {
            fuel: policy.fuel,
            memory_cap_bytes: policy.memory_cap_bytes,
        }
    }
}
