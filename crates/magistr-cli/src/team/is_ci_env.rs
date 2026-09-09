//! CI environment detection for role resolution.

/// Whether a CI environment is active (`CI` set to a non-empty,
/// non-false value) — feeds the `Ci` role fallback in role resolution.
#[must_use]
pub fn is_ci_env() -> bool {
    std::env::var("CI").is_ok_and(|v| {
        let v = v.trim();
        !v.is_empty() && v != "0" && !v.eq_ignore_ascii_case("false")
    })
}
