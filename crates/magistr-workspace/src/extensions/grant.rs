/// A grant authorizes exactly the declared interface and version.
#[must_use]
pub fn grant_covers(grant: &str, import: &str) -> bool {
    grant == import
}

#[cfg(test)]
#[path = "grant.tests.rs"]
mod tests;
