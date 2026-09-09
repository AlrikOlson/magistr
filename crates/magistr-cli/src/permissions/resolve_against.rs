use std::path::{Component, Path, PathBuf};

/// Resolve `path` the way the agent's filesystem would, so two spellings
/// of one file compare equal: a relative path is joined to `root` (the
/// session's working directory — `None` leaves it relative), `.` and
/// `..` are folded lexically, and a path that exists is canonicalized so
/// a symlinked root (`/var` → `/private/var` on macOS) agrees with the
/// entry that was recorded through it.
///
/// Lexical first, canonical second is the order that keeps a `Write` to
/// a file that does not exist yet resolvable: `canonicalize` fails on
/// it, and the lexical answer is what the write will create. A `..` on
/// an absolute path never climbs above the root component, and a
/// leading `..` on a relative path with no root is kept as it is —
/// there is nothing to fold it into. A path still relative after all
/// that is never canonicalized: that would resolve it against this
/// process's cwd, which is not the agent's.
#[must_use]
pub fn resolve_against(root: Option<&Path>, path: &Path) -> PathBuf {
    let joined = match root {
        Some(root) if path.is_relative() => root.join(path),
        _ => path.to_path_buf(),
    };
    let lexical = normalize(&joined);
    if lexical.is_relative() {
        // `canonicalize` would resolve this against the *process* cwd,
        // which is not where the agent's relative path points.
        return lexical;
    }
    std::fs::canonicalize(&lexical).unwrap_or(lexical)
}

fn normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                let popped =
                    matches!(out.components().next_back(), Some(Component::Normal(_))) && out.pop();
                if !popped && !out.has_root() {
                    out.push("..");
                }
            }
            other => out.push(other),
        }
    }
    out
}

#[cfg(test)]
#[path = "resolve_against.tests.rs"]
mod tests;
