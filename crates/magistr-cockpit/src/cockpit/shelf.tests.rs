use std::io::Cursor;
use std::path::PathBuf;

use super::*;
use crate::pixel::CellSize;
use crate::turn::StreamImage;

fn refs(list: &[&str]) -> Vec<String> {
    list.iter().map(|r| (*r).to_owned()).collect()
}

/// A still of `kind` called `name`, decoded from a flat PNG held in
/// memory — the data a job would have loaded, without the file.
fn still(name: &str, kind: StillKind) -> ShelfStill {
    let mut pixmap = tiny_skia::Pixmap::new(12, 8).expect("a pixmap allocates");
    pixmap.fill(tiny_skia::Color::from_rgba8(200, 40, 40, 255));
    let bytes = pixmap.encode_png().expect("png");
    let image = StreamImage::decode(Cursor::new(bytes).get_ref(), name, CellSize::DEFAULT)
        .expect("the picture decodes");
    ShelfStill {
        name: name.to_owned(),
        source: PathBuf::from("golden").join(kind.dir_name()).join(name),
        kind,
        image,
    }
}

#[test]
fn a_new_shelf_reads_its_refs_off_the_chunks_cross_refs() {
    let shelf = Shelf::new(
        "tui2-5b",
        &refs(&[
            "task:verify",
            "think:491",
            "chunk:other",
            "think:490",
            "think:491",
        ]),
    );
    assert_eq!(shelf.chunk, "tui2-5b");
    assert_eq!(shelf.think_refs, vec![490, 491], "ascending, deduplicated");
    assert_eq!(shelf.task_refs, vec!["verify"]);
    assert_eq!(shelf.trace_text(), "think 490 \u{b7} 491");
    assert_eq!(shelf.trace_refs_text(), "think:490 \u{b7} think:491");
    assert_eq!(shelf.ship_text(), "task:verify");
    assert!(shelf.commit.is_none() && shelf.gates.is_empty() && shelf.stills.is_empty());
}

#[test]
fn every_receipt_says_so_when_nothing_was_recorded() {
    let shelf = Shelf::new("x", &[]);
    assert_eq!(shelf.commit_text(), "none found");
    assert_eq!(shelf.cost_text(), "not recorded");
    assert_eq!(shelf.trace_text(), "none linked");
    assert_eq!(shelf.trace_refs_text(), "none linked");
    assert_eq!(shelf.ship_text(), "none linked");
    assert_eq!(shelf.shipped_text(), "no stamp");
    assert_eq!(shelf.gates_text(), "none kept");
    assert_eq!(shelf.recording_text(), "not recorded");
    assert_eq!(shelf.tokens_text(), None);
    assert_eq!(shelf.spend_text(), "not recorded");
    let doc = shelf.receipts_markdown();
    assert!(doc.contains("- Gates: none kept\n"), "{doc}");
    assert!(doc.contains("- Recording: not recorded\n"), "{doc}");
    assert!(
        !doc.contains("## stills"),
        "no picture section without pictures"
    );
}

#[test]
fn the_report_rows_count_the_gates_the_tokens_and_the_recording() {
    let mut shelf = Shelf::new("x", &[]);
    shelf.gates = vec![("check".to_owned(), true), ("test".to_owned(), true)];
    assert_eq!(shelf.gates_text(), "2 passed");
    shelf.gates.push(("lint".to_owned(), false));
    assert_eq!(shelf.gates_text(), "2 of 3 passed");
    shelf.cost_cents = Some(207);
    shelf.tokens = Some(41_000);
    assert_eq!(shelf.tokens_text().as_deref(), Some("41k tokens"));
    assert_eq!(shelf.spend_text(), "2.07 USD \u{b7} 41k tokens");
    shelf.tokens = Some(412);
    assert_eq!(shelf.tokens_text().as_deref(), Some("412 tokens"));
    shelf.tokens = Some(1_250);
    assert_eq!(shelf.tokens_text().as_deref(), Some("1.3k tokens"));
    shelf.tokens = Some(2_000);
    assert_eq!(shelf.tokens_text().as_deref(), Some("2k tokens"));
    shelf.recording_secs = Some(12 * 60 + 30);
    assert_eq!(shelf.recording_text(), "12 min");
    shelf.recording_secs = Some(45);
    assert_eq!(shelf.recording_text(), "45 s");
    let doc = shelf.receipts_markdown();
    assert!(doc.contains("- Cost: 2.07 USD \u{b7} 2k tokens\n"), "{doc}");
    assert!(doc.contains("- Recording: 45 s\n"), "{doc}");
}

#[test]
fn the_copy_law_spells_money_and_stamps() {
    let mut shelf = Shelf::new("x", &[]);
    shelf.cost_cents = Some(207);
    shelf.shipped_secs = Some(1_788_341_100);
    shelf.commit = Some(("a3f9c21".to_owned(), "feat: the shelf".to_owned()));
    assert_eq!(shelf.cost_text(), "2.07 USD");
    assert_eq!(shelf.shipped_text(), "2026-09-02 09:25 UTC");
    assert_eq!(shelf.commit_text(), "a3f9c21 \u{b7} feat: the shelf");
    shelf.cost_cents = Some(5);
    assert_eq!(shelf.cost_text(), "0.05 USD");
}

#[test]
fn the_receipts_document_lists_every_receipt_and_the_pictures_by_kind() {
    let mut shelf = Shelf::new("tui2-5b", &refs(&["think:7", "task:verify"]));
    shelf.gates = vec![("check".to_owned(), true), ("test".to_owned(), false)];
    shelf.stills = vec![
        still("roadmap-mocha-80x24.png", StillKind::Still),
        still("gate-pop-mocha-80x24.png", StillKind::Filmstrip),
    ];
    assert_eq!(shelf.count(StillKind::Still), 1);
    assert_eq!(shelf.count(StillKind::Filmstrip), 1);
    let doc = shelf.receipts_markdown();
    assert!(doc.starts_with("# tui2-5b\n"), "{doc}");
    assert!(
        doc.contains("- Gates: check passed \u{b7} test failed\n"),
        "{doc}"
    );
    assert!(doc.contains("- Trace: think 7\n"), "{doc}");
    assert!(doc.contains("- Ship: task:verify\n"), "{doc}");
    assert!(
        doc.contains("## stills\n\n- stills/roadmap-mocha-80x24.png\n"),
        "{doc}"
    );
    assert!(
        doc.contains("## filmstrips\n\n- filmstrips/gate-pop-mocha-80x24.png\n"),
        "{doc}"
    );
}
