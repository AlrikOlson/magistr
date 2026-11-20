//! Which environment variables a check's result is bound to.
//!
//! A result is reused while the tools that produced it would act the same.
//! The whole process environment does not say that: a host gives each session
//! its own identifiers, sockets and tokens, so a digest of every variable
//! differs in every session and no earlier result can be reused or counted
//! toward completing a task. The bound set is the variables that choose a
//! command's tools or steer them, by the tools the command uses, plus the
//! names the project declares in `verification.env`.

use std::ffi::OsString;

/// Variables that choose or steer any command. A trailing `*` is a prefix.
const ALWAYS: &[&str] = &[
    "PATH",
    "PATHEXT",
    "HOME",
    "USERPROFILE",
    "LANG",
    "LC_*",
    "TZ",
    "CI",
];

/// Variables read by one tool, keyed by its name in the command's profile.
const BY_TOOL: &[(&str, &[&str])] = &[
    (
        "cargo",
        &[
            "CARGO_*",
            "RUSTC*",
            "RUSTDOC*",
            "RUSTFLAGS",
            "RUSTUP_*",
            "RUST_*",
            "CC",
            "CC_*",
            "CXX",
            "CXX_*",
            "AR",
            "AR_*",
            "CFLAGS",
            "CFLAGS_*",
            "CXXFLAGS",
            "CXXFLAGS_*",
            "LDFLAGS",
        ],
    ),
    ("cargo-nextest", &["NEXTEST*"]),
    ("node", &["NODE_*"]),
    ("npm", &["NPM_CONFIG_*"]),
    (
        "python",
        &["PYTHON*", "VIRTUAL_ENV", "CONDA_PREFIX", "PIP_*"],
    ),
    ("git", &["GIT_*"]),
];

/// The variables of `variables` a result is bound to, sorted by name: the
/// ones every command reads, the ones the tools in `profile` read and the
/// ones `declared` names. Names compare without regard to case, as Windows
/// resolves them, and are returned in upper case with their values.
pub(super) fn bound(
    variables: impl IntoIterator<Item = (OsString, OsString)>,
    profile: &str,
    declared: &[String],
) -> Vec<(String, Vec<u8>)> {
    let tools: Vec<&str> = profile.split_whitespace().collect();
    let patterns: Vec<String> = ALWAYS
        .iter()
        .chain(
            BY_TOOL
                .iter()
                .filter(|(tool, _)| tools.contains(tool))
                .flat_map(|(_, patterns)| patterns.iter()),
        )
        .map(|pattern| (*pattern).to_owned())
        .chain(declared.iter().map(|pattern| pattern.to_ascii_uppercase()))
        .collect();
    let mut bound: Vec<_> = variables
        .into_iter()
        .map(|(name, value)| {
            (
                name.to_string_lossy().to_ascii_uppercase(),
                value.as_encoded_bytes().to_vec(),
            )
        })
        .filter(|(name, _)| patterns.iter().any(|pattern| is_match(pattern, name)))
        .collect();
    bound.sort();
    bound
}

fn is_match(pattern: &str, name: &str) -> bool {
    match pattern.strip_suffix('*') {
        Some(prefix) => name.starts_with(prefix),
        None => name == pattern,
    }
}

#[cfg(test)]
#[path = "execution_environment.tests.rs"]
mod tests;
