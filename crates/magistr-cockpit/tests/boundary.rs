//! The cockpit knows no terminal (cockpit-crate-cutover): no file under
//! `crates/magistr-cockpit` may name a terminal protocol — `Protocol`,
//! `Passthrough`, `Kitty`, `Sixel` — reach into magistr-tui, or mention
//! the modules that stayed there; and the crate root keeps its
//! `deny(dead_code, unused)`.
//!
//! The first rule is what makes the capability vocabulary
//! (`capability::Mode`) the only way a surface tells the core what it
//! can show: a renderer that folded `Mode::Graphics(Protocol::Kitty(..))`
//! to say "I have pixels" is the impersonation this crate was carved out
//! to retire, and it cannot come back without tripping this walk.

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
fn terminal_names() -> Vec<String> {
    [
        ["Protocol", "::"],
        ["Msg::", "Reply"],
        ["fn ", "from_kitty"],
        ["fn ", "from_sgr"],
        ["fn ", "keyboard_flags"],
        ["Passthrough", "::"],
        ["Kitty", "("],
        ["Sixel", "Compositor"],
        ["magistr", "_tui"],
        ["crate::", "graphics"],
        ["crate::", "screenshot"],
        ["crate::", "runtime"],
        ["crate::", "term"],
        ["crate::", "flat"],
    ]
    .into_iter()
    .map(|pair| pair.concat())
    .collect()
}

fn offenders(needles: &[String]) -> Vec<String> {
    let mut files = Vec::new();
    walk(&crate_root(), &mut files);
    assert!(files.len() >= 100, "walked only {} files", files.len());
    let this_file = Path::new(file!()).file_name().map(|n| n.to_os_string());
    let mut out = Vec::new();
    for file in files {
        if file.file_name().map(|n| n.to_os_string()) == this_file {
            continue;
        }
        let source = std::fs::read_to_string(&file).expect("read source");
        for (idx, line) in source.lines().enumerate() {
            for needle in needles {
                if line.contains(needle.as_str()) {
                    out.push(format!("{}:{}: {needle}", file.display(), idx + 1));
                }
            }
        }
    }
    out
}

#[test]
fn no_cockpit_surface_names_a_terminal_protocol_or_the_terminal_crate() {
    let offenders = offenders(&terminal_names());
    assert!(
        offenders.is_empty(),
        "the terminal reached into the cockpit:\n{}",
        offenders.join("\n")
    );
}

#[test]
fn the_crate_root_denies_dead_and_unused_code() {
    let lib = std::fs::read_to_string(crate_root().join("src/lib.rs")).expect("read lib.rs");
    assert!(
        lib.contains("#![deny(dead_code, unused)]"),
        "lib.rs lost its deny"
    );
}

#[test]
fn no_file_allows_dead_or_unused_code() {
    let needles: Vec<String> = [
        ["allow(", "dead_code"],
        ["allow(", "unused"],
        ["expect(", "dead_code"],
        ["expect(", "unused"],
    ]
    .into_iter()
    .map(|pair| pair.concat())
    .collect();
    let offenders = offenders(&needles);
    assert!(
        offenders.is_empty(),
        "allow blankets:\n{}",
        offenders.join("\n")
    );
}
