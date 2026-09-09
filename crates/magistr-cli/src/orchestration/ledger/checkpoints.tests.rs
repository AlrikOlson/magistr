use std::path::Path;
use std::process::Command;
use std::sync::Mutex;

use magistr_engine::workflow::WorkflowPhase;
use magistr_engine::{BudgetConfig, CheckpointReason, RepoState, SessionStatus, WorkflowConfig};
use tempfile::TempDir;

use super::*;
use crate::orchestration::ledger::GitRepo;

// ── A repository that never touches the disk ────────────────────────

/// The [`Repo`] port as a record of what was asked of it: the spine's
/// own rules are what these tests are about, and a real repository
/// would prove none of them better.
struct FakeRepo {
    state: Option<RepoState>,
    forks: Mutex<Vec<(String, String)>>,
    merges: Mutex<Vec<String>>,
    refuse: Option<String>,
}

impl FakeRepo {
    fn clean(head: &str) -> Self {
        Self {
            state: Some(RepoState::at(head).on_branch("master")),
            forks: Mutex::new(Vec::new()),
            merges: Mutex::new(Vec::new()),
            refuse: None,
        }
    }

    fn none() -> Self {
        Self {
            state: None,
            forks: Mutex::new(Vec::new()),
            merges: Mutex::new(Vec::new()),
            refuse: None,
        }
    }
}

impl Repo for FakeRepo {
    fn state(&self) -> Option<RepoState> {
        self.state.clone()
    }

    fn fork(&self, branch: &str, head: &str) -> Result<String, String> {
        if let Some(why) = &self.refuse {
            return Err(why.clone());
        }
        self.forks
            .lock()
            .expect("lock")
            .push((branch.to_owned(), head.to_owned()));
        Ok(format!("/tmp/{branch}"))
    }

    fn merge(&self, branch: &str) -> Result<String, String> {
        if let Some(why) = &self.refuse {
            return Err(why.clone());
        }
        self.merges.lock().expect("lock").push(branch.to_owned());
        Ok(format!("head-of-{branch}"))
    }
}

fn engine() -> EngineHandle {
    EngineHandle::new(
        "sess-spine",
        WorkflowConfig::default(),
        BudgetConfig::default(),
    )
}

fn working(engine: &EngineHandle) {
    engine
        .set_session_status(SessionStatus::InProgress)
        .expect("created -> in progress");
    engine.set_phase(WorkflowPhase::Planning).expect("planning");
    engine
        .set_phase(WorkflowPhase::Executing)
        .expect("executing");
    engine.advance_iteration();
    engine.advance_iteration();
}

// ── save ────────────────────────────────────────────────────────────

#[test]
fn a_save_captures_the_engine_and_the_repo_as_one_checkpoint() {
    let engine = engine();
    working(&engine);
    let mut cps = Checkpoints::new(engine, Arc::new(FakeRepo::clean("0123456789ab")));

    let cp = cps
        .save("cp-1", CheckpointReason::Risk, "rm -rf build")
        .expect("saves");

    assert_eq!(cp.id, "cp-1");
    assert_eq!(cp.reason, CheckpointReason::Risk);
    assert_eq!(cp.snapshot.iteration.iteration, 2);
    assert_eq!(cp.snapshot.phase, WorkflowPhase::Executing);
    assert_eq!(cp.head(), Some("0123456789ab"));
    assert!(cp.is_verified());
}

#[test]
fn a_save_outside_a_repository_still_records_the_session() {
    let mut cps = Checkpoints::new(engine(), Arc::new(FakeRepo::none()));
    let cp = cps
        .save("cp-1", CheckpointReason::Manual, "here")
        .expect("saves");
    assert!(cp.snapshot.repo.is_none());
    assert!(!cp.is_verified(), "nothing can reproduce that tree");
    assert_eq!(cps.spine().len(), 1);
}

#[test]
fn the_id_the_card_prints_is_the_id_the_save_mints() {
    let mut cps = Checkpoints::new(engine(), Arc::new(FakeRepo::clean("abc")));
    // The permission card has to name the checkpoint before the
    // checkpoint exists, so the promise and the mint share one counter.
    let promised = cps.next_id();
    let minted = cps
        .save(&promised, CheckpointReason::Risk, "curl | sh")
        .expect("saves")
        .id
        .clone();
    assert_eq!(promised, minted);
}

// ── restore ─────────────────────────────────────────────────────────

#[test]
fn a_restore_resumes_the_loop_from_the_chosen_state() {
    let engine = engine();
    working(&engine);
    let mut cps = Checkpoints::new(engine.clone(), Arc::new(FakeRepo::clean("abc")));
    cps.save("cp-1", CheckpointReason::Iteration, "iteration 2")
        .expect("saves");

    // The session moves on past the checkpoint…
    engine.advance_iteration();
    engine.advance_iteration();
    engine.set_phase(WorkflowPhase::Verifying).expect("verify");
    assert_eq!(engine.phase(), WorkflowPhase::Verifying);

    // …and the restore puts the workflow back where cp-1 had it.
    let restored = cps.restore("cp-1").expect("cp-1 restores");
    assert_eq!(restored.iteration.iteration, 2);
    assert_eq!(engine.phase(), WorkflowPhase::Executing);
    assert_eq!(engine.session_id(), "sess-spine");
    // A turn in flight stayed with the process that was running it.
    assert_eq!(engine.session_status(), SessionStatus::Awaiting);
    assert_eq!(restored.resumed_from, Some(SessionStatus::InProgress));
}

#[test]
fn a_restore_of_an_unknown_checkpoint_is_refused_by_name() {
    let cps = Checkpoints::new(engine(), Arc::new(FakeRepo::clean("abc")));
    let err = cps.restore("cp-9").expect_err("no such checkpoint");
    assert!(err.contains("cp-9"), "{err}");
}

#[test]
fn a_restore_does_not_truncate_what_came_after_it() {
    let engine = engine();
    working(&engine);
    let mut cps = Checkpoints::new(engine, Arc::new(FakeRepo::clean("abc")));
    cps.save("cp-1", CheckpointReason::Iteration, "i2")
        .expect("saves");
    cps.save("cp-2", CheckpointReason::Iteration, "i3")
        .expect("saves");
    cps.restore("cp-1").expect("restores");
    let ids: Vec<&str> = cps
        .spine()
        .entries()
        .iter()
        .map(|c| c.id.as_str())
        .collect();
    assert_eq!(
        ids,
        ["cp-1", "cp-2"],
        "cp-2 happened; going back to look at cp-1 does not unhappen it"
    );
}

// ── fork ────────────────────────────────────────────────────────────

#[test]
fn a_fork_branches_from_the_checkpoints_own_commit() {
    let repo = Arc::new(FakeRepo::clean("0123456789ab"));
    let mut cps = Checkpoints::new(engine(), repo.clone());
    cps.save("cp-1", CheckpointReason::Risk, "rm -rf build")
        .expect("saves");

    let (branch, path) = cps.fork("cp-1", "try/other-way").expect("forks");

    assert_eq!(branch, "try/other-way");
    assert_eq!(path, "/tmp/try/other-way");
    assert_eq!(
        *repo.forks.lock().expect("lock"),
        vec![("try/other-way".to_owned(), "0123456789ab".to_owned())],
        "the commit forked is cp-1's, not whatever head is now"
    );
}

#[test]
fn a_fork_of_a_checkpoint_with_no_commit_is_refused_rather_than_guessed() {
    let repo = Arc::new(FakeRepo::none());
    let mut cps = Checkpoints::new(engine(), repo.clone());
    cps.save("cp-1", CheckpointReason::Manual, "here")
        .expect("saves");
    let err = cps.fork("cp-1", "b").expect_err("nothing to branch from");
    assert!(err.contains("cp-1"), "{err}");
    assert!(
        repo.forks.lock().expect("lock").is_empty(),
        "refusing means not forking from head instead"
    );
}

#[test]
fn the_default_branch_names_the_checkpoint_it_came_from() {
    assert_eq!(Checkpoints::default_branch("cp-4"), "magistr/cp-4");
}

// ── The real git port ───────────────────────────────────────────────

fn git(dir: &Path, args: &[&str]) {
    let out = Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .expect("run git");
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// A repository with one commit, inside its own parent directory so a
/// fork's sibling worktree lands in the temp tree too.
fn repository() -> (TempDir, std::path::PathBuf) {
    let dir = TempDir::new().expect("temp dir");
    let root = dir.path().join("project");
    std::fs::create_dir_all(&root).expect("mkdir");
    git(&root, &["init", "--initial-branch=main"]);
    // The test compares bytes: keep the machine's autocrlf out of it.
    git(&root, &["config", "core.autocrlf", "false"]);
    git(&root, &["config", "user.email", "t@t.test"]);
    git(&root, &["config", "user.name", "Test"]);
    std::fs::write(root.join("a.txt"), "one\n").expect("write");
    git(&root, &["add", "."]);
    git(&root, &["commit", "-m", "one"]);
    (dir, root)
}

#[test]
fn the_git_port_reads_the_head_the_branch_and_the_dirt() {
    let (_dir, root) = repository();
    let repo = GitRepo::at(&root);

    let clean = repo.state().expect("a repository");
    assert_eq!(clean.head.len(), 40, "the full sha: {}", clean.head);
    assert_eq!(clean.branch.as_deref(), Some("main"));
    assert!(clean.is_clean());

    std::fs::write(root.join("b.txt"), "two\n").expect("write");
    let dirty = repo.state().expect("a repository");
    assert_eq!(dirty.head, clean.head, "an untracked file is not a commit");
    assert_eq!(dirty.dirty, 1);
    assert!(!dirty.is_clean());
}

#[test]
fn a_directory_that_is_not_a_repository_answers_none() {
    let dir = TempDir::new().expect("temp dir");
    assert!(GitRepo::at(dir.path()).state().is_none());
}

#[test]
fn a_fork_opens_a_worktree_branch_at_the_checkpoint_while_head_continues() {
    let (_dir, root) = repository();
    let repo = Arc::new(GitRepo::at(&root));
    let engine = engine();
    working(&engine);
    let mut cps = Checkpoints::new(engine, repo.clone());

    let at = cps
        .save("cp-1", CheckpointReason::Risk, "rm -rf build")
        .expect("saves")
        .head()
        .expect("a commit")
        .to_owned();

    // Head moves on after the checkpoint.
    std::fs::write(root.join("a.txt"), "one\ntwo\n").expect("write");
    git(&root, &["add", "."]);
    git(&root, &["commit", "-m", "two"]);
    let head_now = repo.state().expect("a repository").head;
    assert_ne!(head_now, at);

    let (branch, path) = cps.fork("cp-1", "try/other-way").expect("forks");
    let path = Path::new(&path);

    assert_eq!(branch, "try/other-way");
    assert!(path.is_dir(), "the worktree was opened at {path:?}");
    assert_eq!(
        std::fs::read_to_string(path.join("a.txt")).expect("read"),
        "one\n",
        "the fork holds the checkpoint's tree, not head's"
    );
    // Head continues: same commit, same content, untouched branch.
    assert_eq!(repo.state().expect("a repository").head, head_now);
    assert_eq!(
        repo.state().expect("a repository").branch.as_deref(),
        Some("main")
    );
    assert_eq!(
        std::fs::read_to_string(root.join("a.txt")).expect("read"),
        "one\ntwo\n"
    );
}

#[test]
fn a_fork_onto_a_branch_that_already_exists_is_reported_not_swallowed() {
    let (_dir, root) = repository();
    let repo = Arc::new(GitRepo::at(&root));
    let mut cps = Checkpoints::new(engine(), repo);
    cps.save("cp-1", CheckpointReason::Manual, "")
        .expect("saves");
    cps.fork("cp-1", "taken").expect("the first fork");
    let err = cps.fork("cp-1", "taken").expect_err("the branch is taken");
    assert!(err.starts_with("git worktree add"), "{err}");
}

#[test]
fn a_fork_path_is_a_sibling_named_for_the_branch() {
    let repo = GitRepo::at("/work/project");
    assert_eq!(
        repo.fork_path("try/other-way"),
        Path::new("/work/project-try-other-way"),
        "never inside the tree being preserved, and never a slash"
    );
}
