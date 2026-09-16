//! Render alternative cockpit compositions through the production pixel pipeline.

use tiny_skia::Pixmap;

use super::{kitty, placement::Placement};
use crate::graphics::{Decision, Passthrough, Protocol};
use crate::pixel::{Compression, IdSpace};
use crate::render::Screen;
use crate::runtime::Surfaces;
use magistr_cockpit::cockpit::{FrameClock, Model, Msg, update};
use magistr_cockpit::layout::{LayoutError, LayoutTree, Node, Resolved};
use magistr_cockpit::render::{Buffer, paint};

/// A design-study capture using the runtime's real layout, surfaces and wire encoder.
/// Custom views remain outside the production shell until explicitly adopted.
pub struct Preview;

#[cfg(test)]
#[path = "preview.tests.rs"]
mod tests;

impl Preview {
    /// Capture a custom composition using the direct Kitty graphics profile.
    pub fn frame(
        model: &Model,
        clock: &FrameClock,
        view: impl Fn(&Model, &FrameClock) -> Node,
    ) -> Result<Pixmap, LayoutError> {
        let (_, bytes) = Self::wire(model, clock, Protocol::Kitty(Passthrough::Direct), view)?;
        Ok(Self::raster(model, &kitty::decode(&bytes)))
    }

    /// Reconstruct exactly the opaque strips placed by the production renderer.
    /// The plain-terminal rasterizer has fixed font metrics; graphics captures
    /// instead use the terminal metrics that generated their actual pixels.
    pub(super) fn raster(model: &Model, images: &[Placement]) -> Pixmap {
        let cell = model.cell_size();
        let size = model.size();
        let (width, height) = cell.pixels(size.cols, size.rows);
        let mut canvas =
            Pixmap::new(u32::from(width.max(1)), u32::from(height.max(1))).expect("bounded frame");
        for image in images {
            let Some(pixels) =
                tiny_skia::PixmapRef::from_bytes(&image.rgba, image.width, image.height)
            else {
                continue;
            };
            canvas.draw_pixmap(
                i32::from(image.col) * i32::from(cell.width),
                i32::from(image.row) * i32::from(cell.height),
                pixels,
                &tiny_skia::PixmapPaint::default(),
                tiny_skia::Transform::identity(),
                None,
            );
        }
        canvas
    }

    /// Resolve physical rectangles for capture inspection and hit testing.
    pub fn layout(
        model: &Model,
        clock: &FrameClock,
        view: impl Fn(&Model, &FrameClock) -> Node,
    ) -> Result<Resolved, LayoutError> {
        let mut model = model.clone();
        update(
            &mut model,
            Msg::Mode(Decision::Graphics(Protocol::Kitty(Passthrough::Direct)).mode()),
        );
        let mut surfaces = Surfaces::default();
        let root = view(&model, clock);
        let resolved = surfaces.layout(&root, &model, &mut LayoutTree::new())?;
        let size = model.size();
        surfaces.show(
            &root,
            &mut model,
            &resolved,
            &mut screen(size, Protocol::Kitty(Passthrough::Direct)),
        );
        surfaces.layout(&view(&model, clock), &model, &mut LayoutTree::new())
    }

    pub(super) fn wire(
        model: &Model,
        clock: &FrameClock,
        protocol: Protocol,
        view: impl Fn(&Model, &FrameClock) -> Node,
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
}

fn screen(size: magistr_cockpit::cockpit::Size, protocol: Protocol) -> Screen {
    let mut screen = Screen::new(size.cols, size.rows);
    screen.enable_graphics(protocol, Compression::None, IdSpace::FIRST);
    screen
}
