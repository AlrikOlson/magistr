//! Bounded, non-redirecting HTTPS transport for identity providers.
use super::secure_endpoint::secure_endpoint;
use crate::error::MagistrError;
use serde_json::Value;
use std::time::Duration;

const MAX_BODY: u64 = 1024 * 1024;
const MAX_CACHE_AGE: Duration = Duration::from_secs(300);

pub(super) struct AuthHttp {
    agent: ureq::Agent,
}

impl AuthHttp {
    pub(super) fn new() -> Self {
        let config = ureq::Agent::config_builder()
            .https_only(true)
            .max_redirects(0)
            .http_status_as_error(false)
            .timeout_global(Some(Duration::from_secs(30)))
            .build();
        Self {
            agent: ureq::Agent::new_with_config(config),
        }
    }

    #[cfg(test)]
    pub(super) fn with_agent(agent: ureq::Agent) -> Self {
        Self { agent }
    }

    pub(super) fn get_json(&self, url: &str) -> Result<(Value, Duration), MagistrError> {
        secure_endpoint(url, false)?;
        let response = self
            .agent
            .get(url)
            .header("Accept", "application/json")
            .call()
            .map_err(|_| MagistrError::Other("identity provider HTTPS request failed".into()))?;
        read_json(response, false)
    }

    pub(super) fn post_form(
        &self,
        url: &str,
        form: &[(&str, &str)],
    ) -> Result<Value, MagistrError> {
        secure_endpoint(url, false)?;
        let response = self
            .agent
            .post(url)
            .header("Accept", "application/json")
            .send_form(form.iter().copied())
            .map_err(|_| MagistrError::Other("identity provider HTTPS request failed".into()))?;
        read_json(response, true).map(|(value, _)| value)
    }

    pub(super) fn github_user(&self, access_token: &str) -> Result<Value, MagistrError> {
        let response = self
            .agent
            .get(super::identity_provider::GITHUB_USER_URL)
            .header("Accept", "application/vnd.github+json")
            .header("Authorization", &format!("Bearer {access_token}"))
            .header("User-Agent", "magistr")
            .call()
            .map_err(|_| MagistrError::Other("GitHub user request failed".into()))?;
        read_json(response, false).map(|(value, _)| value)
    }
}

fn read_json(
    mut response: ureq::http::Response<ureq::Body>,
    allow_oauth_error: bool,
) -> Result<(Value, Duration), MagistrError> {
    let status = response.status();
    if !(status.is_success() || allow_oauth_error && status.as_u16() == 400) {
        return Err(MagistrError::Other(format!(
            "identity provider returned HTTP {}",
            status.as_u16()
        )));
    }
    let ttl = cache_age(response.headers());
    let bytes = response
        .body_mut()
        .with_config()
        .limit(MAX_BODY + 1)
        .read_to_vec()
        .map_err(|_| {
            MagistrError::Other(
                "identity provider response was unreadable or exceeded 1 MiB".into(),
            )
        })?;
    if bytes.len() > MAX_BODY as usize {
        return Err(MagistrError::Other(
            "identity provider response exceeded 1 MiB".into(),
        ));
    }
    let value: Value = serde_json::from_slice(&bytes)
        .map_err(|_| MagistrError::Other("identity provider returned malformed JSON".into()))?;
    if !value.is_object() {
        return Err(MagistrError::Other(
            "identity provider response must be a JSON object".into(),
        ));
    }
    if !status.is_success() && value.get("error").and_then(Value::as_str).is_none() {
        return Err(MagistrError::Other(
            "identity provider returned HTTP 400 without an OAuth error".into(),
        ));
    }
    Ok((value, ttl))
}

fn cache_age(headers: &ureq::http::HeaderMap) -> Duration {
    let mut seconds = MAX_CACHE_AGE.as_secs();
    for header in headers.get_all("cache-control") {
        let Ok(value) = header.to_str() else {
            return Duration::ZERO;
        };
        for directive in value.split(',').map(str::trim) {
            if directive.eq_ignore_ascii_case("no-store")
                || directive.eq_ignore_ascii_case("no-cache")
            {
                return Duration::ZERO;
            }
            if let Some((name, value)) = directive.split_once('=')
                && name.trim().eq_ignore_ascii_case("max-age")
            {
                let Ok(age) = value.trim().trim_matches('"').parse::<u64>() else {
                    return Duration::ZERO;
                };
                seconds = seconds.min(age);
            }
        }
    }
    if let Some(age) = headers.get("age") {
        let age = age
            .to_str()
            .ok()
            .and_then(|value| value.parse::<u64>().ok());
        seconds = seconds.saturating_sub(age.unwrap_or(u64::MAX));
    }
    Duration::from_secs(seconds)
}

#[cfg(test)]
#[path = "auth_http.tests.rs"]
mod tests;
