use std::path::PathBuf;

use magistr_cockpit::cockpit::Shelf;

use super::*;
use crate::element::Kind;
use crate::fixture::Fixture;

/// Every text run in `root`, in tree order.
fn texts(root: &Element) -> Vec<String> {
    let mut out = Vec::new();
    root.each(&mut |_, element| {
        if let Kind::Text { content, .. } = &element.kind {
            out.push(content.clone());
        }
    });
    out
}

fn pictures(root: &Element) -> usize {
    let mut n = 0;
    root.each(&mut |_, element| {
        if matches!(element.kind, Kind::Picture { .. }) {
            n += 1;
        }
    });
    n
}

#[test]
fn the_gallery_shows_the_pictures_the_receipts_and_the_one_export() {
    let lex = Lexicon::of(Medium::Night);
    let shelf = Fixture::shelf();
    let section = ShelfComposition::el(&shelf, 392.0, lex.base, Medium::Night);
    assert_eq!(
        pictures(&section),
        shelf.stills.len(),
        "every still is a picture"
    );
    let texts = texts(&section);
    assert!(texts.contains(&"deliverables".to_owned()));
    assert!(
        texts.contains(&"2 stills \u{b7} 3 filmstrips".to_owned()),
        "{texts:?}"
    );
    for key in ["commit", "gates", "cost", "trace", "ship"] {
        assert!(
            texts.contains(&key.to_owned()),
            "the {key} receipt: {texts:?}"
        );
    }
    assert!(texts.contains(&shelf.commit_text()), "{texts:?}");
    assert!(texts.contains(&"2.07 USD".to_owned()), "{texts:?}");
    assert!(texts.contains(&"think 388".to_owned()), "{texts:?}");
    assert!(texts.contains(&"task:verify".to_owned()), "{texts:?}");
    assert!(texts.contains(&"Export".to_owned()));
    assert!(
        !texts.contains(&"bundle".to_owned()),
        "no bundle until Export ran"
    );
    let mut export = false;
    section.each(&mut |_, element| export |= element.id == Some(ShelfComposition::EXPORT));
    assert!(export, "the verb wears its id");
}

#[test]
fn a_shelf_with_nothing_recorded_says_so_and_shows_no_pictures() {
    let lex = Lexicon::of(Medium::Day);
    let shelf = Shelf::new("bare", &[]);
    let section = ShelfComposition::el(&shelf, 392.0, lex.base, Medium::Day);
    assert_eq!(pictures(&section), 0);
    let texts = texts(&section);
    assert!(
        texts.contains(&"0 stills \u{b7} 0 filmstrips".to_owned()),
        "{texts:?}"
    );
    assert!(
        texts.contains(&"none found".to_owned()),
        "commit: {texts:?}"
    );
    assert!(texts.contains(&"none kept".to_owned()), "gates: {texts:?}");
    assert!(
        texts.contains(&"not recorded".to_owned()),
        "cost: {texts:?}"
    );
    assert!(texts.contains(&"none linked".to_owned()), "refs: {texts:?}");
}

#[test]
fn an_exported_shelf_names_its_bundle_by_its_tail() {
    let lex = Lexicon::of(Medium::Night);
    let mut shelf = Fixture::shelf();
    shelf.bundle = Some(PathBuf::from("/home/me/proj/.magistr/deliverables/gui2-2a"));
    let section = ShelfComposition::el(&shelf, 392.0, lex.base, Medium::Night);
    let texts = texts(&section);
    assert!(texts.contains(&"bundle".to_owned()), "{texts:?}");
    assert!(
        texts.contains(&".magistr/deliverables/gui2-2a".to_owned()),
        "{texts:?}"
    );
}

#[test]
fn past_the_thumbnail_cap_the_rest_is_counted_not_drawn() {
    let lex = Lexicon::of(Medium::Night);
    let mut shelf = Fixture::shelf();
    let extra = shelf.stills[0].clone();
    for _ in 0..12 {
        shelf.stills.push(extra.clone());
    }
    let section = ShelfComposition::el(&shelf, 392.0, lex.base, Medium::Night);
    assert_eq!(pictures(&section), 8);
    assert!(texts(&section).contains(&"+9 more".to_owned()));
}
