use super::*;
use crate::medium::Medium;

fn runs(root: &Element) -> Vec<(String, Srgba, Voice)> {
    let mut runs = Vec::new();
    root.each(&mut |_, element| {
        if let crate::element::Kind::Text { content, voice } = &element.kind {
            runs.push((
                content.clone(),
                element.fill.expect("a run carries ink"),
                *voice,
            ));
        }
    });
    runs
}

fn texts(root: &Element) -> Vec<String> {
    runs(root).into_iter().map(|(text, ..)| text).collect()
}

/// A run's pane keeps the output's tail and counts the rest above it;
/// a short output is shown whole; an empty one says so.
#[test]
fn the_output_pane_is_bounded_to_its_tail() {
    let lex = Lexicon::of(Medium::Night);
    let long: String = (1..=30).map(|n| format!("line {n}\n")).collect();
    let detail = ToolDetail::Run {
        command: "cargo nextest run",
        output: &long,
        term: None,
    };
    let texts = texts(&detail.el(lex.mantle, &lex));
    assert_eq!(texts[0], "command");
    assert_eq!(texts[1], "cargo nextest run");
    assert_eq!(texts[2], "output");
    assert_eq!(texts[3], "\u{2026} 18 earlier lines");
    assert_eq!(texts[4], "line 19");
    assert_eq!(texts.last().map(String::as_str), Some("line 30"));
    assert_eq!(texts.len(), 4 + ToolDetail::PANE_LINES);

    let short = ToolDetail::Run {
        command: "ls",
        output: "a\nb\n",
        term: None,
    };
    assert_eq!(
        &texts_of(&short, &lex)[3..],
        ["a", "b"],
        "the trailing newline is not a line"
    );
    let empty = ToolDetail::Run {
        command: "true",
        output: "",
        term: None,
    };
    assert_eq!(texts_of(&empty, &lex)[3], "no output");
}

fn texts_of(detail: &ToolDetail<'_>, lex: &Lexicon) -> Vec<String> {
    texts(&detail.el(lex.mantle, lex))
}

/// A search marks the query in every hit, without regard to case, in
/// the match line's bold accent run.
#[test]
fn a_search_marks_the_query_in_its_hits() {
    let lex = Lexicon::of(Medium::Night);
    let detail = ToolDetail::Search {
        query: "Contrast",
        hits: "theme/ink.rs:12: fn ensure_contrast(\ndocs/architecture/desktop.md:88: the contrast floor\n",
    };
    let runs = runs(&detail.el(lex.mantle, &lex));
    let marked: Vec<&(String, Srgba, Voice)> = runs
        .iter()
        .filter(|(text, ink, voice)| *ink == lex.mauve && voice.weight == 700 && text == "contrast")
        .collect();
    assert_eq!(marked.len(), 2, "one mark per hit: {runs:?}");
    assert_eq!(
        marks("a b a", "a"),
        [(0, 1), (4, 5)],
        "every occurrence, by byte"
    );
    assert!(
        marks("anything", "").is_empty(),
        "an empty query marks nothing"
    );
}

/// A read names its path and range on one line; a fetch its url in
/// blue; an MCP tool its server · tool.
#[test]
fn the_subjects_read_as_the_design_spells_them() {
    let lex = Lexicon::of(Medium::Night);
    let read = ToolDetail::Read {
        path: "src/main.rs",
        range: Some((120, Some(40))),
        excerpt: "fn main() {}",
    };
    assert_eq!(
        texts_of(&read, &lex)[1],
        "src/main.rs \u{b7} lines 120\u{2013}159"
    );
    let open = ToolDetail::Read {
        path: "src/main.rs",
        range: Some((7, None)),
        excerpt: "",
    };
    assert_eq!(texts_of(&open, &lex)[1], "src/main.rs \u{b7} from line 7");
    let fetch = ToolDetail::Fetch {
        url: "https://example.org",
        body: "<html>",
    };
    let runs = runs(&fetch.el(lex.mantle, &lex));
    assert_eq!(runs[1].0, "https://example.org");
    assert_eq!(runs[1].1, lex.blue);
    let mcp = ToolDetail::Mcp {
        server: Some("workbench-mcp"),
        tool: "read",
        input: "{}",
        output: "ok",
    };
    assert_eq!(texts_of(&mcp, &lex)[1], "workbench-mcp \u{b7} read");
    let think = ToolDetail::Think {
        thought: "one\ntwo",
    };
    assert_eq!(texts_of(&think, &lex), ["thought", "one", "two"]);
}

/// A check's rows wear the diagnostic's own ink: error red, warning
/// yellow, help blue, a location subdued, the rest the text ink; the
/// command leads when the recorder kept one.
#[test]
fn a_checks_rows_wear_their_diagnostic_ink() {
    let lex = Lexicon::of(Medium::Night);
    let detail = ToolDetail::Check {
        command: Some("cargo clippy"),
        rows: "error: unused variable `x`\n  --> src/main.rs:3:9\nwarning: dead code\nhelp: remove it\nsrc/lib.rs:10:1\nplain",
        term: None,
    };
    let runs = runs(&detail.el(lex.mantle, &lex));
    let ink = |needle: &str| {
        runs.iter()
            .find(|(text, ..)| text.contains(needle))
            .unwrap_or_else(|| panic!("{needle} is on the detail"))
            .1
    };
    assert_eq!(runs[1].0, "cargo clippy");
    assert_eq!(ink("error:"), lex.red);
    assert_eq!(ink("-->"), lex.sb0);
    assert_eq!(ink("warning:"), lex.yellow);
    assert_eq!(ink("help:"), lex.blue);
    assert_eq!(ink("src/lib.rs:10:1"), lex.sb0);
    assert_eq!(ink("plain"), lex.tx);
    let bare = ToolDetail::Check {
        command: None,
        rows: "ok",
        term: None,
    };
    assert_eq!(texts_of(&bare, &lex), ["diagnostics", "ok"]);
}

/// The receipt line joins its parts and names a cut record's size.
#[test]
fn the_receipt_line_names_a_cut_record() {
    let lex = Lexicon::of(Medium::Night);
    let whole = ToolDetail::receipt(
        &["exit 0".to_owned(), String::new(), "1.2 s".to_owned()],
        None,
        lex.mantle,
        &lex,
    );
    assert_eq!(
        texts(&whole),
        ["exit 0 \u{b7} 1.2 s"],
        "an empty part is skipped"
    );
    let cut = ToolDetail::receipt(
        &["receipt #238".to_owned()],
        Some(236_544),
        lex.mantle,
        &lex,
    );
    assert_eq!(
        texts(&cut),
        ["receipt #238 \u{b7} cut at 64 KiB \u{b7} 231 KiB recorded"]
    );
}

/// No naked text: every run a detail sets is claimed.
#[test]
fn every_run_is_claimed() {
    let lex = Lexicon::of(Medium::Day);
    let details = [
        ToolDetail::Run {
            command: "ls",
            output: "a",
            term: None,
        },
        ToolDetail::Read {
            path: "p",
            range: None,
            excerpt: "e",
        },
        ToolDetail::Search {
            query: "q",
            hits: "a q b",
        },
        ToolDetail::Fetch {
            url: "u",
            body: "b",
        },
        ToolDetail::Think { thought: "t" },
        ToolDetail::Mcp {
            server: None,
            tool: "t",
            input: "i",
            output: "o",
        },
        ToolDetail::Check {
            command: None,
            rows: "r",
            term: None,
        },
    ];
    for detail in &details {
        for run in detail.el(lex.mantle, &lex).runs() {
            assert!(run.owner.is_some(), "{:?} is naked", run.text);
        }
    }
    for run in ToolDetail::receipt(&["x".to_owned()], Some(1), lex.mantle, &lex).runs() {
        assert!(run.owner.is_some());
    }
}

/// A program the reader ended says so where its progress strip stood: `killed by you`
/// in the failure hue, on a run and on a check alike, and nothing of the kind on a run
/// that ended on its own.
#[test]
fn a_killed_programs_body_names_the_human_where_the_strip_was() {
    let lex = Lexicon::of(Medium::Night);
    let mut term = Term::new();
    term.push(b"building\r\n", 1);
    let mut killed = term.clone();
    killed.kill();
    for detail in [
        ToolDetail::Run {
            command: "cargo build",
            output: "",
            term: Some((&killed, 0, true)),
        },
        ToolDetail::Check {
            command: None,
            rows: "",
            term: Some((&killed, 0, true)),
        },
    ] {
        let root = detail.el(lex.base, &lex);
        let words: Vec<(String, Srgba, Voice)> = runs(&root)
            .into_iter()
            .filter(|(text, ..)| text == "killed by you")
            .collect();
        assert_eq!(words.len(), 1, "{words:?}");
        assert_eq!(words[0].1, lex.red);
        assert_eq!(words[0].2, Voice::RECEIPT_KEY, "the cap voice");
    }
    term.finish();
    let quiet = ToolDetail::Run {
        command: "cargo build",
        output: "",
        term: Some((&term, 0, true)),
    }
    .el(lex.base, &lex);
    assert!(!texts(&quiet).iter().any(|text| text == "killed by you"));
}
