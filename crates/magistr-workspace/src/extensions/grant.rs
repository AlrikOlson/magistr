/// Does a recorded grant cover a manifest import? Exact match, or the same
/// `package/interface@major` — the additive-within-major rule read on the
/// grant side, so a record granted under `…@0.1.0` still authorizes the
/// rebuilt plugin's `…@0.2.0` import. Nothing crosses a major.
#[must_use]
pub fn grant_covers(grant: &str, import: &str) -> bool {
    if grant == import {
        return true;
    }
    match (split_major(grant), split_major(import)) {
        (Some((g, g_major)), Some((i, i_major))) => g == i && g_major == i_major,
        _ => false,
    }
}

fn split_major(name: &str) -> Option<(&str, u64)> {
    let (base, version) = name.rsplit_once('@')?;
    let mut parts = version.split('.');
    let major = parts.next()?;
    let minor = parts.next()?;
    let patch = parts.next()?;
    if parts.next().is_some()
        || [major, minor, patch]
            .iter()
            .any(|part| part.is_empty() || !part.bytes().all(|b| b.is_ascii_digit()))
    {
        return None;
    }
    Some((base, major.parse().ok()?))
}

#[cfg(test)]
#[path = "grant.tests.rs"]
mod tests;
