use super::*;
use crate::layout::Rect;
use crate::pixel::{CellSize, PixelSurface};

/// The RGBA buffer as rows of glyphs by luminance: ` ` below a tenth,
/// `.` `+` `#` upward — coarse enough to survive anti-aliasing lanes.
fn ascii(surface: &PixelSurface) -> String {
    let width = usize::from(surface.width());
    surface
        .pixels()
        .chunks(4)
        .map(|px| {
            let lum =
                (u32::from(px[0]) * 299 + u32::from(px[1]) * 587 + u32::from(px[2]) * 114) / 1000;
            match lum {
                0..=40 => ' ',
                41..=110 => '.',
                111..=180 => '+',
                _ => '#',
            }
        })
        .collect::<Vec<char>>()
        .chunks(width)
        .map(|row| row.iter().collect::<String>())
        .collect::<Vec<String>>()
        .join("\n")
}

fn render(sparkline: &Sparkline, cols: u16, rows: u16) -> PixelSurface {
    let mut surface = PixelSurface::new(Rect::new(0, 0, cols, rows), CellSize::new(4, 8));
    surface.paint(sparkline);
    surface
}

#[test]
fn snap_rising_series_on_a_12x2_cell_surface() {
    let sparkline = Sparkline::new([1.0, 3.0, 2.0, 5.0, 4.0, 8.0, 6.0, 9.0]);
    assert_eq!(sparkline.values().len(), 8);
    insta::assert_snapshot!(ascii(&render(&sparkline, 12, 2)));
}

#[test]
fn snap_line_only_on_a_light_background() {
    let sparkline = Sparkline::new([5.0, 1.0, 5.0, 1.0, 5.0])
        .area(false)
        .line(20, 20, 20)
        .background(240, 240, 240);
    insta::assert_snapshot!(ascii(&render(&sparkline, 8, 2)));
}

#[test]
fn a_single_value_is_a_flat_line_and_no_values_is_background_only() {
    let one = render(&Sparkline::new([4.0]), 4, 1);
    let art = ascii(&one);
    assert!(art.lines().any(|row| row.trim().len() >= 14), "{art}");
    let none = render(&Sparkline::new(Vec::<f32>::new()), 4, 1);
    assert!(
        none.pixels().chunks(4).all(|px| px == [26, 27, 38, 255]),
        "background only"
    );
}

#[test]
fn non_finite_values_are_skipped_and_a_flat_series_sits_in_the_middle() {
    let flat = render(&Sparkline::new([2.0, f32::NAN, 2.0, 2.0]).area(false), 4, 2);
    let art = ascii(&flat);
    let rows: Vec<&str> = art.lines().collect();
    assert_eq!(rows.len(), 16);
    let lit: Vec<usize> = rows
        .iter()
        .enumerate()
        .filter(|(_, row)| row.contains('#') || row.contains('+'))
        .map(|(i, _)| i)
        .collect();
    assert!(lit.iter().all(|&i| (6..=9).contains(&i)), "{art}");
}

/// the sweep reveals the line from the left, one frame per
/// `t`, and at `1` (or `None`) the frame is the whole sparkline.
#[test]
fn a_sweep_reveals_the_line_from_the_left_and_ends_whole() {
    const VALUES: [f32; 8] = [1.0, 3.0, 2.0, 5.0, 4.0, 8.0, 6.0, 9.0];
    let lit = |surface: &PixelSurface| {
        ascii(surface)
            .chars()
            .filter(|c| !c.is_whitespace())
            .count()
    };
    let whole = render(&Sparkline::new(VALUES), 12, 2);
    let half = render(&Sparkline::new(VALUES).sweep(Some(0.4)), 12, 2);
    assert!(lit(&half) < lit(&whole), "{} < {}", lit(&half), lit(&whole));
    assert!(lit(&half) > 0, "the revealed part is drawn");
    // Past the edge (plus the head dot's reach) nothing is drawn.
    let width = usize::from(half.width());
    for row in ascii(&half).lines() {
        let tail: String = row.chars().skip(width * 6 / 10).collect();
        assert!(tail.trim().is_empty(), "drawn past the edge: {row:?}");
    }
    let done = render(&Sparkline::new(VALUES).sweep(Some(1.0)), 12, 2);
    assert_eq!(done.pixels(), whole.pixels(), "1 is the whole line");
    let none = render(&Sparkline::new(VALUES).sweep(Some(0.0)), 12, 2);
    assert_eq!(lit(&none), 0, "0 is the background alone");
    // Frames grow monotonically as the sweep advances.
    let mut last = 0;
    for step in 0..=10 {
        let frame = render(
            &Sparkline::new(VALUES).sweep(Some(step as f32 / 10.0)),
            12,
            2,
        );
        let count = lit(&frame);
        assert!(count >= last, "step {step}: {count} < {last}");
        last = count;
    }
}
