//! Resolve pricing for a model id an agent advertised.

use super::model_entry::ModelEntry;
use super::model_registry::ModelRegistry;

/// Look up the built-in pricing entry for a model id an agent advertised.
///
/// Agents advertise a mix of concrete ids (`claude-fable-5-1`) and bare aliases
/// (`sonnet`, `opus`, `default`), optionally carrying a context-window suffix
/// (`claude-fable-5-1[1m]`). Only the concrete ids can be priced: an alias is
/// resolved by the agent's own SDK to a model magistr cannot see, so guessing
/// which one would reintroduce exactly the fabrication that made the picker
/// disagree with `magistr models`.
///
/// Returns `None` for aliases and unknown ids — the caller should show no
/// price rather than a wrong one.
#[must_use]
pub fn advertised_pricing<'a>(
    advertised_id: &str,
    registry: &'a ModelRegistry,
) -> Option<&'a ModelEntry> {
    registry.get(strip_context_suffix(advertised_id))
}

/// Strip a trailing context-window marker, e.g. `claude-fable-5-1[1m]`.
fn strip_context_suffix(id: &str) -> &str {
    match id.split_once('[') {
        Some((base, rest)) if rest.ends_with(']') => base,
        _ => id,
    }
}

#[cfg(test)]
#[path = "advertised_pricing.tests.rs"]
mod tests;
