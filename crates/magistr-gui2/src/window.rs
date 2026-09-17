//! The window by hand: the native runtime on a fixture.

use magistr_cockpit::cockpit::Size;

use crate::error::Gui2Error;
use crate::fixture::Fixture;
use crate::metrics::CellMetrics;
use crate::native::Native;

/// Open an interactive fixture window with the given title.
///
/// Use [`Fixture::busy`] when `view` is `None`; otherwise select a reference view from
/// [`Fixture::VIEWS`]. These windows have no session producers, so entered prompts are
/// not sent to an agent.
///
/// Views open at [`Fixture::CAPTURE`]'s logical size. This allows comparison with
/// headless parity captures on displays with different scale factors. The model is
/// regridded when the window size changes.
///
/// # Errors
///
/// Return the first event-loop error, or [`Gui2Error::Metrics`] if the embedded font
/// cannot provide the required grid.
pub fn open(title: &str, view: Option<&str>) -> Result<(), Gui2Error> {
    let Some(view) = view else {
        return Native::new(title)
            .run(Fixture::with_pixels(Fixture::busy(Size::new(80, 24))))
            .map(|_| ());
    };
    // The dev route: the kitchen sink is the element substrate's own
    // page, not a cockpit fixture, so it runs its own loop.
    if view == "sink" {
        return crate::sink_window::open_sink(title);
    }
    // The grid the capture implies at the *default* cell, which is the
    // grid the still is rendered on. A 2x window derives the same one
    // from twice the physical pixels and twice the cell.
    let mut text = crate::text::TextEngine::embedded();
    let cell = text
        .metrics(CellMetrics::DEFAULT_PX)
        .ok_or(Gui2Error::Metrics(CellMetrics::DEFAULT_PX))?;
    let grid = Fixture::capture_grid((cell.width(), cell.height()));
    let model = Fixture::mock(view, grid)
        .ok_or_else(|| Gui2Error::Window(format!("no fixture builds view {view}")))?;
    let (width, height) = Fixture::CAPTURE;
    Native::new(title)
        .with_logical_size(f64::from(width), f64::from(height))
        .run(Fixture::with_pixels(model))
        .map(|_| ())
}

#[cfg(test)]
#[path = "window.tests.rs"]
mod tests;
