//! `update` and `view` are pure: no file under
//! `src/cockpit` or `src/turn` (their tests aside) may reach a clock, the
//! file system, a process, the environment, a thread, a database or
//! stdout. The runtime owns every effect and hands the core what it needs
//! as messages and the frame clock.

use std::path::{Path, PathBuf};

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs")
            && !path.to_string_lossy().ends_with(".tests.rs")
        {
            out.push(path);
        }
    }
}

#[test]
fn the_core_reaches_no_effect() {
    let files = core_files();
    assert!(files.len() >= 16, "walked only {} files", files.len());
    let forbidden = [
        "Instant::now",
        "SystemTime",
        "std::fs",
        "std::process",
        "std::env",
        "std::thread",
        // Not `std::io` wholesale: `io::Cursor` is an in-memory adapter
        // with no effect behind it, and the image decoders take a reader
        // rather than a slice. What the ban is actually for is the three
        // standard streams — a file reached through `io::Read` is already
        // caught by `std::fs`.
        "io::stdout",
        "io::stdin",
        "io::stderr",
        "std::net",
        "rusqlite",
        "tokio",
        "println!",
        "eprintln!",
        "print!",
        "unsafe",
        "from_os",
        "Command::new",
        "block_on",
    ];
    let mut offenders = Vec::new();
    for file in &files {
        let source = std::fs::read_to_string(file).expect("read source");
        for (idx, line) in source.lines().enumerate() {
            for token in forbidden {
                if line.contains(token) {
                    offenders.push(format!("{}:{}: {token}", file.display(), idx + 1));
                }
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "effects in the core:\n{}",
        offenders.join("\n")
    );
}

#[test]
fn update_and_view_are_the_only_entry_points() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/cockpit");
    for name in ["update.rs", "view.rs", "model.rs", "msg.rs", "cmd.rs"] {
        assert!(root.join(name).is_file(), "src/cockpit/{name} missing");
    }
    let update = std::fs::read_to_string(root.join("update.rs")).expect("update.rs");
    let view = std::fs::read_to_string(root.join("view.rs")).expect("view.rs");
    assert!(update.contains("pub fn update(model: &mut Model, msg: Msg) -> Vec<Cmd>"));
    assert!(view.contains("pub fn view(model: &Model, clock: &FrameClock) -> Node"));
}

/// The producers' modules run effects — a blocking pool, a
/// socket, the store — but none of them spawns a process, opens a
/// database or writes files itself, and the core never reaches their
/// effectful handles: `update` and `view` see only the event types.
#[test]
fn the_producers_run_no_process_no_database_and_write_no_files() {
    let src = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    for module in ["job", "engine", "agent"] {
        walk(&src.join(module), &mut files);
    }
    assert!(files.len() >= 12, "walked only {} files", files.len());
    let forbidden = [
        "std::process",
        "Command::new",
        "rusqlite",
        "std::fs::write",
        "File::create",
        "OpenOptions",
        "println!",
        "eprintln!",
        "print!",
        "unsafe",
    ];
    let offenders = offenders(&files, &forbidden);
    assert!(
        offenders.is_empty(),
        "effects in the producers:\n{}",
        offenders.join("\n")
    );
}

#[test]
fn the_core_never_names_an_effectful_producer_handle() {
    let files = core_files();
    let forbidden = [
        "EngineClient",
        "NativeEngines",
        "TasConfig",
        "spawn_feed",
        "spawn_session",
        "Commands",
        "AcpTransport",
        "AcpEvent",
        "Jobs",
        "spawn_job",
        "spawn_blocking",
        "Inbox",
        "UnixStream",
        // The loop driver (the root crate's src/orchestration/loop_driver)
        // reports to the core as LoopEvent messages and takes LoopCmd
        // decisions back; none of its machinery is named here.
        "LoopDriver",
        "IterationLoop",
        "WorkflowConfig",
        "Connector",
        "connect_transport",
        "run_command",
    ];
    let offenders = offenders(&files, &forbidden);
    assert!(
        offenders.is_empty(),
        "producer handles in the core:\n{}",
        offenders.join("\n")
    );
}

/// The theme is a value: its one read of the process environment is
/// `Theme::from_os` (theme/decision.rs), and only the entry points call it.
#[test]
fn the_theme_reads_the_environment_in_one_place() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/theme");
    let mut files = Vec::new();
    walk(&root, &mut files);
    assert!(files.len() >= 4, "walked only {} files", files.len());
    let elsewhere: Vec<PathBuf> = files
        .iter()
        .filter(|f| f.file_name().is_some_and(|n| n != "decision.rs"))
        .cloned()
        .collect();
    let offenders = offenders(&elsewhere, &["std::env", "std::io", "println!", "unsafe"]);
    assert!(
        offenders.is_empty(),
        "effects in the theme:\n{}",
        offenders.join("\n")
    );
    let theme = std::fs::read_to_string(root.join("decision.rs")).expect("decision.rs");
    assert_eq!(
        theme.matches("std::env").count(),
        1,
        "exactly one environment read, in from_os"
    );
}

/// The core: the cockpit and the turn model it shows.
fn core_files() -> Vec<PathBuf> {
    let src = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    for module in ["cockpit", "turn"] {
        walk(&src.join(module), &mut files);
    }
    files
}

fn offenders(files: &[PathBuf], forbidden: &[&str]) -> Vec<String> {
    let mut out = Vec::new();
    for file in files {
        let source = std::fs::read_to_string(file).expect("read source");
        for (idx, line) in source.lines().enumerate() {
            for token in forbidden {
                if line.contains(token) {
                    out.push(format!("{}:{}: {token}", file.display(), idx + 1));
                }
            }
        }
    }
    out
}
