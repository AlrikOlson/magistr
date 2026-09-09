//! Who is running magistr — local identity, verified where a team asks.

use std::path::Path;
use std::process::{Command, Stdio};

use chrono::Utc;

use super::auth::{IdentityCache, VerifiedIdentity};
use super::team_config::TeamConfig;

/// The resolved identity of whoever is running magistr.
///
/// Resolution precedence: the `MAGISTR_USER` environment variable
/// (accepts `Name <email>`, a bare email, or a bare username), then an
/// unexpired verified login from [`super::auth`], then git config
/// `user.name` / `user.email`, then the OS username.
///
/// Only the verified branch is authentication; every other source is
/// self-asserted, which [`provider`] is how a caller tells them apart.
/// `MAGISTR_USER` still outranks a login because it is an explicit
/// local override — and an override cannot forge verification, since it
/// leaves `provider` unset and so fails [`super::auth::require_verified`].
///
/// [`provider`]: Identity::provider
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Identity {
    /// Display name; falls back to the email local part or `"unknown"`.
    pub name: String,
    /// Email when any source provided one.
    pub email: Option<String>,
    /// The provider key that vouched for this identity, when one did.
    /// `None` means self-asserted.
    pub provider: Option<String>,
}

impl Identity {
    /// Resolve the local identity for a project directory.
    #[must_use]
    pub fn resolve(dir: &Path) -> Self {
        let env_user = std::env::var("MAGISTR_USER")
            .ok()
            .filter(|s| !s.trim().is_empty());
        let git_name = git_config(dir, "user.name");
        let git_email = git_config(dir, "user.email");
        let os_user = std::env::var("USER")
            .ok()
            .or_else(|| std::env::var("USERNAME").ok())
            .filter(|s| !s.trim().is_empty());
        from_parts(env_user, verified(dir), git_name, git_email, os_user)
    }

    /// The matching key against roster member ids: email when present,
    /// the name otherwise.
    #[must_use]
    pub fn id(&self) -> &str {
        self.email.as_deref().unwrap_or(&self.name)
    }
}

/// The verified login this project should resolve with, if any.
///
/// The roster's `identity_provider` is the preference, so a machine
/// logged into two providers resolves as the one this team asked for.
/// A roster that will not load is treated as no roster: identity
/// resolution must not fail, it has callers that cannot report.
fn verified(dir: &Path) -> Option<VerifiedIdentity> {
    let config = TeamConfig::load(dir).ok().flatten();
    let preferred = config
        .as_ref()
        .and_then(|config| config.identity_provider.as_ref())
        .map(super::auth::IdentityProvider::key);
    IdentityCache::load()
        .select(preferred.as_deref(), Utc::now())
        .cloned()
}

/// Combine the source values by precedence. Pure — the tested core of
/// [`Identity::resolve`].
fn from_parts(
    env_user: Option<String>,
    verified: Option<VerifiedIdentity>,
    git_name: Option<String>,
    git_email: Option<String>,
    os_user: Option<String>,
) -> Identity {
    if let Some(spec) = env_user {
        return parse_spec(&spec);
    }
    if let Some(verified) = verified {
        return Identity {
            name: verified.display_name().to_string(),
            email: verified.email.clone(),
            provider: Some(verified.provider),
        };
    }
    let email = git_email;
    let name = git_name
        .or(os_user)
        .or_else(|| email.as_deref().map(|e| local_part(e).to_string()))
        .unwrap_or_else(|| "unknown".to_string());
    Identity {
        name,
        email,
        provider: None,
    }
}

/// Parse a `MAGISTR_USER` spec: `Name <email>`, a bare email, or a name.
fn parse_spec(spec: &str) -> Identity {
    let spec = spec.trim();
    if let Some((name, rest)) = spec.split_once('<')
        && let Some(email) = rest.strip_suffix('>')
    {
        let email = email.trim();
        if !email.is_empty() {
            let name = name.trim();
            let name = if name.is_empty() {
                local_part(email).to_string()
            } else {
                name.to_string()
            };
            return Identity {
                name,
                email: Some(email.to_string()),
                provider: None,
            };
        }
    }
    if spec.contains('@') {
        Identity {
            name: local_part(spec).to_string(),
            email: Some(spec.to_string()),
            provider: None,
        }
    } else {
        Identity {
            name: spec.to_string(),
            email: None,
            provider: None,
        }
    }
}

fn local_part(email: &str) -> &str {
    email.split('@').next().unwrap_or(email)
}

/// Read one git config key for `dir`, quietly (no stderr noise when the
/// directory is not a repository or the key is unset).
fn git_config(dir: &Path, key: &str) -> Option<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["config", "--get", key])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let value = String::from_utf8(output.stdout).ok()?;
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_string())
}

#[cfg(test)]
#[path = "identity.tests.rs"]
mod tests;
