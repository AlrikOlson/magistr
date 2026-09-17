use super::*;

/// A window is never opened for a view no fixture builds — the name is
/// refused before winit is touched, so a typo is one line of output
/// rather than an empty cockpit that looks like a rendering bug.
#[test]
fn an_unknown_view_is_refused_before_a_window_opens() {
    let error = open("magistr", Some("zz")).expect_err("no fixture builds view zz");
    assert!(format!("{error}").contains("zz"));
}

/// The grid a named view opens on is the grid its still is rendered on,
/// so the live window and the parity gate's picture are the same
/// arrangement and a difference between them is a difference in
/// *density handling* rather than in layout.
#[test]
fn a_named_view_opens_on_the_grid_its_still_is_rendered_on() {
    let mut text = crate::text::TextEngine::embedded();
    let cell = text
        .metrics(CellMetrics::DEFAULT_PX)
        .expect("the embedded face yields a grid");
    let grid = Fixture::capture_grid((cell.width(), cell.height()));
    let (width, height) = Fixture::CAPTURE;
    // Doubling both physical pixels and cell dimensions preserves the logical grid.
    assert_eq!(grid, cell.grid(width, height));
    let doubled = CellMetrics::new(text.shaper(), cell.font(), CellMetrics::DEFAULT_PX * 2.0)
        .expect("a 2x grid");
    assert_eq!(grid, doubled.grid(width * 2, height * 2));
}
