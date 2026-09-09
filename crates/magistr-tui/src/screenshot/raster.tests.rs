use super::*;
use magistr_cockpit::render::{Color, Style};
use magistr_cockpit::theme::ThemeMode;

const FG: [u8; 3] = [255, 0, 0];
const BG: [u8; 3] = [0, 0, 40];

fn palette() -> Palette {
    ThemeMode::Dark.palette()
}

fn style() -> Style {
    Style::new()
        .fg(Color::Rgb(255, 0, 0))
        .bg(Color::Rgb(0, 0, 40))
}

/// A one-row buffer holding `cells`, rasterized.
fn render(cells: &[Cell]) -> Pixmap {
    let mut buffer = Buffer::new(u16::try_from(cells.len()).expect("cols"), 1);
    for (col, cell) in cells.iter().enumerate() {
        buffer.set(u16::try_from(col).expect("col"), 0, cell.clone());
    }
    rasterize(&buffer, &palette(), &mut Typeface::new(), &[])
}

fn px(pixmap: &Pixmap, x: u32, y: u32) -> [u8; 3] {
    let p = pixmap.pixel(x, y).expect("on the pixmap");
    [p.red(), p.green(), p.blue()]
}

/// Every pixel of the cell at `col` that is not the background.
fn lit(pixmap: &Pixmap, col: u32) -> Vec<(u32, u32)> {
    let mut out = Vec::new();
    for y in 0..20 {
        for x in col * 10..col * 10 + 10 {
            if px(pixmap, x, y) != BG {
                out.push((x, y));
            }
        }
    }
    out
}

#[test]
fn an_empty_buffer_is_one_pixel_and_a_cell_is_ten_by_twenty() {
    let empty = rasterize(&Buffer::new(0, 0), &palette(), &mut Typeface::new(), &[]);
    assert_eq!((empty.width(), empty.height()), (1, 1));
    let two = render(&[Cell::blank(), Cell::blank()]);
    assert_eq!((two.width(), two.height()), (20, 20));
}

#[test]
fn a_blank_cell_is_the_palettes_base_edge_to_edge() {
    let pixmap = render(&[Cell::blank()]);
    let base = palette().base;
    for y in 0..20 {
        for x in 0..10 {
            assert_eq!(px(&pixmap, x, y), [base.r, base.g, base.b], "({x},{y})");
        }
    }
}

#[test]
fn a_glyph_lights_foreground_pixels_only_inside_its_cell() {
    let pixmap = render(&[Cell::new("M", style()), Cell::new(" ", style())]);
    let inside = lit(&pixmap, 0);
    assert!(!inside.is_empty(), "M draws something");
    assert!(
        inside.iter().any(|&(x, y)| px(&pixmap, x, y) == FG),
        "a solid red pixel"
    );
    assert!(
        lit(&pixmap, 1).is_empty(),
        "the space beside it stays background"
    );
}

#[test]
fn reverse_swaps_the_colours_and_dim_pulls_toward_the_background() {
    let reversed = render(&[Cell::new(" ", style().attrs(Attrs::REVERSE))]);
    assert_eq!(
        px(&reversed, 0, 0),
        FG,
        "the background became the foreground"
    );
    let plain = render(&[Cell::new("M", style())]);
    let dim = render(&[Cell::new("M", style().attrs(Attrs::DIM))]);
    let red = |p: &Pixmap| {
        lit(p, 0)
            .iter()
            .map(|&(x, y)| u32::from(px(p, x, y)[0]))
            .max()
    };
    let (plain_red, dim_red) = (red(&plain).expect("lit"), red(&dim).expect("lit"));
    assert!(dim_red < plain_red, "dim {dim_red} < {plain_red}");
    assert!(dim_red > u32::from(BG[0]), "dim is still visible");
}

#[test]
fn underline_and_strikethrough_are_rules_across_the_cell() {
    let mut face = Typeface::new();
    let baseline = face.baseline();
    let _ = face.glyph(' ', false);
    let under = render(&[Cell::new(" ", style().attrs(Attrs::UNDERLINE))]);
    for x in 0..10 {
        assert_eq!(px(&under, x, baseline + 2), FG, "underline at x={x}");
    }
    assert_eq!(
        px(&under, 5, baseline),
        BG,
        "nothing on the baseline itself"
    );
    let strike = render(&[Cell::new(" ", style().attrs(Attrs::STRIKETHROUGH))]);
    let row = baseline - baseline / 3;
    for x in 0..10 {
        assert_eq!(px(&strike, x, row), FG, "strikethrough at x={x}");
    }
}

#[test]
fn bold_is_heavier_and_italic_leans_right_at_the_top() {
    let weight = |p: &Pixmap| -> u32 {
        lit(p, 0)
            .iter()
            .map(|&(x, y)| u32::from(px(p, x, y)[0]))
            .sum()
    };
    let regular = render(&[Cell::new("M", style())]);
    let bold = render(&[Cell::new("M", style().attrs(Attrs::BOLD))]);
    assert!(weight(&bold) > weight(&regular));
    let italic = render(&[Cell::new("l", style().attrs(Attrs::ITALIC))]);
    let upright = render(&[Cell::new("l", style())]);
    let top_left = |p: &Pixmap| {
        lit(p, 0)
            .iter()
            .filter(|&&(_, y)| y < 8)
            .map(|&(x, _)| x)
            .min()
    };
    assert!(
        top_left(&italic) > top_left(&upright),
        "{:?} vs {:?}",
        top_left(&italic),
        top_left(&upright)
    );
}

#[test]
fn a_wide_grapheme_without_a_glyph_is_a_box_over_both_cells() {
    // Only the head is written: the buffer lays the continuation cell
    // itself (writing over it would blank the head).
    let cell = Cell::new("中", style());
    assert_eq!(cell.width(), 2);
    let mut buffer = Buffer::new(2, 1);
    buffer.set(0, 0, cell);
    let pixmap = rasterize(&buffer, &palette(), &mut Typeface::new(), &[]);
    assert_eq!(pixmap.width(), 20);
    // The box's top edge runs across both cells at y=2; its interior is background.
    assert_ne!(
        px(&pixmap, 5, 2),
        BG,
        "the box's top edge in the first cell"
    );
    assert_ne!(px(&pixmap, 15, 2), BG, "…and in the second");
    assert_eq!(px(&pixmap, 10, 10), BG, "hollow inside");
    assert_ne!(px(&pixmap, 18, 10), BG, "the right edge in the second cell");
}

#[test]
fn a_private_use_glyph_is_a_box_and_a_space_never_is() {
    let tofu = render(&[Cell::new("\u{e000}", style())]);
    assert!(!lit(&tofu, 0).is_empty());
    let space = render(&[Cell::new(" ", style())]);
    assert!(lit(&space, 0).is_empty());
}

#[test]
fn rasterizing_the_same_buffer_twice_is_byte_identical() {
    let cells = [
        Cell::new("a", style().attrs(Attrs::BOLD | Attrs::ITALIC)),
        Cell::new("─", style().attrs(Attrs::DIM)),
        Cell::new("x", style().attrs(Attrs::UNDERLINE | Attrs::STRIKETHROUGH)),
    ];
    assert_eq!(render(&cells).data(), render(&cells).data());
}

#[test]
fn the_bars_filled_cell_is_solid_and_its_empty_cell_is_hollow() {
    // Both parallelograms come from the fallback face; the
    // filled one is its twin with the middle inked in, so the empty
    // one's lit pixels are a strict subset of the filled one's and
    // there are far fewer of them. Before the fallback both drew as the
    // same hollow box and a fill glide was a strip of identical frames.
    let full = render(&[Cell::new("▰", style())]);
    let empty = render(&[Cell::new("▱", style())]);
    let (inked, outline) = (lit(&full, 0), lit(&empty, 0));
    assert!(
        !outline.is_empty(),
        "the empty cell still draws its outline"
    );
    assert!(
        inked.len() > outline.len(),
        "a filled bar cell lights more than an empty one: {} vs {}",
        inked.len(),
        outline.len()
    );
    for pixel in &outline {
        assert!(
            inked.contains(pixel),
            "{pixel:?} is outline, so the filled cell lights it too"
        );
    }
    assert_ne!(
        full.data(),
        empty.data(),
        "a bar's fill has to be visible, or a fill glide is a strip of identical frames"
    );
}

/// The lit pixels of the cell at column 0, by row.
fn rule_rows(pixmap: &Pixmap) -> Vec<(u32, Vec<u32>)> {
    let mut rows: Vec<(u32, Vec<u32>)> = Vec::new();
    for y in 0..20 {
        let xs: Vec<u32> = (0..10).filter(|&x| px(pixmap, x, y) != BG).collect();
        if !xs.is_empty() {
            rows.push((y, xs));
        }
    }
    rows
}

#[test]
fn the_four_styles_draw_four_distinguishable_rules() {
    let drawn: Vec<Pixmap> = [
        Attrs::UNDERLINE,
        Attrs::UNDERLINE_DOUBLE,
        Attrs::UNDERLINE_CURLY,
        Attrs::UNDERLINE_DOTTED,
        Attrs::UNDERLINE_DASHED,
    ]
    .into_iter()
    .map(|attrs| render(&[Cell::new(" ", style().attrs(attrs))]))
    .collect();
    for (i, a) in drawn.iter().enumerate() {
        assert!(!lit(a, 0).is_empty(), "style {i} draws a rule");
        for (j, b) in drawn.iter().enumerate().skip(i + 1) {
            assert_ne!(a.data(), b.data(), "style {i} and style {j} look the same");
        }
    }
}

#[test]
fn a_double_rule_is_two_rows_and_a_curly_one_waves() {
    let double = rule_rows(&render(&[Cell::new(
        " ",
        style().attrs(Attrs::UNDERLINE_DOUBLE),
    )]));
    assert_eq!(double.len(), 2, "two rules: {double:?}");
    assert!(
        double[1].0 - double[0].0 == 2,
        "with a gap between them: {double:?}"
    );
    for (_, xs) in &double {
        assert_eq!(xs.len(), 10, "each running the width of the cell");
    }
    let curly = rule_rows(&render(&[Cell::new(
        " ",
        style().attrs(Attrs::UNDERLINE_CURLY),
    )]));
    assert_eq!(curly.len(), 3, "a wave touches three rows: {curly:?}");
    let total: usize = curly.iter().map(|(_, xs)| xs.len()).sum();
    assert_eq!(total, 10, "one pixel per column, all the same wave");
}

#[test]
fn dotted_and_dashed_rules_are_gapped_single_rows() {
    for (attrs, name) in [
        (Attrs::UNDERLINE_DOTTED, "dotted"),
        (Attrs::UNDERLINE_DASHED, "dashed"),
    ] {
        let rows = rule_rows(&render(&[Cell::new(" ", style().attrs(attrs))]));
        assert_eq!(rows.len(), 1, "{name} is one row: {rows:?}");
        let xs = &rows[0].1;
        assert!(!xs.is_empty() && xs.len() < 10, "{name} has gaps: {xs:?}");
    }
}

#[test]
fn a_rule_colour_paints_the_underline_and_leaves_the_text_alone() {
    const RULE: [u8; 3] = [0, 200, 0];
    let cell = Cell::new(
        "M",
        style()
            .attrs(Attrs::UNDERLINE_CURLY | Attrs::STRIKETHROUGH)
            .underline_color(Color::Rgb(0, 200, 0)),
    );
    let pixmap = render(&[cell]);
    let painted: Vec<[u8; 3]> = lit(&pixmap, 0)
        .iter()
        .map(|&(x, y)| px(&pixmap, x, y))
        .collect();
    assert!(painted.contains(&RULE), "the rule is drawn in its colour");
    assert!(
        painted.contains(&FG),
        "the glyph and the strikethrough stay the text colour"
    );
}

#[test]
fn two_neighbouring_rules_share_one_unbroken_run() {
    // the face advances 9 px inside the 10 px cell, so a ring
    // drawn from its ─ showed a column of background at every cell
    // boundary. Synthesized, the run crosses the seam.
    let pixmap = render(&[
        Cell::new("─", style()),
        Cell::new("─", style()),
        Cell::new("╮", style()),
    ]);
    let row = (0..20)
        .find(|&y| px(&pixmap, 0, y) == FG)
        .expect("the rule inks a row");
    for x in 0..20 {
        assert_eq!(px(&pixmap, x, row), FG, "({x},{row}) is on the run");
    }
    // And the corner picks the run up on the same row, edge to edge.
    assert_eq!(px(&pixmap, 20, row), FG, "the arc's arm meets the run");
}

#[test]
fn a_corner_meets_the_stile_below_it_across_the_row_seam() {
    let pixmap = render(&[Cell::new("╭", style())]);
    let mut buffer = Buffer::new(1, 2);
    buffer.set(0, 0, Cell::new("╭", style()));
    buffer.set(0, 1, Cell::new("│", style()));
    let two = rasterize(&buffer, &palette(), &mut Typeface::new(), &[]);
    let col = (0..10)
        .find(|&x| px(&pixmap, x, 19) == FG)
        .expect("the arc's down arm reaches the bottom");
    for y in 15..25 {
        assert_eq!(px(&two, col, y), FG, "({col},{y}) is on the stile");
    }
}

#[test]
fn a_synthesized_glyph_takes_neither_the_bold_face_nor_the_italic_shear() {
    // Box drawing is geometry: the heavy code points are how the
    // vocabulary asks for a thicker line, so an attribute must not
    // move or fatten one.
    let plain = render(&[Cell::new("┼", style())]);
    let loud = render(&[Cell::new("┼", style().attrs(Attrs::BOLD | Attrs::ITALIC))]);
    assert_eq!(plain.data(), loud.data());
}
