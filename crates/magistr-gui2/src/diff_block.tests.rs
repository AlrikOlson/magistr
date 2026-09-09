use super::*;
use crate::diff_row::DiffRow;
use crate::medium::Medium;

/// The block frames its header and rows in one hairline ring at the
/// panel radius — the one legitimate 1px ring on this sheet — and
/// stays read-only.
#[test]
fn the_block_frames_header_and_rows() {
    let lex = Lexicon::of(Medium::Night);
    let block = DiffBlock::el(
        "diff · theme/ink.rs",
        vec![
            DiffRow::del("- old", lex.base, &lex),
            DiffRow::add("+ new", lex.base, &lex),
            DiffRow::ctx("  ctx", &lex),
        ],
        lex.base,
        &lex,
    );
    assert!(block.stroke.is_some(), "the frame ring");
    assert!(!block.dashed, "solid — dashed is the slot's");
    assert_eq!(block.radius, 11.0, "the panel radius");
    let mut runs = 0;
    block.each(&mut |_, element| {
        assert!(element.id.is_none() && !element.focusable);
        if matches!(element.kind, crate::element::Kind::Text { .. }) {
            runs += 1;
        }
    });
    assert_eq!(runs, 4, "the header and its three lines");
}
