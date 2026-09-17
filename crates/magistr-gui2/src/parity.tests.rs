use super::*;

fn solid(width: u32, height: u32, rgb: (u8, u8, u8)) -> Pixmap {
    let mut pixmap = Pixmap::new(width, height).expect("allocates");
    pixmap.fill(tiny_skia::Color::from_rgba8(rgb.0, rgb.1, rgb.2, 255));
    pixmap
}

fn paint(pixmap: &mut Pixmap, x: u32, y: u32, w: u32, h: u32, rgb: (u8, u8, u8)) {
    let width = pixmap.width();
    let colour = PremultipliedColorU8::from_rgba(rgb.0, rgb.1, rgb.2, 255).expect("opaque");
    for yy in y..y + h {
        for xx in x..x + w {
            pixmap.pixels_mut()[(yy * width + xx) as usize] = colour;
        }
    }
}

#[test]
fn identical_images_differ_nowhere_and_pass() {
    let a = solid(20, 10, (22, 22, 34));
    let verdict = Parity::compare(&a, &a, &Mask::none(), Medium::Night);
    assert_eq!(verdict.differing, 0);
    assert_eq!(verdict.unmasked, 200);
    assert!(verdict.passes());
    assert_eq!(verdict.status(), "PASS");
    assert_eq!(
        verdict.to_string(),
        "0.00% of 200 unmasked px differ (threshold 1.50%)"
    );
}

#[test]
fn a_shade_within_the_tolerance_is_the_same_pixel_and_a_colour_is_not() {
    let a = PremultipliedColorU8::from_rgba(100, 100, 100, 255).expect("opaque");
    let shade = PremultipliedColorU8::from_rgba(124, 76, 100, 255).expect("opaque");
    let colour = PremultipliedColorU8::from_rgba(125, 100, 100, 255).expect("opaque");
    assert!(Parity::same(a, shade));
    assert!(!Parity::same(a, colour));
}

#[test]
fn the_share_is_over_the_unmasked_pixels_and_the_threshold_decides() {
    let capture = solid(100, 10, (0, 0, 0));
    let mut still = solid(100, 10, (0, 0, 0));
    // 20 differing pixels of 1000: 2%, over the default 1.5%.
    paint(&mut still, 0, 0, 20, 1, (255, 255, 255));
    let whole = Parity::compare(&capture, &still, &Mask::none(), Medium::Day);
    assert_eq!((whole.differing, whole.unmasked), (20, 1000));
    assert!(!whole.passes());
    assert_eq!(whole.status(), "FAIL");
    // Masking 10 of them leaves 10 of 990: 1.01%, under.
    let mask = Mask::parse(r#"{"excluded": [{"x": 0, "y": 0, "w": 10, "h": 1, "why": "live"}]}"#)
        .expect("parses");
    let masked = Parity::compare(&capture, &still, &mask, Medium::Day);
    assert_eq!((masked.differing, masked.unmasked), (10, 990));
    assert!(masked.passes());
    // A tighter threshold fails the same picture: the threshold is the
    // view's, not the tool's.
    let tight = Mask::parse(r#"{"threshold": 0.005}"#).expect("parses");
    assert!(!Parity::compare(&capture, &still, &tight, Medium::Day).passes());
    // A medium-scoped rect masks nothing in the other medium.
    let night_only = Mask::parse(
        r#"{"excluded": [{"x": 0, "y": 0, "w": 20, "h": 1, "why": "halo", "media": "night"}]}"#,
    )
    .expect("parses");
    assert!(Parity::compare(&capture, &still, &night_only, Medium::Night).passes());
    assert!(!Parity::compare(&capture, &still, &night_only, Medium::Day).passes());
}

#[test]
fn a_still_of_another_size_fails_whole_whatever_the_threshold() {
    let capture = solid(10, 10, (0, 0, 0));
    let still = solid(10, 9, (0, 0, 0));
    let lenient = Mask::parse(r#"{"threshold": 1.0}"#).expect("parses");
    let verdict = Parity::compare(&capture, &still, &lenient, Medium::Night);
    assert!(!verdict.passes());
    assert_eq!(verdict.differing, 100);
    assert_eq!(verdict.size_mismatch, Some(((10, 10), (10, 9))));
    assert_eq!(
        verdict.to_string(),
        "the still is 10x9 and the capture 10x10; nothing to compare"
    );
}

#[test]
fn each_region_is_measured_over_its_own_unmasked_pixels_and_its_budget_decides() {
    let capture = solid(100, 10, (0, 0, 0));
    let mut still = solid(100, 10, (0, 0, 0));
    // 20 differing pixels, all in the left half: 4% of the left
    // region's 500, 0% of the right's, 2% of the frame's 1000.
    paint(&mut still, 0, 0, 20, 1, (255, 255, 255));
    let mask = Mask::parse(
        r#"{"threshold": 0.05, "regions": [
            {"name": "left", "x": 0, "y": 0, "w": 50, "h": 10, "budget": 0.03},
            {"name": "right", "x": 50, "y": 0, "w": 50, "h": 10, "budget": 0.03}
        ]}"#,
    )
    .expect("parses");
    let verdict = Parity::compare(&capture, &still, &mask, Medium::Day);
    // The frame is under its threshold; the left region is not — the
    // region is the primary signal and the frame only the backstop.
    assert!(verdict.share() <= 0.05);
    assert_eq!(verdict.regions.len(), 2);
    let left = &verdict.regions[0];
    assert_eq!((left.differing, left.unmasked), (20, 500));
    assert!(!left.passes());
    let right = &verdict.regions[1];
    assert_eq!((right.differing, right.unmasked), (0, 500));
    assert!(right.passes());
    assert!(!verdict.passes(), "a region over budget fails the view");
    assert_eq!(verdict.status(), "FAIL");
    assert_eq!(left.to_string(), "left 4.00% of 500 px (budget 3.00%)");
    assert!(
        verdict.to_string().contains("left 4.00%"),
        "the verdict names the region that missed: {verdict}"
    );
    // A masked rect inside a region takes its pixels out of the
    // region's count too, so masking never shifts a region's share
    // onto what is left.
    let masked = Mask::parse(
        r#"{"threshold": 0.05,
            "excluded": [{"x": 0, "y": 0, "w": 20, "h": 1, "why": "live"}],
            "regions": [{"name": "left", "x": 0, "y": 0, "w": 50, "h": 10, "budget": 0.03}]}"#,
    )
    .expect("parses");
    let verdict = Parity::compare(&capture, &still, &masked, Medium::Day);
    let left = &verdict.regions[0];
    assert_eq!((left.differing, left.unmasked), (0, 480));
    assert!(verdict.passes());
}

#[test]
fn a_size_mismatch_reports_no_regions() {
    let capture = solid(10, 10, (0, 0, 0));
    let still = solid(10, 9, (0, 0, 0));
    let mask = Mask::parse(
        r#"{"regions": [{"name": "all", "x": 0, "y": 0, "w": 10, "h": 10, "budget": 1.0}]}"#,
    )
    .expect("parses");
    let verdict = Parity::compare(&capture, &still, &mask, Medium::Night);
    assert!(verdict.regions.is_empty());
    assert!(!verdict.passes());
}

#[test]
fn the_diff_image_is_grey_where_same_red_where_not_and_blue_under_the_mask() {
    let capture = solid(30, 1, (200, 200, 200));
    let mut still = capture.clone();
    paint(&mut still, 10, 0, 10, 1, (0, 0, 0));
    let mask = Mask::parse(r#"{"excluded": [{"x": 20, "y": 0, "w": 10, "h": 1, "why": "live"}]}"#)
        .expect("parses");
    let diff = Parity::diff(&capture, &still, &mask, Medium::Night);
    assert_eq!((diff.width(), diff.height()), (30, 1));
    let px = |x: usize| {
        let p = diff.pixels()[x];
        (p.red(), p.green(), p.blue())
    };
    let (r, g, b) = px(0);
    assert!(r == g && g == b, "same pixels are grey: {:?}", px(0));
    assert_eq!(px(15), (255, 40, 40), "differing pixels are red");
    let (r, g, b) = px(25);
    assert!(
        b > r && b > g && r == g,
        "masked pixels are tinted blue: {:?}",
        px(25)
    );
}

#[test]
fn the_diff_of_a_size_mismatch_is_all_red_at_the_captures_size() {
    let capture = solid(4, 2, (0, 0, 0));
    let still = solid(3, 2, (0, 0, 0));
    let diff = Parity::diff(&capture, &still, &Mask::none(), Medium::Day);
    assert_eq!((diff.width(), diff.height()), (4, 2));
    assert!(
        diff.pixels()
            .iter()
            .all(|p| p.red() == 255 && p.green() == 40)
    );
}
