use super::*;
use crate::cockpit::{ShelfStill, StillKind};
use crate::shelf::load_still;

/// A flat PNG written at `path`, and the still loaded from it.
fn still_at(path: &Path) -> ShelfStill {
    std::fs::create_dir_all(path.parent().expect("a parent")).expect("dirs");
    let mut pixmap = tiny_skia::Pixmap::new(12, 8).expect("a pixmap allocates");
    pixmap.fill(tiny_skia::Color::from_rgba8(200, 40, 40, 255));
    std::fs::write(path, pixmap.encode_png().expect("png")).expect("write");
    load_still(path).expect("the still loads")
}

#[test]
fn the_bundle_is_the_receipts_beside_the_pictures_by_kind() {
    let dir = tempfile::tempdir().expect("tempdir");
    let golden = dir.path().join("golden");
    let mut shelf = Shelf::new("tui2-5b", &["think:7".to_owned()]);
    shelf.gates = vec![("check".to_owned(), true), ("test".to_owned(), false)];
    shelf.stills = vec![
        still_at(&golden.join("screens").join("roadmap-mocha-80x24.png")),
        still_at(&golden.join("motion").join("gate-pop-mocha-80x24.png")),
    ];
    assert_eq!(shelf.count(StillKind::Still), 1);
    assert_eq!(shelf.count(StillKind::Filmstrip), 1);

    let out = dir.path().join("bundle");
    let written = write_bundle(&shelf, None, &out).expect("the bundle writes");
    assert_eq!(
        written,
        vec![
            out.join("receipts.md"),
            out.join("stills").join("roadmap-mocha-80x24.png"),
            out.join("filmstrips").join("gate-pop-mocha-80x24.png"),
        ]
    );
    for path in &written {
        assert!(path.exists(), "{} was written", path.display());
    }
    let doc = std::fs::read_to_string(out.join("receipts.md")).expect("receipts");
    assert!(doc.starts_with("# tui2-5b\n"), "{doc}");
    assert!(
        doc.contains("- Gates: check passed \u{b7} test failed\n"),
        "{doc}"
    );
    assert!(doc.contains("- Trace: think 7\n"), "{doc}");
    assert!(
        doc.contains("## stills\n\n- stills/roadmap-mocha-80x24.png\n"),
        "{doc}"
    );
    assert!(
        doc.contains("## filmstrips\n\n- filmstrips/gate-pop-mocha-80x24.png\n"),
        "{doc}"
    );
    // The copy is the golden's own bytes.
    assert_eq!(
        std::fs::read(&written[1]).expect("copy"),
        std::fs::read(&shelf.stills[0].source).expect("source")
    );
}

#[test]
fn a_bundle_whose_picture_is_gone_is_an_error_not_a_silent_gap() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut shelf = Shelf::new("x", &[]);
    let mut still = still_at(&dir.path().join("golden").join("a.png"));
    still.source = dir.path().join("golden").join("vanished.png");
    shelf.stills = vec![still];
    assert!(write_bundle(&shelf, None, &dir.path().join("bundle")).is_err());
}

#[test]
fn a_bundle_with_a_chapter_carries_the_document_beside_the_receipts() {
    use crate::cockpit::Chapter;
    let dir = tempfile::tempdir().expect("tempdir");
    let shelf = Shelf::new("tui2-5b", &["think:7".to_owned()]);
    let chapter = Chapter {
        chunk: "tui2-5b".to_owned(),
        title: "The shelf, opened".to_owned(),
        prose: "One record, three routes.".to_owned(),
        ..Chapter::default()
    };
    let out = dir.path().join("bundle");
    let written = write_bundle(&shelf, Some(&chapter), &out).expect("the bundle writes");
    assert_eq!(
        written,
        vec![out.join("receipts.md"), out.join("chapter.md")]
    );
    let doc = std::fs::read_to_string(out.join("chapter.md")).expect("chapter");
    assert_eq!(doc, chapter.markdown(&shelf));
    assert!(doc.starts_with("# The shelf, opened\n"), "{doc}");
    assert!(doc.contains("- Trace: think 7\n"), "{doc}");
}
