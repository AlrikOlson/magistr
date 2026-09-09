//! Which formatter runs actually happen for a set of archetypes.

use std::path::{Path, PathBuf};

use magistr_core::types::Archetype;

/// The distinct formatter runs a set of archetypes asks for, and which
/// archetypes each run answers for.
///
/// # Why an archetype is not always its own run
///
/// Two rules, and both were learned from a real project (`shar-remaster`,
/// where 29 of 29 `magistr_format` calls failed):
///
/// 1. **A command identical to the project's own runs at the project root.**
///    An archetype's command normally runs in the archetype's root — that is
///    what an archetype *is*. But a command byte-identical to the top-level
///    `format.command` is not the archetype's formatter at all: it is the
///    project's, copied onto every archetype, and the project's command is
///    written to run at the project root — the single-archetype path is the
///    proof, since it is the same string run from the root and it works.
///    Running such a command from a sub-directory is how
///    `cargo fmt --manifest-path rust/Cargo.toml` becomes "manifest path
///    `rust/Cargo.toml` does not exist", once per archetype, for ever.
///
/// 2. **The same command in the same directory is one run.** Eight identical
///    `cargo fmt`s over one workspace are not eight formatters; they are one
///    formatter raced against itself — and they were spawned in *parallel*,
///    so they were writing the same files at the same time. Deduplicating is
///    not an optimization here, it is the difference between a defined and an
///    undefined result.
///
/// The response shape is unchanged: every archetype still gets its own entry,
/// it is just that archetypes sharing a run share its outcome.
pub(super) struct FormatRuns {
    runs: Vec<FormatRun>,
}

/// One command, in one directory, answering for one or more archetypes.
pub(super) struct FormatRun {
    /// Where the command runs.
    pub(super) dir: PathBuf,
    /// The archetype whose `format` config the command comes from, and every
    /// other archetype this run answers for.
    pub(super) archetypes: Vec<String>,
    /// The format config to run, taken from the first archetype in the group.
    pub(super) format: magistr_core::FormatConfig,
    /// The files this run should format, when the caller named any. `None`
    /// means "everything the command covers".
    pub(super) files: Option<Vec<String>>,
}

impl FormatRuns {
    /// Plan the runs for `archetypes` under `root`, where `project_command`
    /// is the top-level `format.command`.
    ///
    /// `files`, when given, is split by archetype root: a run at the project
    /// root takes every file, a run inside an archetype takes the files under
    /// that archetype, and an archetype with none of them is not run at all —
    /// formatting `web/app.ts` must not invoke the Rust formatter.
    pub(super) fn plan(
        root: &Path,
        project_command: &str,
        archetypes: &[Archetype],
        files: Option<&[&str]>,
    ) -> Self {
        let mut runs: Vec<FormatRun> = Vec::new();
        for arch in archetypes {
            // Rule 1: the project's own command runs where it was written to.
            let at_root = arch.format.command == project_command;
            let dir = if at_root {
                root.to_owned()
            } else {
                root.join(&arch.root)
            };
            let files = files.map(|files| {
                if at_root {
                    files.iter().map(|f| (*f).to_owned()).collect()
                } else {
                    files
                        .iter()
                        .filter(|f| under(Path::new(f), &arch.root))
                        .map(|f| (*f).to_owned())
                        .collect()
                }
            });
            // Rule 2: the same command in the same directory is one run.
            if let Some(existing) = runs
                .iter_mut()
                .find(|r| r.dir == dir && r.format.command == arch.format.command)
            {
                existing.archetypes.push(arch.id.clone());
                continue;
            }
            runs.push(FormatRun {
                dir,
                archetypes: vec![arch.id.clone()],
                format: arch.format.clone(),
                files,
            });
        }
        Self { runs }
    }

    /// The planned runs, in the order their first archetype was declared.
    pub(super) fn runs(&self) -> &[FormatRun] {
        &self.runs
    }
}

impl FormatRun {
    /// Whether this run has nothing to do: the caller named files and none of
    /// them are this archetype's.
    pub(super) fn is_empty(&self) -> bool {
        self.files.as_ref().is_some_and(Vec::is_empty)
    }
}

/// Whether `path` is inside `root`, with `.` and an empty root meaning the
/// whole project. Both are project-relative, so this is a prefix question and
/// never a filesystem one — a file the caller named need not exist yet.
fn under(path: &Path, root: &Path) -> bool {
    if root.as_os_str().is_empty() || root == Path::new(".") {
        return true;
    }
    path.starts_with(root)
}

#[cfg(test)]
#[path = "format_runs.tests.rs"]
mod tests;
