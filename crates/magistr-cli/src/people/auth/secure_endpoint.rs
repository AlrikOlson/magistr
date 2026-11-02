//! Validate transport trust anchors before any request is sent.
use crate::error::MagistrError;

/// Require an absolute HTTPS URI without embedded credentials or fragments.
pub(super) fn secure_endpoint(value: &str, issuer: bool) -> Result<(), MagistrError> {
    let uri = value.parse::<ureq::http::Uri>().map_err(|_| {
        MagistrError::Other("identity provider endpoint must be an absolute HTTPS URL".into())
    })?;
    if uri.scheme_str() != Some("https")
        || uri.host().is_none_or(str::is_empty)
        || uri.authority().is_some_and(|a| a.as_str().contains('@'))
        || value.contains('#')
        || (issuer && uri.query().is_some())
    {
        return Err(MagistrError::Other(
            "identity provider endpoint must use HTTPS without credentials or fragments; issuer must not have a query".into(),
        ));
    }
    Ok(())
}
#[cfg(test)]
#[path = "secure_endpoint.tests.rs"]
mod tests;
