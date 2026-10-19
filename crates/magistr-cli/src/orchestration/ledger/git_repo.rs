//! The [`Repo`] port over the git CLI.

use std::path::{Path, PathBuf};
use std::process::Command;

use magistr_engine::RepoState;

use super::repo::Repo;

/// The git working tree a session runs in, read and forked through the
/// `git` binary.
///
/// The CLI rather than a library for the reason `.cargo/config.toml`
/// already sets `git-fetch-with-cli`: it is the git the user's own
/// configuration, credential helpers and hooks are wired into, and this
/// crate already shells out to it everywhere else it needs one.
///
/// A fork is `git worktree add -b <branch> <path> <head>`. That verb is
/// the whole design: it materialises the checkpoint's commit in a
/// *second* directory on a *new* branch, so the session's own tree —
/// head — is not touched, not stashed and not checked out from under a
/// running agent.
pub struct GitRepo {
    root: PathBuf,
}

impl GitRepo {
    /// The repository the session at `root` is running in.
    #[must_use]
    pub fn at(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// Where a fork onto `branch` is opened: a sibling of the
    /// repository named for the branch, so two forks never collide and
    /// nothing is ever written inside the tree being preserved.
    #[must_use]
    pub fn fork_path(&self, branch: &str) -> PathBuf {
        let name = self.root.file_name().map_or_else(
            || "magistr".to_owned(),
            |n| n.to_string_lossy().into_owned(),
        );
        let safe: String = branch
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
            .collect();
        self.root
            .parent()
            .unwrap_or(Path::new("."))
            .join(format!("{name}-{safe}"))
    }

    /// Git's registered working copies, including locked and unavailable paths.
    pub fn working_copies(&self) -> Result<String, String> {
        self.run(&["worktree", "list", "--porcelain", "-z"])
    }

    pub fn create_working_copy(&self, path: &Path, branch: &str, head: &str) -> Result<(), String> {
        let path = path
            .to_str()
            .ok_or_else(|| "The working-copy path must be valid text.".to_owned())?;
        self.run(&["worktree", "add", "-b", branch, "--", path, head])
            .map(|_| ())
    }

    pub fn archive_working_copy(&self, path: &Path) -> Result<(), String> {
        let path = path
            .to_str()
            .ok_or_else(|| "The working-copy path must be valid text.".to_owned())?;
        self.run(&[
            "worktree",
            "lock",
            "--reason",
            magistr_engine::native::ProjectLocation::ARCHIVE_REASON,
            "--",
            path,
        ])
        .map(|_| ())
    }

    pub fn reopen_working_copy(&self, path: &Path) -> Result<(), String> {
        let path = path
            .to_str()
            .ok_or_else(|| "The working-copy path must be valid text.".to_owned())?;
        self.run(&["worktree", "unlock", "--", path]).map(|_| ())
    }

    pub fn changes(&self) -> Result<(u32, u32), String> {
        // Runtime coordination and journal files are owned state, not source changes.
        // Project configuration remains part of the status.
        let status = self.run(&[
            "status",
            "--porcelain=v2",
            "--untracked-files=all",
            "--",
            ".",
            ":(exclude).magistr/workspace/**",
            ":(exclude).magistr/working-copy.lock",
            ":(exclude).magistr/workflow-owner.lock",
            ":(exclude).magistr/project-owner.lock",
        ])?;
        let dirty = status.lines().filter(|line| !line.is_empty()).count() as u32;
        let conflicts = status.lines().filter(|line| line.starts_with("u ")).count() as u32;
        Ok((dirty, conflicts))
    }

    fn run(&self, args: &[&str]) -> Result<String, String> {
        let output = Command::new("git")
            .args(args)
            .current_dir(&self.root)
            .output()
            .map_err(|e| format!("git {}: {e}", args.join(" ")))?;
        if !output.status.success() {
            return Err(format!(
                "git {}: {}",
                args.join(" "),
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
    }
}

impl Repo for GitRepo {
    /// Read the commit, the branch and the dirt in **one** invocation.
    ///
    /// `git status --porcelain=v2 --branch` answers all three, which is
    /// two things at once. It is *correct*: three separate calls read
    /// three moments, so a tree changing under them could report a sha
    /// from before an edit with a dirt count from after it, and a
    /// checkpoint is exactly the record that must not straddle a
    /// change. And it is *cheap*: a checkpoint runs on the session's
    /// own blocking thread at every iteration boundary and before every
    /// risky permission, so its cost is on the path of a turn — three
    /// subprocesses there was measurable enough to make the loop
    /// driver's deadline-driven goal test flake.
    fn state(&self) -> Option<RepoState> {
        let out = self.run(&["status", "--porcelain=v2", "--branch"]).ok()?;
        let mut head = None;
        let mut branch = None;
        let mut dirty = 0u32;
        for line in out.lines() {
            if let Some(oid) = line.strip_prefix("# branch.oid ") {
                // An unborn branch answers `(initial)`, which is not a
                // commit — a checkpoint there has nothing to fork from
                // and says so rather than recording the word.
                if oid != "(initial)" {
                    head = Some(oid.to_owned());
                }
            } else if let Some(name) = line.strip_prefix("# branch.head ") {
                // A detached head answers `(detached)`, which is not a
                // branch name; recording it would make the snapshot
                // claim a branch that cannot be checked out.
                if name != "(detached)" && !name.is_empty() {
                    branch = Some(name.to_owned());
                }
            } else if !line.starts_with('#') && !line.is_empty() {
                dirty = dirty.saturating_add(1);
            }
        }
        let mut state = RepoState::at(head?).with_dirty(dirty);
        if let Some(name) = branch {
            state = state.on_branch(name);
        }
        Some(state)
    }

    fn fork(&self, branch: &str, head: &str) -> Result<String, String> {
        let path = self.fork_path(branch);
        crate::application::WorkingCopies::at(&self.root)
            .map_err(|error| error.to_string())?
            .create(&path, branch, head)
            .map_err(|error| error.to_string())?;
        Ok(path.to_string_lossy().into_owned())
    }

    /// `git merge --no-ff --no-edit <branch>`, then the head it landed
    /// on.
    ///
    /// `--no-ff` so an approved review is one commit a reader can point
    /// at even when the branch was strictly ahead — the merge is the
    /// record of the approval, and a fast-forward would leave no record
    /// at all. `--no-edit` because there is no editor on a session's
    /// blocking thread. A conflict, a dirty tree or a branch that does
    /// not exist all leave git's own message as the refusal.
    fn merge(&self, branch: &str) -> Result<String, String> {
        self.run(&["merge", "--no-ff", "--no-edit", branch])?;
        self.run(&["rev-parse", "HEAD"])
    }
}
