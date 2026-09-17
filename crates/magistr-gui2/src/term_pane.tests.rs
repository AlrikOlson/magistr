use magistr_cockpit::theme::Pen;
use magistr_theme::math::contrast_ratio;

use super::*;
use crate::element::Kind;
use crate::medium::Medium;

fn bytes(c: Srgba) -> (u8, u8, u8) {
    let b = |x: f32| (x * 255.0).round().clamp(0.0, 255.0) as u8;
    (b(c.r), b(c.g), b(c.b))
}

/// A term with `n` numbered rows written, a cursor on the last.
fn written(n: usize) -> Term {
    let mut term = Term::new();
    let rows: String = (1..=n).map(|i| format!("row {i}\r\n")).collect();
    term.push(rows.trim_end_matches("\r\n").as_bytes(), 0);
    term
}

/// The Cells leaf under `pane`, as its rows.
fn cells(pane: &Element) -> Vec<Vec<PaneTile>> {
    let mut found = None;
    pane.each(&mut |_, element| {
        if let Kind::Grid { rows, .. } = &element.kind {
            found = Some(rows.clone());
        }
    });
    found.expect("the pane carries a Cells leaf")
}

fn texts(pane: &Element) -> Vec<String> {
    pane.runs().into_iter().map(|run| run.text).collect()
}

/// The program's colours are the medium's: an ANSI red is the
/// lexicon's red, an ANSI 4 its blue, a bright white the text ink, a
/// cube entry xterm's own and truecolor itself — and the default ink
/// is the text on the well.
#[test]
fn colours_resolve_through_the_lexicon_the_cube_and_truecolor() {
    for medium in [Medium::Night, Medium::Day] {
        let lex = Lexicon::of(medium);
        let well = lex.base;
        let cell = |fg: Color, bg: Color, attrs: Attrs| TermCell {
            text: "a".to_owned(),
            fg,
            bg,
            attrs,
        };
        let red = TermPane::resolve(
            &cell(Color::Indexed(1), Color::Default, Attrs::NONE),
            well,
            &lex,
        );
        assert_eq!(bytes(red.ink), bytes(lex.red), "{medium:?}");
        assert_eq!(red.ground, None, "the default ground is the well");
        let blue_on_black = TermPane::resolve(
            &cell(Color::Indexed(4), Color::Indexed(0), Attrs::NONE),
            well,
            &lex,
        );
        assert_eq!(blue_on_black.ground.map(bytes), Some(bytes(lex.s0)));
        let white = TermPane::resolve(
            &cell(Color::Indexed(15), Color::Default, Attrs::NONE),
            well,
            &lex,
        );
        assert_eq!(bytes(white.ink), bytes(lex.tx));
        let plain = TermPane::resolve(
            &cell(Color::Default, Color::Default, Attrs::NONE),
            well,
            &lex,
        );
        assert_eq!(bytes(plain.ink), bytes(lex.tx), "default ink is the text");
        let cube = TermPane::resolve(
            &cell(Color::Indexed(196), Color::Default, Attrs::NONE),
            well,
            &lex,
        );
        assert_eq!(
            bytes(cube.ink),
            (255, 0, 0),
            "the cube is xterm's on both media"
        );
        let peach = TermPane::resolve(
            &cell(Color::Rgb(250, 179, 135), Color::Default, Attrs::NONE),
            well,
            &lex,
        );
        let theme = |c: Srgba| {
            let (r, g, b) = bytes(c);
            ThemeColor::new(r, g, b)
        };
        match medium {
            // Peach clears the floor on the night ground: it is itself.
            Medium::Night => assert_eq!(bytes(peach.ink), (250, 179, 135)),
            // On the day ground it does not, and is deepened until it
            // does — truecolor through the floor, not around it.
            Medium::Day => {
                assert_ne!(bytes(peach.ink), (250, 179, 135));
                assert!(contrast_ratio(theme(peach.ink), theme(well)) >= Pen::MIN_CONTRAST);
            }
        }
    }
}

/// Reverse video swaps ink and ground before the floor — on the well
/// when the program named no ground — and is spent there; bold, italic
/// and underline ride through as the cuts and the rule.
#[test]
fn reverse_swaps_and_the_attributes_ride_through() {
    let lex = Lexicon::of(Medium::Night);
    let well = lex.base;
    let reversed = TermPane::resolve(
        &TermCell {
            text: "x".to_owned(),
            fg: Color::Indexed(2),
            bg: Color::Default,
            attrs: Attrs::REVERSE | Attrs::BOLD | Attrs::ITALIC | Attrs::UNDERLINE,
        },
        well,
        &lex,
    );
    assert_eq!(reversed.ground.map(bytes), Some(bytes(lex.green)));
    assert_eq!(bytes(reversed.ink), bytes(TermPane::floor(well, lex.green)));
    assert!(reversed.bold && reversed.italic && reversed.underline);
}

/// A dim grey on the night ground would sink under the floor; the pane
/// lifts it — and a red on a red ground is lifted too — so every cell
/// of a real screen clears the terminal pen's ratio in both media.
#[test]
fn every_cell_clears_the_contrast_floor() {
    let mut term = Term::new();
    term.push(
        b"\x1b[2m\x1b[38;5;236mdim grey\x1b[0m \x1b[31;41mred on red\x1b[0m \
          \x1b[38;2;30;30;40mnear black\x1b[0m \x1b[97;107mwhite on white\x1b[0m \
          \x1b[1;32m   Compiling\x1b[0m done",
        0,
    );
    for medium in [Medium::Night, Medium::Day] {
        let lex = Lexicon::of(medium);
        let pane = TermPane::el(&term, 0, true, lex.mantle, &lex);
        let well = Lexicon::composite(
            lex.mantle,
            Srgba::new(lex.base.r, lex.base.g, lex.base.b, TermPane::WELL),
        );
        let mut checked = 0;
        for row in cells(&pane) {
            for cell in row.iter().filter(|c| !c.is_quiet()) {
                let on = cell.ground.unwrap_or(well);
                let theme = |c: Srgba| {
                    let (r, g, b) = bytes(c);
                    ThemeColor::new(r, g, b)
                };
                let ratio = contrast_ratio(theme(cell.ink), theme(on));
                assert!(
                    ratio + 1e-9 >= Pen::MIN_CONTRAST,
                    "{medium:?} {:?} ink {:?} on {:?}: {ratio:.2}",
                    cell.text,
                    bytes(cell.ink),
                    bytes(on)
                );
                checked += 1;
            }
        }
        assert!(checked > 30, "{checked} cells swept");
    }
}

/// The pane is eight rows of the program's screen; the cursor is a
/// block in the text ink on the row the cursor is on while the phase
/// is on, and gone on the off phase; under reduced motion it stands;
/// a finished pane has none.
#[test]
fn the_cursor_blinks_on_ticks_and_stands_under_reduced_motion() {
    let lex = Lexicon::of(Medium::Night);
    let term = written(3);
    let (row, col) = term.cursor().expect("a live pane has a cursor");
    let block = |pane: &Element| {
        let rows = cells(pane);
        assert_eq!(rows.len(), Term::PANE_ROWS);
        rows[row][TermPane::RAIL + usize::from(col)]
            .ground
            .map(bytes)
            == Some(bytes(lex.tx))
    };
    assert!(TermPane::cursor_on(0, false));
    assert!(!TermPane::cursor_on(TermPane::BLINK_STEPS / 2, false));
    assert!(TermPane::cursor_on(TermPane::BLINK_STEPS, false));
    assert!(TermPane::cursor_on(TermPane::BLINK_STEPS / 2, true));
    assert!(block(&TermPane::el(&term, 0, false, lex.mantle, &lex)));
    assert!(!block(&TermPane::el(
        &term,
        TermPane::BLINK_STEPS / 2,
        false,
        lex.mantle,
        &lex
    )));
    assert!(block(&TermPane::el(
        &term,
        TermPane::BLINK_STEPS / 2,
        true,
        lex.mantle,
        &lex
    )));
    let mut finished = written(3);
    finished.finish();
    assert!(!block(&TermPane::el(&finished, 0, true, lex.mantle, &lex)));
}

/// While the pane follows there is no pill; scrolled up, the pill
/// counts the rows that arrived, and the thumb appears once the
/// program has written more than the pane holds.
#[test]
fn the_pill_counts_arrivals_and_the_thumb_sizes_the_scrollback() {
    let lex = Lexicon::of(Medium::Night);
    let short = written(3);
    let pane = TermPane::el(&short, 0, true, lex.mantle, &lex);
    assert!(texts(&pane).is_empty(), "no pill while following");
    assert_eq!(
        pane.count(),
        3,
        "well · row · cells — no thumb under eight rows"
    );

    let mut long = written(30);
    let pane = TermPane::el(&long, 0, true, lex.mantle, &lex);
    assert!(texts(&pane).is_empty(), "still following");
    assert_eq!(pane.count(), 7, "well · row · cells · track · three shares");

    long.scroll_up(5);
    long.push(b"\r\nrow 31\r\nrow 32", 1);
    let pane = TermPane::el(&long, 0, true, lex.mantle, &lex);
    let texts = texts(&pane);
    assert_eq!(
        texts,
        [format!("{} Latest \u{b7} +2", Icons::UNICODE.latest)]
    );
    let owners: Vec<_> = pane.runs().into_iter().map(|r| r.owner).collect();
    assert_eq!(owners, [Some(Component::Chip)]);
    assert!(
        cells(&pane).iter().all(|row| row
            .iter()
            .all(|c| c.ground.map(bytes) != Some(bytes(lex.tx)))),
        "a held pane shows no cursor"
    );
}

/// The swatch exhibits the sixteen grounds and inks and every
/// attribute — three rows, all through the same resolution.
#[test]
fn the_swatch_holds_the_sixteen_grounds_and_inks() {
    let lex = Lexicon::of(Medium::Day);
    let rows = cells(&TermPane::swatch(lex.mantle, &lex));
    assert_eq!(rows.len(), 3);
    let grounds: Vec<_> = rows[0]
        .iter()
        .filter_map(|c| c.ground.map(bytes))
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();
    assert_eq!(
        grounds.len(),
        10,
        "ten distinct grounds: two blacks, six hues (the bright six are the same), two whites"
    );
    assert!(rows[2].iter().any(|c| c.bold));
    assert!(rows[2].iter().any(|c| c.italic));
    assert!(rows[2].iter().any(|c| c.underline));
}

/// The dock's pane is the terminal's whole window — widened, its rows reach further up
/// than the card's eight — cut to the rows a rise has reached, tail first, with the
/// cursor told against those rows; the thumb sizes against what is shown.
#[test]
fn the_window_pane_reaches_up_and_is_cut_to_the_rows_the_rise_reached() {
    let lex = Lexicon::of(Medium::Night);
    let mut term = written(30);
    term.set_window(16);
    let whole = TermPane::window(&term, 16, 0, true, lex.mantle, &lex);
    let rows = cells(&whole);
    assert_eq!(rows.len(), 16);
    // Past the rail's tiles, the program's own.
    let text = |row: &Vec<PaneTile>| {
        row.iter()
            .skip(TermPane::RAIL)
            .map(|c| c.text.as_str())
            .collect::<String>()
            .trim_end()
            .to_owned()
    };
    assert_eq!(text(&rows[0]), "row 15", "sixteen rows up from the tail");
    assert_eq!(text(&rows[14]), "row 29");
    assert!(
        rows[15]
            .iter()
            .any(|c| c.ground.map(bytes) == Some(bytes(lex.tx))),
        "the cursor block stands on the window's last row"
    );
    // Cut to five rows: the tail again, the cursor on its last row.
    let cut = TermPane::window(&term, 5, 0, true, lex.mantle, &lex);
    let rows = cells(&cut);
    assert_eq!(rows.len(), 5);
    assert_eq!(text(&rows[0]), "row 26");
    assert!(
        rows[4]
            .iter()
            .any(|c| c.ground.map(bytes) == Some(bytes(lex.tx))),
        "the cursor moved with the cut"
    );
    // The card's own pane is the window's last eight, unchanged.
    let card = cells(&TermPane::el(&term, 0, true, lex.mantle, &lex));
    assert_eq!(card.len(), Term::PANE_ROWS);
    assert_eq!(text(&card[0]), "row 23");
    assert_eq!(
        whole.count(),
        7,
        "well · row · cells · track · three shares — the thumb sizes against the window"
    );
}
