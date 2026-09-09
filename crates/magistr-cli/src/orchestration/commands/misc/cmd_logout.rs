//! `magistr logout` — drop cached verified identities.

use color_eyre::eyre;

use crate::error::MagistrError;
use crate::output::OutputRenderer;
use crate::team::auth::IdentityCache;
use crate::utils::format as fmt;

/// Clear one provider's cached login, or every one of them.
///
/// Naming a provider that is not cached is reported, not treated as an
/// error: `logout` is idempotent by intent — the caller wants the
/// credential gone, and it is.
pub fn cmd_logout(provider: Option<&str>, renderer: &dyn OutputRenderer) -> eyre::Result<()> {
    let mut cache = IdentityCache::load();
    let (cleared, subject) = match provider {
        Some(key) => (usize::from(cache.remove(key)), key.to_string()),
        None => (cache.clear(), "all providers".to_string()),
    };
    cache.save()?;

    let message = if cleared == 0 {
        format!("No cached login for {subject}")
    } else {
        format!("Cleared {cleared} cached login(s) for {subject}")
    };

    if renderer.is_machine_readable() {
        let data = serde_json::json!({ "result": message, "cleared": cleared });
        renderer
            .json(&mut std::io::stdout(), &data)
            .map_err(|e| MagistrError::Serialize(e.to_string()))?;
    } else {
        eprintln!("{} {message}", fmt::icon_pass());
    }
    Ok(())
}
