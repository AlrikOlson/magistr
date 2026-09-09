//! The window declares what it can show; it never impersonates a terminal. No file
//! under `crates/magistr-gui2` may reach magistr-tui — the manifest has no such
//! dependency, and the source must not spell the path either — and none may fold a
//! terminal protocol into `Msg::Mode`: the pixels this surface has are declared through
//! `magistr_cockpit::capability::Mode`, which has no protocol to name.

use std::path::{Path, PathBuf};

fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if path.file_name().is_some_and(|n| n == "target") {
                continue;
            }
            walk(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs" || e == "toml") {
            out.push(path);
        }
    }
}

/// Spelled so this file does not itself trip the check.
fn forbidden() -> Vec<String> {
    [
        ["magistr", "_tui"],
        ["magistr", "-tui = "],
        ["Protocol::", "Kitty"],
        ["Protocol::", "Sixel"],
        ["Passthrough", "::"],
        ["Mode::Graphics", "("],
    ]
    .into_iter()
    .map(|pair| pair.concat())
    .collect()
}

#[test]
fn the_window_never_names_the_terminal_crate_or_a_terminal_protocol() {
    let mut files = Vec::new();
    walk(&crate_root(), &mut files);
    assert!(files.len() >= 40, "walked only {} files", files.len());
    let this_file = Path::new(file!()).file_name().map(|n| n.to_os_string());
    let needles = forbidden();
    let mut offenders = Vec::new();
    for file in files {
        if file.file_name().map(|n| n.to_os_string()) == this_file {
            continue;
        }
        let source = std::fs::read_to_string(&file).expect("read source");
        for (idx, line) in source.lines().enumerate() {
            for needle in &needles {
                if line.contains(needle.as_str()) {
                    offenders.push(format!("{}:{}: {needle}", file.display(), idx + 1));
                }
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "the window impersonates a terminal:\n{}",
        offenders.join("\n")
    );
}
