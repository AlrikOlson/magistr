use super::*;
use magistr_cockpit::layout::Rect;
use magistr_cockpit::render::{Cell, Headline};

fn count(haystack: &[u8], needle: &[u8]) -> usize {
    haystack
        .windows(needle.len())
        .filter(|w| *w == needle)
        .count()
}

fn frame(encoder: &mut Encoder, changes: &[Change<'_>]) -> Vec<u8> {
    let mut out = Vec::new();
    encoder.encode(80, false, changes, &mut out);
    out
}

#[test]
fn an_empty_frame_is_exactly_the_bracket() {
    let out = frame(&mut Encoder::new(), &[]);
    assert_eq!(out, b"\x1b[?2026h\x1b[?2026l");
}

#[test]
fn every_frame_has_exactly_one_bsu_and_one_esu() {
    let a = Cell::new("a", Style::new());
    let mut encoder = Encoder::new();
    for _ in 0..3 {
        let out = frame(
            &mut encoder,
            &[Change {
                col: 0,
                row: 0,
                cell: &a,
            }],
        );
        assert_eq!(count(&out, BSU), 1);
        assert_eq!(count(&out, ESU), 1);
        assert!(out.starts_with(BSU) && out.ends_with(ESU));
    }
}

#[test]
fn a_single_default_cell_costs_under_64_bytes() {
    let a = Cell::new("a", Style::new());
    let out = frame(
        &mut Encoder::new(),
        &[Change {
            col: 39,
            row: 11,
            cell: &a,
        }],
    );
    assert_eq!(out, b"\x1b[?2026h\x1b[12;40Ha\x1b[?2026l");
    assert!(out.len() < 64, "{} bytes", out.len());
}

#[test]
fn a_single_truecolor_cell_still_costs_under_64_bytes() {
    let style = Style::new()
        .fg(Color::Rgb(255, 255, 255))
        .bg(Color::Rgb(0, 0, 0))
        .attrs(Attrs::BOLD);
    let a = Cell::new("a", style);
    let out = frame(
        &mut Encoder::new(),
        &[Change {
            col: 119,
            row: 39,
            cell: &a,
        }],
    );
    assert!(out.len() < 64, "{} bytes", out.len());
    assert_eq!(count(&out, b"\x1b[1;38;2;255;255;255;48;2;0;0;0m"), 1);
    assert!(out.ends_with(b"\x1b[0m\x1b[?2026l"), "pen reset before ESU");
}

#[test]
fn adjacent_cells_share_one_cursor_move_and_one_sgr() {
    let style = Style::new().fg(Color::Indexed(3));
    let cells: Vec<Cell> = ["a", "b", "c"]
        .iter()
        .map(|s| Cell::new(s, style.clone()))
        .collect();
    let changes: Vec<Change<'_>> = cells
        .iter()
        .enumerate()
        .map(|(i, cell)| Change {
            col: u16::try_from(i).unwrap_or(0),
            row: 2,
            cell,
        })
        .collect();
    let out = frame(&mut Encoder::new(), &changes);
    assert_eq!(count(&out, b"\x1b[3;1H"), 1);
    assert_eq!(
        count(&out, b"H"),
        1,
        "one CUP: {}",
        String::from_utf8_lossy(&out)
    );
    assert_eq!(count(&out, b"\x1b[38;5;3m"), 1);
    assert!(count(&out, b"abc") == 1);
}

#[test]
fn a_wide_glyph_advances_the_cursor_two_columns() {
    let wide = Cell::new("中", Style::new());
    let a = Cell::new("a", Style::new());
    let out = frame(
        &mut Encoder::new(),
        &[
            Change {
                col: 0,
                row: 0,
                cell: &wide,
            },
            Change {
                col: 2,
                row: 0,
                cell: &a,
            },
        ],
    );
    assert_eq!(count(&out, b"H"), 1, "{}", String::from_utf8_lossy(&out));
}

#[test]
fn a_gap_needs_a_second_cursor_move() {
    let a = Cell::new("a", Style::new());
    let out = frame(
        &mut Encoder::new(),
        &[
            Change {
                col: 0,
                row: 0,
                cell: &a,
            },
            Change {
                col: 5,
                row: 0,
                cell: &a,
            },
        ],
    );
    assert_eq!(count(&out, b"\x1b[1;1H"), 1);
    assert_eq!(count(&out, b"\x1b[1;6H"), 1);
}

#[test]
fn the_cursor_is_remembered_across_frames() {
    let a = Cell::new("a", Style::new());
    let mut encoder = Encoder::new();
    let _first = frame(
        &mut encoder,
        &[Change {
            col: 0,
            row: 0,
            cell: &a,
        }],
    );
    let second = frame(
        &mut encoder,
        &[Change {
            col: 1,
            row: 0,
            cell: &a,
        }],
    );
    assert_eq!(second, b"\x1b[?2026ha\x1b[?2026l");
    encoder.reset();
    let third = frame(
        &mut encoder,
        &[Change {
            col: 2,
            row: 0,
            cell: &a,
        }],
    );
    assert_eq!(count(&third, b"\x1b[1;3H"), 1);
}

#[test]
fn writing_into_the_last_column_forgets_the_cursor() {
    let a = Cell::new("a", Style::new());
    let mut encoder = Encoder::new();
    let mut out = Vec::new();
    encoder.encode(
        2,
        false,
        &[Change {
            col: 1,
            row: 0,
            cell: &a,
        }],
        &mut out,
    );
    out.clear();
    encoder.encode(
        2,
        false,
        &[Change {
            col: 0,
            row: 1,
            cell: &a,
        }],
        &mut out,
    );
    assert_eq!(count(&out, b"\x1b[2;1H"), 1);
}

#[test]
fn a_clear_frame_erases_the_display_inside_the_bracket() {
    let mut encoder = Encoder::new();
    let mut out = Vec::new();
    encoder.encode(80, true, &[], &mut out);
    assert_eq!(out, b"\x1b[?2026h\x1b[2J\x1b[?2026l");
}

#[test]
fn hyperlinks_open_on_the_first_linked_cell_and_close_before_esu() {
    let linked = Style::new().link("https://example.com");
    let l = Cell::new("l", linked.clone());
    let k = Cell::new("k", linked);
    let plain = Cell::new("p", Style::new());
    let out = frame(
        &mut Encoder::new(),
        &[
            Change {
                col: 0,
                row: 0,
                cell: &l,
            },
            Change {
                col: 1,
                row: 0,
                cell: &k,
            },
            Change {
                col: 2,
                row: 0,
                cell: &plain,
            },
        ],
    );
    assert_eq!(count(&out, b"\x1b]8;;https://example.com\x1b\\"), 1);
    assert_eq!(count(&out, LINK_CLOSE), 1);
    let open = out
        .windows(4)
        .position(|w| w == b"\x1b]8;")
        .unwrap_or(usize::MAX);
    let close = out
        .windows(LINK_CLOSE.len())
        .position(|w| w == LINK_CLOSE)
        .unwrap_or(0);
    assert!(open < close);
}

#[test]
fn every_attribute_and_color_form_is_encoded() {
    let style = Style::new()
        .attrs(
            Attrs::BOLD
                | Attrs::DIM
                | Attrs::ITALIC
                | Attrs::UNDERLINE
                | Attrs::REVERSE
                | Attrs::STRIKETHROUGH,
        )
        .fg(Color::Indexed(200))
        .bg(Color::Rgb(1, 22, 333u16 as u8));
    let cell = Cell::new("x", style);
    let out = frame(
        &mut Encoder::new(),
        &[Change {
            col: 0,
            row: 0,
            cell: &cell,
        }],
    );
    assert_eq!(count(&out, b"\x1b[1;2;3;4;7;9;38;5;200;48;2;1;22;77m"), 1);
}

#[test]
fn numbers_render_all_their_digits() {
    let mut out = Vec::new();
    push_number(&mut out, 0);
    out.push(b' ');
    push_number(&mut out, 7);
    out.push(b' ');
    push_number(&mut out, 65536);
    assert_eq!(out, b"0 7 65536");
}

#[test]
fn changing_a_non_default_pen_resets_before_the_new_parameters() {
    let bold = Cell::new("a", Style::new().attrs(Attrs::BOLD));
    let red = Cell::new("b", Style::new().fg(Color::Indexed(1)));
    let out = frame(
        &mut Encoder::new(),
        &[
            Change {
                col: 0,
                row: 0,
                cell: &bold,
            },
            Change {
                col: 1,
                row: 0,
                cell: &red,
            },
        ],
    );
    assert_eq!(count(&out, b"\x1b[1m"), 1);
    assert_eq!(count(&out, b"\x1b[0;38;5;1m"), 1);
}

/// One cell in `style` at the origin, encoded by a fresh encoder.
fn styled_frame(styled_underlines: bool, style: Style) -> Vec<u8> {
    let cell = Cell::new("x", style);
    let mut encoder = Encoder::new();
    if styled_underlines {
        encoder.enable_underline_styles();
    }
    frame(
        &mut encoder,
        &[Change {
            col: 0,
            row: 0,
            cell: &cell,
        }],
    )
}

/// The SGR the frame sets its pen with: what follows the cursor move,
/// up to and including its `m`.
fn opening_sgr(out: &[u8]) -> Vec<u8> {
    let moved = out
        .iter()
        .position(|b| *b == b'H')
        .expect("the frame moves the cursor");
    let sgr = &out[moved + 1..];
    let end = sgr
        .iter()
        .position(|b| *b == b'm')
        .expect("the pen ends with m");
    sgr[..=end].to_vec()
}

#[test]
fn a_styled_underline_is_sgr_four_colon_its_code() {
    for (attrs, expected) in [
        (Attrs::UNDERLINE, b"\x1b[4m".to_vec()),
        (Attrs::UNDERLINE_DOUBLE, b"\x1b[4:2m".to_vec()),
        (Attrs::UNDERLINE_CURLY, b"\x1b[4:3m".to_vec()),
        (Attrs::UNDERLINE_DOTTED, b"\x1b[4:4m".to_vec()),
        (Attrs::UNDERLINE_DASHED, b"\x1b[4:5m".to_vec()),
    ] {
        let out = styled_frame(true, Style::new().attrs(attrs));
        assert_eq!(opening_sgr(&out), expected, "{attrs:?}");
    }
}

#[test]
fn a_terminal_outside_the_allowlist_gets_the_plain_rule() {
    for attrs in [
        Attrs::UNDERLINE,
        Attrs::UNDERLINE_DOUBLE,
        Attrs::UNDERLINE_CURLY,
        Attrs::UNDERLINE_DOTTED,
        Attrs::UNDERLINE_DASHED,
    ] {
        let out = styled_frame(false, Style::new().attrs(attrs));
        assert_eq!(opening_sgr(&out), b"\x1b[4m".to_vec(), "{attrs:?}");
        assert!(
            !out.windows(2).any(|w| w == b"4:"),
            "{attrs:?} must not print a sub-parameter"
        );
    }
}

#[test]
fn an_underline_colour_is_sgr_fifty_eight() {
    let style = Style::new()
        .attrs(Attrs::UNDERLINE_CURLY)
        .underline_color(Color::Rgb(1, 2, 3));
    assert_eq!(
        opening_sgr(&styled_frame(true, style.clone())),
        b"\x1b[4:3;58;2;1;2;3m".to_vec()
    );
    let indexed = Style::new()
        .attrs(Attrs::UNDERLINE)
        .underline_color(Color::Indexed(9));
    assert_eq!(
        opening_sgr(&styled_frame(true, indexed)),
        b"\x1b[4;58;5;9m".to_vec()
    );
    assert!(
        !styled_frame(false, style).windows(3).any(|w| w == b";58"),
        "a terminal outside the allowlist is never sent a rule colour"
    );
}

#[test]
fn a_coloured_rule_is_reset_before_esu() {
    let out = styled_frame(
        true,
        Style::new()
            .attrs(Attrs::UNDERLINE_CURLY)
            .underline_color(Color::Rgb(1, 2, 3)),
    );
    // SGR 0 clears the style and the colour together, so `4:0` and `59`
    // never need to reach the wire.
    assert!(out.ends_with(b"\x1b[0m\x1b[?2026l"), "{out:?}");
}

#[test]
fn a_rule_colour_the_terminal_never_saw_does_not_churn_the_pen() {
    let cell = Cell::new(
        "x",
        Style::new()
            .attrs(Attrs::UNDERLINE_CURLY)
            .underline_color(Color::Rgb(1, 2, 3)),
    );
    let changes: Vec<Change<'_>> = (0..4)
        .map(|col| Change {
            col,
            row: 0,
            cell: &cell,
        })
        .collect();
    let out = frame(&mut Encoder::new(), &changes);
    assert_eq!(
        count(&out, b"\x1b[4m"),
        1,
        "four identical cells cost one SGR: {out:?}"
    );
}

/// Two changes on one row, the second at the column the first's width
/// says it ends at: the number of CUPs the frame needed.
fn cups_for_adjacent(first: &str) -> usize {
    let head = Cell::new(first, Style::new());
    let a = Cell::new("a", Style::new());
    let out = frame(
        &mut Encoder::new(),
        &[
            Change {
                col: 0,
                row: 0,
                cell: &head,
            },
            Change {
                col: head.width(),
                row: 0,
                cell: &a,
            },
        ],
    );
    count(&out, b"H")
}

#[test]
fn a_multi_scalar_grapheme_forgets_the_cursor() {
    let family = "\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}\u{200D}\u{1F466}";
    let head = Cell::new(family, Style::new());
    let mut encoder = Encoder::new();
    let mut out = Vec::new();
    encoder.encode(
        80,
        false,
        &[Change {
            col: 0,
            row: 0,
            cell: &head,
        }],
        &mut out,
    );
    assert_eq!(
        encoder.cursor, None,
        "the cursor is forgotten after {family:?}"
    );
}

#[test]
fn the_change_after_a_disputed_grapheme_moves_the_cursor_explicitly() {
    for disputed in [
        "\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}\u{200D}\u{1F466}", // ZWJ family
        "\u{2764}\u{FE0F}",                                             // heart + VS16
        "\u{1F1F8}\u{1F1EA}",                                           // flag pair
        "e\u{301}",                                                     // combining mark
        "\u{1F680}",                                                    // single-scalar emoji
    ] {
        assert_eq!(cups_for_adjacent(disputed), 2, "{disputed:?}");
    }
}

#[test]
fn agreed_graphemes_still_share_one_cursor_move() {
    for agreed in ["a", "中", "한", "ｱ", "─", "✦"] {
        assert_eq!(cups_for_adjacent(agreed), 1, "{agreed:?}");
    }
}

fn headline(text: &str) -> Headline {
    Headline::fill(text, Rect::new(4, 2, 20, 2), Style::new()).expect("the rect fits a headline")
}

#[test]
fn a_headline_is_one_osc_66_run_at_scale_two() {
    let mut out = Vec::new();
    Encoder::new().headline(&headline("magistr"), &mut out);
    assert_eq!(out, b"\x1b]66;s=2;magistr   \x1b\\");
}

#[test]
fn a_headline_run_carries_the_padded_text_the_slot_holds() {
    let mut out = Vec::new();
    let fitted = Headline::fill("ab", Rect::new(0, 0, 10, 2), Style::new()).expect("fits");
    Encoder::new().headline(&fitted, &mut out);
    assert_eq!(out, b"\x1b]66;s=2;ab   \x1b\\");
    let mut wide = Vec::new();
    let fitted = Headline::fill("漢字ab", Rect::new(0, 0, 8, 2), Style::new()).expect("fits");
    Encoder::new().headline(&fitted, &mut wide);
    assert_eq!(wide, "\u{1b}]66;s=2;漢字\u{1b}\\".as_bytes());
}
