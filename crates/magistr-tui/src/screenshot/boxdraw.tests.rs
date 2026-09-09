use super::*;

/// The synthesized bitmap for `ch`, which must exist.
fn cov(ch: char) -> Vec<u8> {
    let g = glyph(ch).unwrap_or_else(|| panic!("{ch:?} is synthesized"));
    assert_eq!((g.left, g.top), (0, 0), "{ch:?} is drawn in its own cell");
    assert_eq!((g.width, g.height), (W, H), "{ch:?} fills the cell");
    g.coverage
}

fn at(c: &[u8], x: usize, y: usize) -> u8 {
    c[y * W + x]
}

/// The rows that hold any ink.
fn rows(c: &[u8]) -> Vec<usize> {
    (0..H)
        .filter(|&y| (0..W).any(|x| at(c, x, y) > 0))
        .collect()
}

/// The columns that hold any ink.
fn cols(c: &[u8]) -> Vec<usize> {
    (0..W)
        .filter(|&x| (0..H).any(|y| at(c, x, y) > 0))
        .collect()
}

fn ink(c: &[u8]) -> u32 {
    c.iter().map(|&v| u32::from(v)).sum()
}

#[test]
fn a_light_rule_is_one_solid_row_from_edge_to_edge() {
    // The whole point of the chunk: the face advances 9 px in a 10 px
    // cell, so its ─ left a column of background at every cell boundary.
    // A synthesized one reaches both edges, so two neighbours share one
    // unbroken run.
    let c = cov('─');
    assert_eq!(rows(&c), vec![9], "a light rule is one row tall");
    for x in 0..W {
        assert_eq!(at(&c, x, 9), 255, "({x},9) is solid");
    }
}

#[test]
fn a_light_stile_is_one_solid_column_from_top_to_bottom() {
    let c = cov('│');
    assert_eq!(cols(&c), vec![4], "a light stile is one column wide");
    for y in 0..H {
        assert_eq!(at(&c, 4, y), 255, "(4,{y}) is solid");
    }
}

#[test]
fn heavy_is_twice_the_light_stroke_and_shares_its_near_edge() {
    assert_eq!(rows(&cov('━')), vec![9, 10]);
    assert_eq!(cols(&cov('┃')), vec![4, 5]);
    assert_eq!(ink(&cov('━')), 2 * ink(&cov('─')));
}

#[test]
fn a_cross_is_the_union_of_a_rule_and_a_stile() {
    let (cross, rule, stile) = (cov('┼'), cov('─'), cov('│'));
    for i in 0..W * H {
        assert_eq!(cross[i], rule[i].max(stile[i]), "cell {i}");
    }
}

#[test]
fn a_corner_reaches_the_two_edges_its_arms_point_at_and_no_others() {
    // ┌ is down and right: solid at the right edge on the rule's row and
    // at the bottom edge on the stile's column, empty at the other two.
    let c = cov('┌');
    assert_eq!(at(&c, W - 1, 9), 255, "the right arm reaches the edge");
    assert_eq!(at(&c, 4, H - 1), 255, "the down arm reaches the edge");
    assert_eq!(at(&c, 0, 9), 0, "nothing points left");
    assert_eq!(at(&c, 4, 0), 0, "nothing points up");
}

#[test]
fn a_tee_carries_three_arms_and_a_half_line_carries_one() {
    let tee = cov('├');
    assert_eq!(at(&tee, 4, 0), 255);
    assert_eq!(at(&tee, 4, H - 1), 255);
    assert_eq!(at(&tee, W - 1, 9), 255);
    assert_eq!(at(&tee, 0, 9), 0, "a right tee points nowhere left");

    let half = cov('╴');
    assert_eq!(at(&half, 0, 9), 255, "the left half reaches the left edge");
    assert_eq!(at(&half, W - 1, 9), 0, "and stops at the middle");
    assert!(ink(&half) * 2 <= ink(&cov('─')) + 255, "about half a rule");
}

#[test]
fn a_mixed_junction_takes_each_arms_own_weight() {
    // ┍ is down light and right heavy: the right arm is two rows deep,
    // the down arm one column wide.
    let c = cov('┍');
    assert_eq!(at(&c, W - 1, 9), 255);
    assert_eq!(at(&c, W - 1, 10), 255, "the right arm is heavy");
    assert_eq!(at(&c, 5, H - 1), 0, "the down arm stays light");
    assert_eq!(at(&c, 4, H - 1), 255);
    assert!(cols(&c).contains(&9), "and it reaches the right edge");
}

#[test]
fn a_dashed_rule_breaks_but_keeps_its_row() {
    // Three or four dashes across ten pixels is finer than the pixel
    // grid, so the break is a drop in coverage rather than a bare cell —
    // which is what a real terminal shows at this size too.
    for ch in ['┄', '┈', '╌'] {
        let c = cov(ch);
        assert_eq!(rows(&c), vec![9], "{ch:?} is a light rule's row");
        assert!(
            (0..W).any(|x| at(&c, x, 9) < 255),
            "{ch:?} breaks the run somewhere"
        );
        assert!(ink(&c) < ink(&cov('─')), "{ch:?} inks less than a rule");
    }
    // The cell is twice as tall as it is wide, so a vertical dash has
    // room for a genuinely empty gap.
    for ch in ['┆', '┊', '╎'] {
        let c = cov(ch);
        assert_eq!(cols(&c), vec![4], "{ch:?} is a light stile's column");
        assert!((0..H).any(|y| at(&c, 4, y) == 0), "{ch:?} leaves a gap");
    }
    assert!(
        ink(&cov('┄')) > ink(&cov('┈')),
        "more dashes across one cell means more gaps"
    );
}

#[test]
fn an_arc_leaves_a_whole_pixel_of_straight_run_at_each_edge() {
    // The acceptance: ╭ meets a ─ to its right and a │ below it on solid
    // pixels, not on the curve's tangent — and it points nowhere else.
    let c = cov('╭');
    assert_eq!(at(&c, W - 1, 9), 255, "the rule's row is solid at the edge");
    assert_eq!(at(&c, W - 2, 9), 255, "and a whole pixel before it");
    assert_eq!(at(&c, 4, H - 1), 255, "the stile's column at the bottom");
    assert_eq!(at(&c, 4, H - 2), 255);
    assert_eq!(at(&c, 0, 9), 0, "nothing points left");
    assert_eq!(at(&c, 4, 0), 0, "nothing points up");
}

#[test]
fn each_arc_points_at_its_own_two_edges() {
    // The four arcs are not pixel reflections of one another: an odd
    // stroke cannot sit on the centre of an even-width cell, so the
    // stile rides column 4 of 10 and the rule row 9 of 20 whichever way
    // the corner turns. What is symmetric is which edges they reach.
    for (ch, up, right, down, left) in [
        ('╭', false, true, true, false),
        ('╮', false, false, true, true),
        ('╰', true, true, false, false),
        ('╯', true, false, false, true),
    ] {
        let c = cov(ch);
        assert_eq!(at(&c, 4, 0) > 0, up, "{ch:?} up");
        assert_eq!(at(&c, W - 1, 9) > 0, right, "{ch:?} right");
        assert_eq!(at(&c, 4, H - 1) > 0, down, "{ch:?} down");
        assert_eq!(at(&c, 0, 9) > 0, left, "{ch:?} left");
    }
}

#[test]
fn an_arc_is_rounder_than_the_corner_it_replaces() {
    // A sharp corner inks its two arms in full; the arc cuts the elbow
    // out and lays a curve across it, so it inks strictly less. (It is
    // circular rather than elliptical on purpose — kitty's #3409 is an
    // elliptical arc stretching on a tall cell, and this cell is 10x20.)
    assert!(ink(&cov('╭')) < ink(&cov('┌')));
    assert_eq!(at(&cov('╭'), 4, 9), 0, "the elbow itself is cut away");
}

#[test]
fn a_diagonal_crosses_its_own_corners_and_the_cross_is_both() {
    let slash = cov('╱');
    assert!(at(&slash, 0, H - 1) > 0, "╱ starts at the bottom left");
    assert!(at(&slash, W - 1, 0) > 0, "and ends at the top right");
    assert_eq!(at(&slash, 0, 0), 0);
    let back = cov('╲');
    assert!(at(&back, 0, 0) > 0, "╲ starts at the top left");
    assert!(at(&back, W - 1, H - 1) > 0);
    let cross = cov('╳');
    for i in 0..W * H {
        assert_eq!(cross[i], slash[i].max(back[i]), "cell {i}");
    }
}

#[test]
fn a_full_block_covers_every_pixel() {
    assert!(cov('█').iter().all(|&v| v == 255));
}

#[test]
fn the_eighth_blocks_grow_by_an_exact_eighth_of_the_cell() {
    // The rects are laid down in cell units and each pixel takes its own
    // overlap, so an eighth of ten columns or of twenty rows lands on the
    // real fractional edge. Only the rounding of a partly covered pixel
    // (half a level, over at most one row or column) separates the ink
    // from the exact fraction.
    let full = f64::from(ink(&cov('█')));
    let slack = 0.5 * (W.max(H) as f64) + 0.5;
    for (n, ch) in "▁▂▃▄▅▆▇█".chars().enumerate() {
        let want = full * (n as f64 + 1.0) / 8.0;
        let got = f64::from(ink(&cov(ch)));
        assert!((got - want).abs() <= slack, "{ch:?} is {}/8: {got}", n + 1);
    }
    for (n, ch) in "▉▊▋▌▍▎▏".chars().enumerate() {
        let want = full * (7.0 - n as f64) / 8.0;
        let got = f64::from(ink(&cov(ch)));
        assert!((got - want).abs() <= slack, "{ch:?} is {}/8: {got}", 7 - n);
    }
}

#[test]
fn the_halves_meet_along_the_cells_own_middle() {
    let (upper, lower) = (cov('▀'), cov('▄'));
    for i in 0..W * H {
        assert_eq!(upper[i].max(lower[i]), 255, "cell {i} is covered once");
        assert_eq!(upper[i].min(lower[i]), 0, "and only once");
    }
    let (left, right) = (cov('▌'), cov('▐'));
    for i in 0..W * H {
        assert_eq!(left[i].max(right[i]), 255, "cell {i}");
        assert_eq!(left[i].min(right[i]), 0, "cell {i}");
    }
}

#[test]
fn the_quadrants_tile_the_cell() {
    let (ul, ur, ll, lr) = (cov('▘'), cov('▝'), cov('▖'), cov('▗'));
    for i in 0..W * H {
        let sum = u32::from(ul[i]) + u32::from(ur[i]) + u32::from(ll[i]) + u32::from(lr[i]);
        assert_eq!(sum, 255, "cell {i} belongs to exactly one quadrant");
    }
    // The composites are unions of those four: ▚ is the two that touch
    // at the centre, ▛ is everything but the lower right.
    let (diag, three) = (cov('▚'), cov('▛'));
    for i in 0..W * H {
        assert_eq!(diag[i], ul[i].max(lr[i]), "▚ cell {i}");
        assert_eq!(three[i], ul[i].max(ur[i]).max(ll[i]), "▛ cell {i}");
    }
}

#[test]
fn a_shade_is_the_full_block_at_a_fraction_of_its_coverage() {
    for (ch, want) in [('░', 64u8), ('▒', 128), ('▓', 191)] {
        assert!(
            cov(ch).iter().all(|&v| v == want),
            "{ch:?} is a uniform {want}"
        );
    }
}

#[test]
fn the_double_family_and_ordinary_text_are_left_to_the_face() {
    // Falling through is what makes a partial synthesis safe: the double
    // lines still draw, they just draw the way the font draws them.
    for ch in ['═', '║', '╔', '╬', 'A', '▰', '▱', '★', ' '] {
        assert!(glyph(ch).is_none(), "{ch:?} falls through to the face");
    }
    assert!(glyph('╿').is_some(), "a mixed-weight line is synthesized");
}

#[test]
fn every_synthesized_code_point_draws_something() {
    // A table hole would silently render as a blank cell rather than
    // fall through, so nothing in the covered ranges may be empty.
    let ranges = [0x2500..=0x254b, 0x256d..=0x257f, 0x2580..=0x259f];
    for cp in ranges.into_iter().flatten() {
        let ch = char::from_u32(cp).expect("a box-drawing code point");
        assert!(ink(&cov(ch)) > 0, "U+{cp:04X} {ch:?} inks something");
    }
    for cp in 0x254c..=0x256c {
        let ch = char::from_u32(cp).expect("a box-drawing code point");
        let synthesized = (0x254c..=0x254f).contains(&cp);
        assert_eq!(
            glyph(ch).is_some(),
            synthesized,
            "U+{cp:04X} {ch:?}: only the double dashes are synthesized here"
        );
    }
}
