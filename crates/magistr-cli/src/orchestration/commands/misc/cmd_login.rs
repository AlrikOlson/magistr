//! `magistr login` — sign in to an identity provider by device code.

use std::path::Path;
use std::time::Duration;

use chrono::Utc;
use color_eyre::eyre;

use crate::error::MagistrError;
use crate::output::OutputRenderer;
use crate::team::TeamConfig;
use crate::team::auth::{
    DeviceAuthorization, HttpDeviceFlow, IdentityCache, IdentityProvider, VerifiedIdentity, login,
};
use crate::utils::format as fmt;

use super::login_provider::login_provider;

/// Run the device-code login and cache the result.
///
/// `status` short-circuits to a listing of what is already cached —
/// the one way to see whether a login has expired without attempting
/// another one.
pub fn cmd_login(
    dir: &Path,
    kind: Option<&str>,
    issuer: Option<&str>,
    client_id: Option<&str>,
    scope: Option<&str>,
    status: bool,
    renderer: &dyn OutputRenderer,
) -> eyre::Result<()> {
    if status {
        return report_status(renderer);
    }

    let configured = TeamConfig::load(dir)?.and_then(|config| config.identity_provider);
    let provider = login_provider(
        configured.as_ref(),
        kind,
        issuer,
        client_id,
        scope,
        std::env::var("MAGISTR_GITHUB_CLIENT_ID").ok().as_deref(),
    )?;

    let machine = renderer.is_machine_readable();
    let flow = HttpDeviceFlow::connect(provider.clone())?;
    let identity = login(
        &flow,
        &mut |auth| prompt(auth, &provider, machine),
        &mut |wait: Duration| std::thread::sleep(wait),
    )?;

    let mut cache = IdentityCache::load();
    cache.put(identity.clone());
    cache.save()?;

    report_login(&identity, &provider, renderer)
}

/// Tell the human what to type and where.
///
/// Suppressed under a machine-readable renderer: a script that asked
/// for JSON cannot type a code anyway, and half-written prose on stdout
/// would corrupt the document.
fn prompt(auth: &DeviceAuthorization, provider: &IdentityProvider, machine: bool) {
    if machine {
        return;
    }
    let target = auth
        .verification_uri_complete
        .as_deref()
        .unwrap_or(&auth.verification_uri);
    eprintln!(
        "{}\n",
        fmt::bold(&format!("🔐 Sign in to {}", provider.label()))
    );
    eprintln!("  {} {}", fmt::bold("Open:"), target);
    eprintln!("  {} {}", fmt::bold("Code:"), fmt::bold(&auth.user_code));
    eprintln!();
    eprintln!(
        "  {}",
        fmt::dim("Waiting for you to finish in the browser…")
    );
}

fn report_login(
    identity: &VerifiedIdentity,
    provider: &IdentityProvider,
    renderer: &dyn OutputRenderer,
) -> eyre::Result<()> {
    if renderer.is_machine_readable() {
        let data = serde_json::json!({
            "result": "signed in",
            "provider": provider.key(),
            "identity": identity,
        });
        renderer
            .json(&mut std::io::stdout(), &data)
            .map_err(|e| MagistrError::Serialize(e.to_string()))?;
        return Ok(());
    }
    let email = identity
        .email
        .as_deref()
        .map(|e| format!(" <{e}>"))
        .unwrap_or_default();
    eprintln!();
    eprintln!(
        "{} Signed in to {} as {}{email}",
        fmt::icon_pass(),
        provider.label(),
        fmt::bold(identity.display_name()),
    );
    eprintln!(
        "  {}",
        fmt::dim(&format!(
            "Valid until {} — `magistr logout` clears it",
            identity.expires_at.to_rfc3339()
        ))
    );
    Ok(())
}

fn report_status(renderer: &dyn OutputRenderer) -> eyre::Result<()> {
    let cache = IdentityCache::load();
    let now = Utc::now();

    if renderer.is_machine_readable() {
        let logins: Vec<_> = cache
            .entries()
            .map(|id| {
                serde_json::json!({
                    "provider": id.provider,
                    "subject": id.subject,
                    "name": id.name,
                    "email": id.email,
                    "expires_at": id.expires_at,
                    "expired": id.is_expired(now),
                })
            })
            .collect();
        renderer
            .json(
                &mut std::io::stdout(),
                &serde_json::json!({ "logins": logins }),
            )
            .map_err(|e| MagistrError::Serialize(e.to_string()))?;
        return Ok(());
    }

    eprintln!("{}\n", fmt::bold("🔐 Verified identities"));
    if cache.is_empty() {
        eprintln!(
            "  {}",
            fmt::dim("Nothing cached — run `magistr login` to sign in.")
        );
        return Ok(());
    }
    for identity in cache.entries() {
        let state = if identity.is_expired(now) {
            "expired".to_string()
        } else {
            format!("until {}", identity.expires_at.to_rfc3339())
        };
        eprintln!(
            "  {}  {}  {}",
            fmt::bold(&format!("{:<28}", identity.provider)),
            identity.display_name(),
            fmt::dim(&state),
        );
    }
    Ok(())
}
