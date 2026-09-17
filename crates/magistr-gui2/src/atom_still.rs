use qs_gpu::cpu_raster::CpuRasterizer;
use qs_gpu::frame::DrawList;
use tiny_skia::Pixmap;

use crate::element::Element;
use crate::element_emit::emit;
use crate::element_layout::ElementLayout;
use crate::element_state::ElementState;
use crate::error::Gui2Error;
use crate::lexicon::Lexicon;
use crate::medium::Medium;
use crate::text::TextEngine;

/// An element tree rasterized on the CPU tier at `width` x `height`,
/// in `medium` — [`Frame::still`](crate::Frame::still) for the a1
/// substrate, and what the atom-scale parity gate takes its pictures
/// with.
///
/// The canvas is the medium's own `base`, because that is the ground
/// every specimen capture was cut over: a capture is the atom's
/// border box photographed on the volume's page, so the pixels outside
/// a rounded corner are the page. The build loops like `Frame::still`
/// does — the atlas admits two pictures a frame, so a tree carrying
/// marks converges over a few passes — and only the settled pass's
/// list is rasterized.
///
/// # Errors
///
/// [`Gui2Error::Elements`] from the layout pass, or
/// [`Gui2Error::Raster`] when the CPU tier refuses the viewport.
pub fn atom_still(
    root: &Element,
    state: &ElementState,
    medium: Medium,
    width: u32,
    height: u32,
) -> Result<Pixmap, Gui2Error> {
    if width == 0 || height == 0 {
        return Err(Gui2Error::Raster(width, height));
    }
    let ground = Lexicon::of(medium).base;
    let mut text = TextEngine::embedded();
    let mut cpu =
        CpuRasterizer::with_colour_page(width, height, text.atlas_size(), text.colour_size())
            .ok_or(Gui2Error::Raster(width, height))?;
    #[allow(clippy::cast_precision_loss)]
    let layout = ElementLayout::compute(root, width as f32, height as f32, &mut text)?;
    // Enough passes for every mark the tree could carry, plus the
    // settled one: the atlas admits two pictures a frame.
    let mut list = DrawList::default();
    for generation in 1..=16u64 {
        text.begin_frame();
        list = DrawList::default();
        list.reset([width, height], ground, generation);
        emit(root, &layout, state, &mut text, &mut list, 0..root.count());
        list.end_batch(None, true);
        list.finish();
        let glyphs = text.take_uploads();
        let images = text.take_image_uploads();
        cpu.upload_glyphs(&glyphs);
        cpu.upload_images(&images);
        if glyphs.is_empty() && images.is_empty() {
            break;
        }
    }
    cpu.render(&list);
    let bytes = cpu.to_srgb_rgba8();
    let mut pixmap = Pixmap::new(width, height).ok_or(Gui2Error::Raster(width, height))?;
    pixmap.data_mut().copy_from_slice(&bytes);
    Ok(pixmap)
}

#[cfg(test)]
#[path = "atom_still.tests.rs"]
mod tests;
