//! The cockpit never reaches into the frozen legacy tree: no
//! file under `crates/magistr-tui` may mention `crate::tui`, `magistr::tui`
//! or `src/tui`, and the crate root keeps its `deny(dead_code, unused)`.

use std::path::{Path, PathBuf};

/// Raw terminal bytes end at the terminal crate. Shared interaction can use
/// the same decoded key and paste without importing a terminal protocol.
#[test]
fn terminal_bytes_and_neutral_input_edit_the_same_draft() {
    use magistr_cockpit::cockpit::{Model, Msg, Size, update};
    use magistr_cockpit::input::{KeyCode, KeyEvent};
    use magistr_tui::input::{Decoder, Event};

    let mut decoded = Model::new(Size::new(80, 24));
    let mut neutral = decoded.clone();
    let mut decoder = Decoder::new();
    let mut events = Vec::new();
    for byte in b"\x1b[200~hello\x1b[201~\x1b[33u" {
        decoder.feed(&[*byte], &mut events);
    }
    for event in events {
        let message = match event {
            Event::Paste(text) => Msg::Paste(text),
            Event::Key(key) => Msg::Key(key),
            other => panic!("unexpected input: {other:?}"),
        };
        assert!(update(&mut decoded, message).is_empty());
    }
    assert!(update(&mut neutral, Msg::Paste("hello".into())).is_empty());
    assert!(update(&mut neutral, Msg::Key(KeyEvent::new(KeyCode::Char('!')))).is_empty());
    assert_eq!(decoded.prompt().text(), neutral.prompt().text());
    assert_eq!(decoded.prompt().text(), "hello!");
}

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
fn forbidden() -> [String; 3] {
    [
        ["crate", "::tui"].concat(),
        ["magistr", "::tui"].concat(),
        ["src", "/tui"].concat(),
    ]
}

#[test]
fn nothing_under_the_crate_references_the_legacy_tui() {
    let mut files = Vec::new();
    walk(&crate_root(), &mut files);
    assert!(files.len() >= 10, "walked only {} files", files.len());
    let this_file = Path::new(file!()).file_name().map(|n| n.to_os_string());
    let mut offenders = Vec::new();
    for file in files {
        if file.file_name().map(|n| n.to_os_string()) == this_file {
            continue;
        }
        let source = std::fs::read_to_string(&file).expect("read source");
        for (idx, line) in source.lines().enumerate() {
            for token in forbidden() {
                if line.contains(&token) {
                    offenders.push(format!("{}:{}: {token}", file.display(), idx + 1));
                }
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "legacy references:\n{}",
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
    let mut files = Vec::new();
    walk(&crate_root(), &mut files);
    let needles = [
        "allow(dead_code",
        "allow(unused",
        "expect(dead_code",
        "expect(unused",
    ];
    let this_file = Path::new(file!()).file_name().map(|n| n.to_os_string());
    let offenders: Vec<String> = files
        .iter()
        .filter(|f| f.file_name().map(|n| n.to_os_string()) != this_file)
        .filter_map(|f| std::fs::read_to_string(f).ok().map(|s| (f, s)))
        .filter(|(_, s)| needles.iter().any(|n| s.contains(n)))
        .map(|(f, _)| f.display().to_string())
        .collect();
    assert!(
        offenders.is_empty(),
        "allow blankets:\n{}",
        offenders.join("\n")
    );
}

/// The entry point observes the engines before it takes the terminal over:
/// a store that cannot be opened is reported on a normal stdout, never
/// inside the alternate screen. A viewer observes rather than admits itself
/// as the project's host, so it never contends with a running loop.
#[test]
fn the_example_observes_the_engines_before_the_terminal() {
    let main = crate_root().join("examples/cockpit/main.rs");
    let source = std::fs::read_to_string(main).expect("examples/cockpit/main.rs");
    assert!(
        !source.contains("EngineClient::open("),
        "a viewer must not take the host lease"
    );
    let engines = source
        .find("EngineClient::observe(")
        .expect("the example observes the engines");
    let terminal = source
        .find("RealTerminal::stdio()")
        .expect("the example takes the terminal over");
    assert!(engines < terminal, "the engines open before raw mode");
    let report = source[engines..terminal]
        .find("println!")
        .expect("a failure to open is printed before the terminal is touched");
    assert!(engines < engines + report);
}
