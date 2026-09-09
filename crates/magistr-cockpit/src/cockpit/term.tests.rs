use super::*;

/// Pane row `row`'s text, trailing blanks trimmed; `None` past the
/// content.
fn text(term: &Term, row: usize) -> Option<String> {
    term.pane_row(row).map(|cells| {
        cells
            .iter()
            .map(|c| c.text.as_str())
            .collect::<String>()
            .trim_end()
            .to_owned()
    })
}

/// `n` numbered lines, each ending the row.
fn lines(from: usize, n: usize) -> Vec<u8> {
    (from..from + n)
        .map(|i| format!("line {i}\r\n"))
        .collect::<String>()
        .into_bytes()
}

#[test]
fn a_red_run_is_a_red_cell_and_the_reset_ends_it() {
    let mut term = Term::new();
    term.push(
        b"\x1b[31mred\x1b[0m plain \x1b[1;38;2;250;179;135mpeach\x1b[0m",
        3,
    );
    let row = term.pane_row(0).expect("the first row");
    assert_eq!(row[0].text, "r");
    assert_eq!(row[0].fg, Color::Indexed(1));
    assert_eq!(row[0].bg, Color::Default);
    assert_eq!(row[3].text, " ");
    assert_eq!(row[3].fg, Color::Default, "the reset took the red off");
    assert_eq!(row[10].text, "p");
    assert_eq!(row[10].fg, Color::Rgb(250, 179, 135), "truecolor is itself");
    assert!(row[10].attrs.contains(Attrs::BOLD));
    assert!(row[0].same_pen(&row[1]));
    assert!(!row[0].same_pen(&row[3]));
    assert_eq!(row.len(), usize::from(Term::COLS), "every column present");
    assert_eq!(row[40].text, " ", "a cell nothing reached is a space");
    assert_eq!(text(&term, 0).as_deref(), Some("red plain peach"));
    assert_eq!(text(&term, 1), None, "one row written: the pane holds one");
    assert_eq!(
        term.cursor(),
        Some((0, 15)),
        "the cursor rests after the text"
    );
}

#[test]
fn a_carriage_return_overwrites_the_row_rather_than_adding_one() {
    let mut term = Term::new();
    term.push(b"progress [###-------] 30%\r", 1);
    term.push(b"progress [#######---] 70%", 2);
    assert_eq!(text(&term, 0).as_deref(), Some("progress [#######---] 70%"));
    assert_eq!(text(&term, 1), None);
    assert_eq!(term.text_rows()[0], "progress [#######---] 70%");
}

#[test]
fn every_attribute_and_a_wide_grapheme_carry_through() {
    let mut term = Term::new();
    // Bold and dim are one intensity to the parser, so they are asked
    // for on different cells.
    term.push(b"\x1b[1mb\x1b[0m\x1b[2;3;4;7mx\x1b[0m \xe6\x97\xa5", 1);
    let row = term.pane_row(0).expect("the first row");
    assert_eq!(row[0].attrs, Attrs::BOLD);
    for attr in [Attrs::DIM, Attrs::ITALIC, Attrs::UNDERLINE, Attrs::REVERSE] {
        assert!(row[1].attrs.contains(attr), "{attr:?}");
    }
    assert!(!row[1].attrs.contains(Attrs::BOLD));
    assert!(row[2].attrs.is_empty());
    assert_eq!(row[3].text, "日");
    assert!(row[4].is_blank(), "the wide grapheme's second column");
    assert!(!row[3].is_blank());
}

#[test]
fn the_pane_is_the_last_eight_rows_written_and_follows_the_tail() {
    let mut term = Term::new();
    term.push(&lines(1, 5), 5);
    // Five lines and the empty row after the last newline: six rows,
    // all of them in the window, two of its slots past the content.
    assert_eq!(text(&term, 0).as_deref(), Some("line 1"));
    assert_eq!(text(&term, 4).as_deref(), Some("line 5"));
    assert_eq!(text(&term, 5).as_deref(), Some(""));
    assert_eq!(text(&term, 6), None);
    assert_eq!(text(&term, Term::PANE_ROWS), None, "never past the window");
    assert_eq!(term.cursor(), Some((5, 0)));

    term.push(&lines(6, 25), 6);
    // Thirty lines and the empty row: 31 rows, seven of them scrolled
    // off the screen into the scrollback, the window on the last eight.
    assert_eq!(text(&term, 0).as_deref(), Some("line 24"));
    assert_eq!(text(&term, 6).as_deref(), Some("line 30"));
    assert_eq!(text(&term, 7).as_deref(), Some(""));
    assert_eq!(term.cursor(), Some((7, 0)));
    assert!(term.is_following());
    assert_eq!(term.arrived(), 0);
    assert_eq!(term.offset(), 0);
}

#[test]
fn scrolling_up_anchors_the_window_and_counts_what_arrives_under_it() {
    let mut term = Term::new();
    term.push(&lines(1, 30), 5);
    term.scroll_up(3);
    assert!(!term.is_following());
    assert_eq!(term.offset(), 3);
    assert_eq!(text(&term, 0).as_deref(), Some("line 21"));
    assert_eq!(text(&term, 7).as_deref(), Some("line 28"));
    assert_eq!(term.arrived(), 0, "nothing came since");
    assert_eq!(term.cursor(), None, "the cursor is off the held window");

    term.push(&lines(31, 4), 6);
    assert_eq!(
        text(&term, 0).as_deref(),
        Some("line 21"),
        "the window held still"
    );
    assert_eq!(term.offset(), 7, "four rows further from the tail");
    assert_eq!(term.arrived(), 4, "four rows arrived under it");

    term.scroll_down(2);
    assert_eq!(term.offset(), 5);
    assert_eq!(text(&term, 0).as_deref(), Some("line 23"));
    assert_eq!(term.arrived(), 4, "still held, still counting");
    assert!(term.follow());
    assert!(term.is_following());
    assert_eq!(term.arrived(), 0);
    assert_eq!(text(&term, 6).as_deref(), Some("line 34"));
    assert_eq!(text(&term, 7).as_deref(), Some(""));
    assert!(!term.follow(), "already on the tail");

    // A scroll past the top clamps to the first eight rows written —
    // seven of them in the parser's scrollback by now.
    term.scroll_up(1_000);
    assert_eq!(text(&term, 0).as_deref(), Some("line 1"));
    assert_eq!(text(&term, 7).as_deref(), Some("line 8"));
    assert_eq!(term.offset(), 35 - Term::PANE_ROWS, "35 rows written");
    // And a scroll back past the tail follows again.
    term.scroll_down(1_000);
    assert!(term.is_following());
    assert_eq!(text(&term, 7).as_deref(), Some(""));
}

#[test]
fn a_short_output_cannot_be_scrolled_and_a_notch_down_while_following_is_nothing() {
    let mut term = Term::new();
    term.push(b"one\r\ntwo", 1);
    term.scroll_up(5);
    assert!(term.is_following(), "two rows: nothing above the window");
    term.scroll_down(5);
    assert!(term.is_following());
    assert_eq!(text(&term, 1).as_deref(), Some("two"));
}

#[test]
fn the_scrollback_is_bounded() {
    let mut term = Term::new();
    for chunk in 0..11 {
        term.push(&lines(chunk * 1_000, 1_000), 1);
    }
    term.scroll_up(usize::MAX);
    // Ten thousand rows above the screen, and the screen's own —
    // the window's eight at the top of what is still there.
    assert_eq!(
        term.offset(),
        Term::SCROLLBACK + usize::from(Term::ROWS) - Term::PANE_ROWS
    );
    // 11,001 rows written, 10,024 kept: the first kept is line 977.
    assert_eq!(text(&term, 0).as_deref(), Some("line 977"));
}

#[test]
fn every_row_carries_the_tick_it_arrived_on() {
    let mut term = Term::new();
    term.push(b"first\r\n", 10);
    term.push(b"second\r\n", 20);
    term.push(b"third", 30);
    assert_eq!(term.stamp(0), Some(10));
    assert_eq!(term.stamp(1), Some(20));
    assert_eq!(term.stamp(2), Some(30));
    assert_eq!(term.stamp(3), None, "a row nothing has reached yet");
    // Rows that scroll into the scrollback keep their stamps and the
    // window's rows read from where the window is.
    term.push(&lines(1, 25), 40);
    term.scroll_up(usize::MAX);
    assert_eq!(term.stamp(0), Some(10), "the first row, in the scrollback");
    assert_eq!(term.stamp(1), Some(20));
    assert_eq!(term.stamp(2), Some(40), "the third row was overwritten");
    term.follow();
    assert_eq!(term.stamp(7), Some(40));
    assert_eq!(term.stamp(Term::PANE_ROWS), None, "past the window");
}

#[test]
fn finishing_freezes_the_cursor_and_hiding_it_hides_it() {
    let mut term = Term::new();
    term.push(b"done\r\n", 1);
    assert_eq!(term.cursor(), Some((1, 0)));
    let mut hidden = term.clone();
    hidden.push(b"\x1b[?25l", 2);
    assert_eq!(hidden.cursor(), None, "DECTCEM off");
    assert!(!term.is_finished());
    term.finish();
    assert!(term.is_finished());
    assert_eq!(term.cursor(), None);
}

#[test]
fn a_clone_is_the_same_screen_and_equal_and_a_push_tells_them_apart() {
    let mut term = Term::new();
    term.push(&lines(1, 30), 5);
    term.scroll_up(2);
    let copy = term.clone();
    assert_eq!(copy, term);
    assert_eq!(copy.offset(), 2);
    assert_eq!(text(&copy, 0).as_deref(), Some("line 22"));
    assert_eq!(copy.arrived(), 0);
    assert_eq!(copy.stamp(0), term.stamp(0));
    term.push(b"more\r\n", 6);
    assert_ne!(copy, term);
    assert_eq!(term.arrived(), 1);
    assert_eq!(
        format!("{term:?}"),
        "Term { bytes: 267, rows: 32, offset: 3, finished: false }"
    );
    assert_eq!(Term::default(), Term::new());
}

#[test]
fn the_kept_bytes_are_bounded_and_the_newest_stay() {
    let mut term = Term::new();
    term.push(&vec![b'x'; Term::KEEP], 1);
    term.push(b"tail", 2);
    assert_eq!(term.bytes().len(), Term::KEEP);
    assert!(term.bytes().ends_with(b"tail"));
}

/// The dock's window: widening it reaches further up
/// into the same content, the card's rows stay the window's last eight
/// so the tail under a raised dock is the tail, and the cursor is told
/// against each. Clamped to the screen's own rows — the parser's view.
#[test]
fn a_wider_window_reaches_up_and_the_card_keeps_the_tail() {
    let mut term = Term::new();
    term.push(&lines(1, 30), 5);
    assert_eq!(term.window(), Term::PANE_ROWS);
    term.set_window(16);
    assert_eq!(term.window(), 16);
    // Thirty-one content rows (the empty one after the last newline):
    // the window ends there and starts sixteen up.
    let window = |t: &Term, row: usize| {
        t.window_row(row).map(|cells| {
            cells
                .iter()
                .map(|c| c.text.as_str())
                .collect::<String>()
                .trim_end()
                .to_owned()
        })
    };
    assert_eq!(window(&term, 0).as_deref(), Some("line 16"));
    assert_eq!(window(&term, 14).as_deref(), Some("line 30"));
    assert_eq!(window(&term, 15).as_deref(), Some(""));
    assert_eq!(window(&term, 16), None, "past the window");
    assert_eq!(
        text(&term, 0).as_deref(),
        Some("line 24"),
        "the card's first"
    );
    assert_eq!(
        text(&term, 7).as_deref(),
        Some(""),
        "the card's last is the tail"
    );
    assert_eq!(text(&term, 8), None);
    assert_eq!(term.window_cursor(), Some((15, 0)));
    assert_eq!(
        term.cursor(),
        Some((7, 0)),
        "the card's cursor, on its own rows"
    );
    assert_eq!(term.window_stamp(0), Some(5));
    assert_eq!(term.stamp(0), term.window_stamp(8));
    // Held: the anchor is the window's end on both readings.
    term.scroll_up(4);
    assert_eq!(window(&term, 15).as_deref(), Some("line 27"));
    assert_eq!(text(&term, 7).as_deref(), Some("line 27"));
    assert_eq!(term.cursor(), None);
    // Clamped to the screen: the parser's view is all there is to read.
    term.set_window(100);
    assert_eq!(term.window(), usize::from(Term::ROWS));
    term.set_window(0);
    assert_eq!(term.window(), 1);
    assert_eq!(
        text(&term, 0),
        term.window_row(0).map(|_| "line 27".to_owned())
    );
    assert_eq!(text(&term, 1), None, "a one-row window is a one-row card");
    // Back to the card's eight: the same screen, the anchor kept.
    term.set_window(Term::PANE_ROWS);
    assert_eq!(text(&term, 7).as_deref(), Some("line 27"));
    assert!(!term.is_following());
}

/// The window is part of the screen a term equals and clones by.
#[test]
fn the_window_rides_a_clone_and_tells_terms_apart() {
    let mut term = Term::new();
    term.push(&lines(1, 30), 1);
    let card = term.clone();
    term.set_window(12);
    assert_ne!(card, term);
    let dock = term.clone();
    assert_eq!(dock, term);
    assert_eq!(dock.window(), 12);
    assert_eq!(
        dock.window_row(0).map(|c| c[0].text.clone()),
        term.window_row(0).map(|c| c[0].text.clone())
    );
}

#[test]
fn the_rail_reads_the_arrival_ticks_from_the_programs_first_byte() {
    let mut term = Term::new();
    assert_eq!(term.started(), None);
    assert_eq!(term.mark(0), None, "nothing written, no rail");
    term.push(b"one\r\ntwo\r\n", 100);
    term.push(b"three\r\n", 105);
    // Well over a second later, then a row in the same second.
    term.push(b"four\r\n", 140);
    term.push(b"five", 142);
    assert_eq!(term.started(), Some(100));
    let first = term.mark(0).expect("the first row");
    assert!(first.tick);
    assert_eq!(first.stamp, Some(0), "the origin is stamped");
    assert!(term.mark(1).expect("two").is_quiet());
    assert!(term.mark(2).expect("three").is_quiet());
    let four = term.mark(3).expect("four");
    assert!(four.silence, "40 ticks of silence break the rail");
    assert_eq!(four.stamp, Some(2));
    assert!(term.mark(4).expect("five").is_quiet());
    assert_eq!(term.mark(5), None, "past the content");
    assert_eq!(term.mark(Term::PANE_ROWS), None);
    // The window's reading is the card's on a card-sized window.
    assert_eq!(term.window_mark(3), term.mark(3));
    // A clone reads the same rail.
    let copy = term.clone();
    assert_eq!(copy.started(), Some(100));
    assert_eq!(copy.mark(3), term.mark(3));
}

#[test]
fn the_programs_progress_report_is_read_and_cleared_at_the_end() {
    use crate::cockpit::Progress;
    let mut term = Term::new();
    assert_eq!(term.progress(), Progress::Cleared);
    term.push(b"building\r\n\x1b]9;4;1;40\x07", 1);
    assert_eq!(term.progress(), Progress::Running(40));
    assert_eq!(
        text(&term, 0).as_deref(),
        Some("building"),
        "the report paints nothing"
    );
    let copy = term.clone();
    assert_eq!(
        copy.progress(),
        Progress::Running(40),
        "replayed with the bytes"
    );
    term.push(b"\x1b]9;4;2;40\x07", 2);
    assert_eq!(term.progress(), Progress::Failed(40));
    term.finish();
    assert_eq!(
        term.progress(),
        Progress::Cleared,
        "a settled card has no bar"
    );
}

/// A kill is a finish the pane can name: the screen freezes as on any
/// end, the mark carries through a clone and tells two terms apart.
#[test]
fn a_kill_is_a_named_finish_that_clones_and_compares() {
    let mut term = Term::new();
    term.push(b"building\r\n", 1);
    let mut same = term.clone();
    assert!(!term.is_killed());
    term.kill();
    assert!(term.is_killed());
    assert!(term.is_finished(), "a killed program ended");
    assert_eq!(term.cursor(), None, "frozen");
    assert_ne!(term, same, "killed and not are two states");
    same.kill();
    assert_eq!(term, same);
    assert!(term.clone().is_killed());
}

/// The transcript is everything the program wrote, stripped: the rows
/// above the screen and the screen's own, trailing blanks off every
/// row, empty rows off the end; the reader's window is left where it
/// was.
#[test]
fn the_transcript_is_the_stripped_scrollback_and_screen() {
    let mut term = Term::new();
    assert_eq!(term.transcript(), "", "nothing written");
    term.push(&lines(1, 30), 1);
    term.push(b"\x1b[1;32m   Finished\x1b[0m release   \r\n", 2);
    term.scroll_up(3);
    let offset = term.offset();
    let text = term.transcript();
    let rows: Vec<&str> = text.lines().collect();
    assert_eq!(rows.len(), 31, "{text:?}");
    assert_eq!(rows[0], "line 1", "from the top of the scrollback");
    assert_eq!(rows[29], "line 30");
    assert_eq!(
        rows[30], "   Finished release",
        "escapes and trailing blanks gone"
    );
    assert!(!text.ends_with('\n'));
    assert_eq!(term.offset(), offset, "the reader's place is untouched");
}
