//! No input may panic the renderer: random writes, resizes and presents on
//! a [`Screen`], with the wide-grapheme invariant checked after every op.

use magistr_cockpit::render::{Cell, Style};
use magistr_tui::render::Screen;
use proptest::prelude::*;

#[derive(Clone, Debug)]
enum Op {
    PutStr { col: u16, row: u16, text: String },
    Set { col: u16, row: u16, symbol: String },
    Resize { width: u16, height: u16 },
    Present,
}

fn grapheme_soup() -> impl Strategy<Value = String> {
    prop::collection::vec(
        prop_oneof![
            Just("a".to_string()),
            Just("中".to_string()),
            Just("e\u{301}".to_string()),
            Just("\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}\u{200D}\u{1F466}".to_string()),
            Just("\u{301}".to_string()),
            Just("\t".to_string()),
            any::<char>().prop_map(String::from),
        ],
        0..6,
    )
    .prop_map(|parts| parts.concat())
}

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        (0..40u16, 0..12u16, grapheme_soup()).prop_map(|(col, row, text)| Op::PutStr {
            col,
            row,
            text
        }),
        (0..40u16, 0..12u16, grapheme_soup()).prop_map(|(col, row, symbol)| Op::Set {
            col,
            row,
            symbol
        }),
        (0..40u16, 0..12u16).prop_map(|(width, height)| Op::Resize { width, height }),
        Just(Op::Present),
    ]
}

/// Every continuation cell follows a head whose width covers it.
fn check_invariant(screen: &mut Screen) {
    let buffer = screen.buffer();
    let width = usize::from(buffer.width());
    for (index, cell) in buffer.cells().iter().enumerate() {
        if !cell.is_continuation() {
            continue;
        }
        let col = index % width;
        let mut head = index;
        while head % width > 0 && buffer.cells()[head].is_continuation() {
            head -= 1;
        }
        let head_cell = &buffer.cells()[head];
        assert!(
            !head_cell.is_continuation(),
            "continuation at col {col} has no head"
        );
        let covered = head % width + usize::from(head_cell.width());
        assert!(
            col < covered,
            "continuation at col {col} beyond head {:?} (covers to {covered})",
            head_cell.symbol
        );
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]
    #[test]
    fn random_ops_never_panic_and_keep_the_wide_invariant(
        (width, height) in (0..40u16, 0..12u16),
        ops in prop::collection::vec(op(), 0..40),
    ) {
        let mut screen = Screen::new(width, height);
        let mut out = Vec::new();
        for op in ops {
            match op {
                Op::PutStr { col, row, text } => {
                    screen.buffer().put_str(col, row, &text, &Style::new());
                }
                Op::Set { col, row, symbol } => {
                    screen.buffer().set(col, row, Cell::new(&symbol, Style::new()));
                }
                Op::Resize { width, height } => screen.resize(width, height),
                Op::Present => {
                    out.clear();
                    screen.present(&mut out);
                    prop_assert!(out.starts_with(b"\x1b[?2026h"));
                    prop_assert!(out.ends_with(b"\x1b[?2026l"));
                }
            }
            check_invariant(&mut screen);
        }
    }
}
