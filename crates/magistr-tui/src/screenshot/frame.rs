//! A cockpit model as the pixels its frame would show.

use tiny_skia::Pixmap;

use super::kitty;
use super::placement::Placement;
use super::profile::Profile;
use super::raster::rasterize;
use super::typeface::Typeface;
use crate::graphics::{Decision, Protocol};
use crate::pixel::{Compression, IdSpace};
use crate::render::Screen;
use crate::runtime::Surfaces;
use magistr_cockpit::cockpit::{FrameClock, Model, Msg, update, view};
use magistr_cockpit::layout::{LayoutError, LayoutTree};
use magistr_cockpit::render::Buffer;
use magistr_cockpit::render::paint;

/// Render `model` the way the runtime would for one frame on a terminal
/// matching `profile` — [`view`] with `clock`, laid out at the model's
/// size, painted into a cell buffer — and rasterize that buffer through
/// the model's theme.
///
/// Under [`Profile::Cells`] that is the whole story, and the result is
/// byte-for-byte what this function returned before terminal profiles
/// existed. Under a graphics profile the frame additionally runs the
/// runtime's own [`Surfaces`] over a real [`Screen`], presents it into a
/// byte buffer and reads the placements back out of those bytes
/// ([`kitty::decode`]) — so a still shows the pixels the cockpit would
/// really put on screen rather than a second drawing of them.
pub fn frame(model: &Model, clock: &FrameClock, profile: Profile) -> Result<Pixmap, LayoutError> {
    let (buffer, images) = match profile.protocol() {
        Some(protocol) => compose(model, clock, protocol)?,
        None => (cells(model, clock)?, Vec::new()),
    };
    Ok(rasterize(
        &buffer,
        &model.theme().palette(),
        &mut Typeface::new(),
        &images,
    ))
}

/// Resolve the same physical layout used by a graphics terminal, including hit rectangles.
pub fn layout(
    model: &Model,
    clock: &FrameClock,
    profile: Profile,
) -> Result<magistr_cockpit::layout::Resolved, LayoutError> {
    let mut model = model.clone();
    if let Some(protocol) = profile.protocol() {
        update(&mut model, Msg::Mode(Decision::Graphics(protocol).mode()));
    }
    Surfaces::default().layout(&view(&model, clock), &model, &mut LayoutTree::new())
}

/// The frame's cell buffer, with no graphics anywhere near it.
fn cells(model: &Model, clock: &FrameClock) -> Result<Buffer, LayoutError> {
    let size = model.size();
    let root = view(model, clock);
    let resolved = LayoutTree::new().compute(&root, size.cols, size.rows)?;
    let mut buffer = Buffer::new(size.cols, size.rows);
    paint(&root, &resolved, &mut buffer);
    Ok(buffer)
}

/// The frame's cell buffer and the images a `protocol` terminal would
/// be showing over it, decoded from the frame's own bytes.
///
/// Two layout passes, for the reason the runtime takes two frames: the
/// tech-trees report their placed rect back to the model and the tab's
/// layout depends on that report, so the first pass is what *makes* the
/// settled layout and the second is it. Each pass gets its own
/// [`Screen`] and its own [`Surfaces`], because a compositor shown the
/// same widget twice without a `present` between would queue two
/// placements for it.
pub(crate) fn compose(
    model: &Model,
    clock: &FrameClock,
    protocol: Protocol,
) -> Result<(Buffer, Vec<Placement>), LayoutError> {
    let (buffer, wire) = wire(model, clock, protocol)?;
    Ok((buffer, kitty::decode(&wire)))
}

/// The frame's cell buffer and the bytes a `protocol` terminal would
/// be sent for it — what [`compose`] decodes, kept apart so a test can
/// count the placements in the very bytes the decoder read.
pub(crate) fn wire(
    model: &Model,
    clock: &FrameClock,
    protocol: Protocol,
) -> Result<(Buffer, Vec<u8>), LayoutError> {
    let size = model.size();
    // The startup probe's decision is what `view` branches on, so a
    // pixel still has to fold it — into a copy, because a screenshot
    // must not move the model its caller handed over. The copy is also
    // what carries the graph-rect reports from the first pass into the
    // second.
    let mut model = model.clone();
    update(&mut model, Msg::Mode(Decision::Graphics(protocol).mode()));

    // The reporting pass. Its own frame is thrown away: the two
    // tech-trees hand their placed rect back to the model here, and the
    // tab's layout depends on that report, so the frame that computed
    // it is by definition not the settled one.
    let mut surfaces = Surfaces::default();
    let root = view(&model, clock);
    let resolved = surfaces.layout(&root, &model, &mut LayoutTree::new())?;
    surfaces.show(&root, &mut model, &resolved, &mut screen(size, protocol));

    // The settled pass, laid out from the reported rects. A third would
    // report the same and change nothing.
    let root = view(&model, clock);
    let resolved = surfaces.layout(&root, &model, &mut LayoutTree::new())?;
    let mut buffer = Buffer::new(size.cols, size.rows);
    paint(&root, &resolved, &mut buffer);

    let mut screen = screen(size, protocol);
    // The cells go on the screen as well as into the buffer we
    // rasterize: they are what leaves the encoder's cursor where the
    // placements' own moves are measured from.
    *screen.buffer() = buffer.clone();
    surfaces.show(&root, &mut model, &resolved, &mut screen);
    buffer = screen.buffer().clone();
    let mut wire = Vec::new();
    screen.present(&mut wire);
    Ok((buffer, wire))
}

/// A screen of `size` compositing through `protocol`.
///
/// `IdSpace::FIRST` rather than the runtime's per-process space: a still
/// must come out the same bytes on every machine, and the tag is the
/// one part of an id that is deliberately not reproducible. Nothing
/// else shares this screen's image store, so there is no collision for
/// a tag to prevent. `Compression::None` for the same reason a test
/// pins the wire — the pixels are the point, and an inflate is a step
/// between the frame and the golden that can only lose.
fn screen(size: magistr_cockpit::cockpit::Size, protocol: Protocol) -> Screen {
    let mut screen = Screen::new(size.cols, size.rows);
    screen.enable_graphics(protocol, Compression::None, IdSpace::FIRST);
    screen
}

#[cfg(test)]
#[path = "frame.tests.rs"]
mod tests;
