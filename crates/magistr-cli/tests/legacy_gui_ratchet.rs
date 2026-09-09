//! The legacy-GUI ratchet (`scripts/legacy-gui-ratchet.sh`, the `legacy-gui`
//! quality gate): the tree named by `.magistr/legacy-gui-ratchet.json` may
//! only shrink.
//!
//! The measure is what git tracks, not what is on disk — `crates/magistr-gui`
//! carries node_modules, dist/ and a generated `src/lib/bindings.ts`, so a
//! filesystem count would move with your last build. These tests therefore
//! run the real script against a temp *git repository* via
//! `RATCHET_PROJECT_ROOT`.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use tempfile::TempDir;

fn script() -> PathBuf {
    Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../..")).join("scripts/legacy-gui-ratchet.sh")
}

fn write(dir: &Path, rel: &str, content: &str) {
    let path = dir.join(rel);
    std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
    std::fs::write(path, content).expect("write");
}

fn git(dir: &Path, args: &[&str]) {
    let out = Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .expect("git");
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// Stage everything so `git ls-files` / `git grep` can see it. The ratchet
/// never needs a commit — it measures the tracked working tree.
fn track(dir: &Path) {
    git(dir, &["add", "-A"]);
}

/// The script the way the gate runs it: through `sh -c`, which follows
/// its shebang to bash on every platform (on Windows, Git's).
fn shell(args: &[&str]) -> Command {
    let script = script().to_string_lossy().replace('\\', "/");
    let line = std::iter::once(script.as_str())
        .chain(args.iter().copied())
        .map(|word| format!("'{word}'"))
        .collect::<Vec<_>>()
        .join(" ");
    let mut cmd = Command::new("sh");
    cmd.arg("-c").arg(line);
    cmd
}

fn run(root: &Path, args: &[&str]) -> Output {
    shell(args)
        .env("RATCHET_PROJECT_ROOT", root)
        .output()
        .expect("run legacy-gui-ratchet.sh")
}

fn text(out: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
}

/// A repo whose frozen tree holds one 3-line file and a matching baseline.
/// The declaration itself lives outside the frozen tree, so it never counts.
fn frozen_project() -> TempDir {
    let tmp = TempDir::new().expect("tempdir");
    git(tmp.path(), &["init", "-q"]);
    write(
        tmp.path(),
        "crates/legacy/a.ts",
        "export const a = 1;\n\nexport const b = 2;\n",
    );
    write(
        tmp.path(),
        ".magistr/legacy-gui-ratchet.json",
        r#"{"path": "crates/legacy", "files": 1, "loc": 3, "unlock": ["legacy-gui", "delete"]}"#,
    );
    track(tmp.path());
    tmp
}

#[test]
fn the_ratchet_passes_at_the_baseline() {
    let tmp = frozen_project();
    let out = run(tmp.path(), &[]);
    assert!(out.status.success(), "{}", text(&out));
    assert!(text(&out).contains("crates/legacy"), "{}", text(&out));
}

#[test]
fn the_ratchet_fails_when_a_file_is_added_under_the_frozen_tree() {
    let tmp = frozen_project();
    write(tmp.path(), "crates/legacy/b.svelte", "<div />\n");
    track(tmp.path());
    let out = run(tmp.path(), &[]);
    assert!(!out.status.success(), "{}", text(&out));
    let msg = text(&out);
    assert!(msg.contains("files"), "{msg}");
}

#[test]
fn the_ratchet_fails_when_lines_grow_without_a_new_file() {
    let tmp = frozen_project();
    write(
        tmp.path(),
        "crates/legacy/a.ts",
        "export const a = 1;\n\nexport const b = 2;\n\nexport const c = 3;\n",
    );
    track(tmp.path());
    let out = run(tmp.path(), &[]);
    assert!(!out.status.success(), "{}", text(&out));
    let msg = text(&out);
    assert!(msg.contains("loc") || msg.contains("lines"), "{msg}");
}

/// The reason the measure is git and not the filesystem: an untracked build
/// artifact under the frozen tree must not move either number.
#[test]
fn untracked_build_output_under_the_frozen_tree_is_not_counted() {
    let tmp = frozen_project();
    write(tmp.path(), ".gitignore", "node_modules/\nbindings.ts\n");
    write(
        tmp.path(),
        "crates/legacy/node_modules/dep/index.js",
        "module.exports = {};\nvar filler = 1;\nvar more = 2;\n",
    );
    write(tmp.path(), "crates/legacy/bindings.ts", "// generated\n");
    track(tmp.path());
    let out = run(tmp.path(), &[]);
    assert!(out.status.success(), "{}", text(&out));
    // .gitignore is tracked but sits outside the frozen tree.
    assert!(text(&out).contains("files=1/1"), "{}", text(&out));
}

/// Lines are counted across every tracked *text* file, not just Rust — this
/// tree is mostly Svelte and TypeScript — and a binary blob contributes none.
#[test]
fn lines_are_counted_across_languages_and_binaries_are_skipped() {
    let tmp = TempDir::new().expect("tempdir");
    git(tmp.path(), &["init", "-q"]);
    write(
        tmp.path(),
        "crates/legacy/a.svelte",
        "<script>\n</script>\n",
    );
    write(tmp.path(), "crates/legacy/b.wgsl", "@vertex\n");
    write(tmp.path(), "crates/legacy/c.rs", "pub struct C;\n");
    std::fs::write(
        tmp.path().join("crates/legacy/icon.icns"),
        [0u8, 1, 0, 2, b'\n', 0, 3, b'\n', 0],
    )
    .expect("write binary");
    write(
        tmp.path(),
        ".magistr/legacy-gui-ratchet.json",
        r#"{"path": "crates/legacy", "files": 4, "loc": 4, "unlock": ["legacy-gui"]}"#,
    );
    track(tmp.path());
    let out = run(tmp.path(), &[]);
    assert!(out.status.success(), "{}", text(&out));
    assert!(text(&out).contains("loc=4/4"), "{}", text(&out));
}

#[test]
fn update_refuses_to_raise_the_baseline() {
    let tmp = frozen_project();
    write(tmp.path(), "crates/legacy/b.svelte", "<div />\n");
    track(tmp.path());
    let out = run(tmp.path(), &["--update"]);
    assert!(!out.status.success(), "{}", text(&out));
    let baseline =
        std::fs::read_to_string(tmp.path().join(".magistr/legacy-gui-ratchet.json")).expect("read");
    assert!(
        baseline.contains("\"files\": 1"),
        "baseline untouched: {baseline}"
    );
}

#[test]
fn update_records_a_lower_baseline_and_keeps_the_declaration() {
    let tmp = frozen_project();
    write(tmp.path(), "crates/legacy/a.ts", "export const a = 1;\n");
    track(tmp.path());
    let out = run(tmp.path(), &["--update"]);
    assert!(out.status.success(), "{}", text(&out));
    let baseline =
        std::fs::read_to_string(tmp.path().join(".magistr/legacy-gui-ratchet.json")).expect("read");
    assert!(baseline.contains("\"files\": 1"), "{baseline}");
    assert!(baseline.contains("\"loc\": 1"), "{baseline}");
    assert!(
        baseline.contains("\"path\": \"crates/legacy\""),
        "{baseline}"
    );
    assert!(baseline.contains("\"legacy-gui\""), "{baseline}");
    // And the gate is green again at the new baseline.
    let out = run(tmp.path(), &[]);
    assert!(out.status.success(), "{}", text(&out));
}

/// `--update` preserves every declared field, including the ones only the
/// write guard reads (`unlock`, `replacement`).
#[test]
fn update_preserves_the_replacement_note() {
    let tmp = TempDir::new().expect("tempdir");
    git(tmp.path(), &["init", "-q"]);
    write(tmp.path(), "crates/legacy/a.ts", "export const a = 1;\n");
    write(
        tmp.path(),
        ".magistr/legacy-gui-ratchet.json",
        r#"{"path": "crates/legacy", "files": 9, "loc": 9, "unlock": ["legacy-gui", "delete"], "replacement": "the native GUI (gui2-1..4)"}"#,
    );
    track(tmp.path());
    let out = run(tmp.path(), &["--update"]);
    assert!(out.status.success(), "{}", text(&out));
    let baseline =
        std::fs::read_to_string(tmp.path().join(".magistr/legacy-gui-ratchet.json")).expect("read");
    assert!(
        baseline.contains("the native GUI (gui2-1..4)"),
        "{baseline}"
    );
    assert!(baseline.contains("\"files\": 1"), "{baseline}");
}

#[test]
fn projects_without_a_declaration_are_not_gated() {
    let tmp = TempDir::new().expect("tempdir");
    git(tmp.path(), &["init", "-q"]);
    write(tmp.path(), "crates/legacy/a.ts", "export const a = 1;\n");
    track(tmp.path());
    let out = run(tmp.path(), &[]);
    assert!(out.status.success(), "{}", text(&out));
}

/// A declared freeze the gate cannot measure is a failure, not a pass — the
/// one place this ratchet is stricter than the src/tui one it descends from.
#[test]
fn a_declaration_outside_a_git_work_tree_fails_loudly() {
    let tmp = TempDir::new().expect("tempdir");
    write(tmp.path(), "crates/legacy/a.ts", "export const a = 1;\n");
    write(
        tmp.path(),
        ".magistr/legacy-gui-ratchet.json",
        r#"{"path": "crates/legacy", "files": 1, "loc": 1}"#,
    );
    let out = run(tmp.path(), &[]);
    assert!(!out.status.success(), "{}", text(&out));
    assert!(text(&out).contains("git"), "{}", text(&out));
}

/// The gate this repo actually runs: the checked-in declaration names the
/// Tauri GUI and its recorded baseline is the tree's current size.
#[test]
fn this_repo_freezes_the_tauri_gui_at_its_recorded_baseline() {
    let root = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."));
    let declared = std::fs::read_to_string(root.join(".magistr/legacy-gui-ratchet.json"))
        .expect("the project declares a frozen GUI tree");
    assert!(
        declared.contains("crates/magistr-gui"),
        "the frozen tree is the Tauri GUI: {declared}"
    );
    assert!(declared.contains("legacy-gui"), "{declared}");
    assert!(declared.contains("delete"), "{declared}");
    let out = shell(&[]).output().expect("run legacy-gui-ratchet.sh");
    assert!(out.status.success(), "{}", text(&out));
}
