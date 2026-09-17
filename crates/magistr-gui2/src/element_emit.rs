use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use qs_gpu::atlas::ImageKey;
use qs_gpu::color::Srgba;
use qs_gpu::frame::{DrawList, Instance};

use crate::element::{Element, Kind};
use crate::element_layout::{ElementLayout, line_box, shape_voice};
use crate::element_state::ElementState;
use crate::fonts::EmbeddedFonts;
use crate::image_quad::ImageQuad;
use crate::mark_raster::mark_raster;
use crate::pane_tile::PaneTile;
use crate::text::TextEngine;

/// How far outside its element a focus ring stands, and how thick it
/// is: visibly separate from the surface's own edge, thin enough to be
/// an annotation rather than a border.
const RING_OFFSET_PX: f32 = 2.0;
const RING_WIDTH_PX: f32 = 1.5;

/// Draw the placed tree straight into a qs-gpu [`DrawList`], depth
/// first — parents under children, exactly the order layout numbered.
///
/// `range` selects a DFS index window: [`Element::each`]'s numbering
/// puts a subtree at contiguous indices, so a frame can emit before a
/// subtree, the subtree under its own batch (the stage's scissor, the
/// a7 sky-under-scrim ordering), and after it — the full range is the
/// whole tree.
///
/// Every primitive the design language uses is emitted here from typed
/// fields: the two-layer shadow and rim from the element's
/// [`Elevation`](crate::Elevation), the fill painted **exactly as
/// declared** — the volume's palette already bakes the surface shift
/// into its own colors (`mantle` *is* the lifted ground), so a derived
/// lift belongs to whoever chose the color, never to this walk — the
/// vertical gradient, the solid or
/// dashed stroke, the glow, the focus ring, and the glyph runs — shaped
/// by the same seam the layout measured through, on the rows the layout
/// stored, so ink lands exactly where height was paid for.
pub fn emit(
    root: &Element,
    layout: &ElementLayout,
    state: &ElementState,
    text: &mut TextEngine,
    list: &mut DrawList,
    range: std::ops::Range<usize>,
) {
    root.each(&mut |index, element| {
        if !range.contains(&index) {
            return;
        }
        let [x, y, w, h] = layout.rect(index);
        if w <= 0.0 || h <= 0.0 {
            return;
        }
        match &element.kind {
            Kind::Surface => surface(element, [x, y, w, h], state, list),
            Kind::Mark { mark, px, ink } => {
                // The same admit-once route Marks::draw takes: keyed by
                // (mark, px, ink), rasterized at exactly px and drawn at
                // exactly px. The atlas admits two pictures a frame, so
                // a declined mark arrives on a later pass — the still
                // harness loops until nothing new uploads.
                let mut hasher = DefaultHasher::new();
                (mark, px, ink).hash(&mut hasher);
                let key = ImageKey(hasher.finish());
                if let Some(entry) = text.image(key, |_| mark_raster(&mark.shapes(), *px, *ink)) {
                    #[allow(clippy::cast_precision_loss)]
                    let side = *px as f32;
                    list.instances.push(Instance::image(
                        x,
                        y,
                        side,
                        side,
                        entry.uv,
                        Srgba::new(1.0, 1.0, 1.0, 1.0),
                    ));
                }
            }
            Kind::Picture { image } => {
                // The feed's own route for a picture: admitted under its
                // digest, fitted to the colour page when it is larger
                // than the page, drawn into the box the element measures
                // — which is the picture's aspect at thumbnail height.
                let page = text.colour_size();
                if let Some(entry) = text.image(ImageKey(image.digest()), |_| {
                    Some(ImageQuad::admission(image, page))
                }) {
                    list.instances.push(Instance::image(
                        x,
                        y,
                        w,
                        h,
                        entry.uv,
                        Srgba::new(1.0, 1.0, 1.0, 1.0),
                    ));
                }
            }
            Kind::Grid { rows, px } => grid(rows, *px, [x, y, w, h], text, list),
            Kind::Text { voice, content } => {
                let ink = element.fill.unwrap_or(Srgba::new(0.0, 0.0, 0.0, 1.0));
                let (line, baseline) = line_box(text, voice);
                for (row, (start, end)) in layout.text_rows(index).iter().enumerate() {
                    #[allow(clippy::cast_precision_loss)]
                    let top = y + row as f32 * line;
                    run(
                        text,
                        voice,
                        &content[*start..*end],
                        x,
                        top + baseline,
                        x + w,
                        ink,
                        list,
                    );
                }
            }
        }
    });
}

/// The box a painted surface actually occupies: its layout rect on the
/// device grid, the way the browser paints one. Layout runs at
/// fractional pixels — the header's 51.75 puts every box under it on
/// a .75 edge — and the capture shows each ring, hairline and track
/// crisp on whole rows because Chrome snaps a painted box's edges to
/// the nearest pixel: the origin rounds, the far edge rounds, the size
/// is their difference, so two boxes that abut in layout abut on the
/// grid too. A box thinner than half a pixel keeps one pixel rather
/// than vanishing, as a border does. Text is not snapped here — its
/// baseline is already the engine's own whole pixel
/// ([`line_box`]) — and a mark's own box is a rasterized picture at
/// exactly its size.
fn snapped([x, y, w, h]: [f32; 4]) -> [f32; 4] {
    // The browser lays out in 1/64-px units, so an edge that is 305.5
    // exactly rounds up; ours arrives as the float sum of the boxes
    // above it (305.4999…) and would round down. Quantize the way the
    // layout unit does before rounding, so a tie is a tie there too.
    let grid = |v: f32| ((v * 64.0).round() / 64.0).round();
    let (x0, y0) = (grid(x), grid(y));
    let x1 = grid(x + w);
    let y1 = grid(y + h);
    let keep = |near: f32, size: f32| if size > 0.0 && near <= 0.0 { 1.0 } else { near };
    [x0, y0, keep(x1 - x0, w), keep(y1 - y0, h)]
}

/// One surface's skin, in the order the light falls: shadows under it,
/// its fill (flat or ramped), the rim over the fill, then its stroke,
/// glow and — while it holds focus — the ring.
fn surface(element: &Element, rect: [f32; 4], state: &ElementState, list: &mut DrawList) {
    let rect = if element.exact { rect } else { snapped(rect) };
    let [x, y, w, h] = rect;
    // Never more than half the shorter side: a radius bigger than the
    // box is a lozenge, and a one-row card is not one.
    let radius = element.radius.min(w.min(h) / 2.0);
    let elevation = element.elevation;
    list.instances.extend(elevation.shadows(rect, radius, 1.0));
    // An interaction override outranks the rest skin entirely — a
    // hovered primary trades its gradient for the hover fill, so the
    // pointer's answer is never painted under the rest face.
    if let Some(fill) = override_fill(element, state) {
        list.instances
            .push(Instance::rect(x, y, w, h, radius, fill));
    } else if let Some([top, bottom]) = element.gradient {
        list.instances.push(Instance::gradient(
            x,
            y,
            w,
            h,
            radius,
            std::f32::consts::FRAC_PI_2,
            top,
            bottom,
        ));
    } else if let Some(fill) = element.fill {
        list.instances
            .push(Instance::rect(x, y, w, h, radius, fill));
    }
    list.instances.extend(elevation.rim(rect, radius, 1.0));
    if let Some((color, width)) = element.stroke {
        if element.dashed {
            let (dash, gap) = element.dash;
            list.instances.push(Instance::dashed_stroke(
                x, y, w, h, radius, width, dash, gap, color,
            ));
        } else {
            list.instances
                .push(Instance::stroke(x, y, w, h, radius, width, color));
        }
    }
    if let Some((color, falloff)) = element.glow {
        list.instances
            .push(Instance::glow(x, y, w, h, radius, falloff.max(1.0), color));
    }
    if let (Some(ring), Some(id)) = (element.focus_ring, element.id)
        && state.focus() == Some(id)
    {
        list.instances.push(Instance::stroke(
            x - RING_OFFSET_PX,
            y - RING_OFFSET_PX,
            w + RING_OFFSET_PX * 2.0,
            h + RING_OFFSET_PX * 2.0,
            radius + RING_OFFSET_PX,
            RING_WIDTH_PX,
            ring,
        ));
    }
}

/// The interaction override, when one applies: the pressed skin while
/// a press is held on this element (falling back to the hover skin),
/// the hover skin while the pointer is over it. `None` means "at rest
/// — paint the rest face", gradient and all.
fn override_fill(element: &Element, state: &ElementState) -> Option<Srgba> {
    let id = element.id?;
    if state.pressed() == Some(id) {
        return element.press_fill.or(element.hover_fill);
    }
    if state.hover() == Some(id) {
        return element.hover_fill;
    }
    None
}

/// A terminal's rows on the mono grid the face makes at `px` — the way a terminal
/// emulator paints: first the grounds, one quad per run of tiles sharing one, then the
/// glyphs, each tile shaped on its own so a column is a column whatever the shaper
/// would have kerned, on the bold or italic cut its tile asks for, then a hairline per
/// run of underlined tiles. The grid's origin is the box's snapped corner, so the tiles
/// land on whole pixels like every other painted box; a column past the box's placed
/// width is clipped, which is what a pane narrower than its program's screen does.
///
/// A quiet tile — a space on the well, no rule — paints nothing at all, which is most
/// of an 80-column screen.
fn grid(
    rows: &[Vec<PaneTile>],
    px: f32,
    rect: [f32; 4],
    text: &mut TextEngine,
    list: &mut DrawList,
) {
    let Some(metrics) = text.metrics(px) else {
        return;
    };
    let (advance, line, baseline) = (metrics.width(), metrics.height(), metrics.baseline());
    let [x, y, w, _] = snapped(rect);
    let limit = x + w;
    let size = qs_text::PxSize::new(px);
    for (r, row) in rows.iter().enumerate() {
        #[allow(clippy::cast_precision_loss)]
        let top = y + r as f32 * line;
        // Grounds first, as runs: the same ground on neighbouring tiles
        // is one quad, so a bar written tile by tile has no seams.
        let mut run: Option<(usize, usize, Srgba)> = None;
        for (col, tile) in row.iter().enumerate() {
            match (&mut run, tile.ground) {
                (Some((_, end, ground)), Some(g)) if *ground == g => *end = col + 1,
                _ => {
                    if let Some((start, end, ground)) = run.take() {
                        ground_quad(start, end, ground, advance, line, x, top, limit, list);
                    }
                    run = tile.ground.map(|g| (col, col + 1, g));
                }
            }
        }
        if let Some((start, end, ground)) = run {
            ground_quad(start, end, ground, advance, line, x, top, limit, list);
        }
        // Then the glyphs, one tile at a time on the cut its tile asks
        // for. The mono family has no bold-italic cut, so a tile asking
        // for both leans: the slope is the rarer cue and the weight is
        // carried by the ink the program chose.
        for (col, tile) in row.iter().enumerate() {
            if tile.is_quiet() || tile.is_blank() {
                continue;
            }
            #[allow(clippy::cast_precision_loss)]
            let pen_x = x + col as f32 * advance;
            if pen_x >= limit {
                break;
            }
            let font = if tile.italic {
                EmbeddedFonts::MONO_ITALIC
            } else if tile.bold {
                EmbeddedFonts::BOLD
            } else {
                EmbeddedFonts::REGULAR
            };
            let shaped = text.run(&tile.text, font, size, qs_text::Features::default());
            for glyph in &shaped.glyphs {
                let Some(placed) = text.glyph(glyph.font, glyph.glyph_id, size, pen_x + glyph.x)
                else {
                    continue;
                };
                let entry = placed.entry;
                #[allow(clippy::cast_precision_loss)]
                list.instances.push(Instance::glyph(
                    placed.pen + entry.left as f32,
                    top + baseline - glyph.y - entry.top as f32,
                    entry.width as f32,
                    entry.height as f32,
                    entry.uv,
                    tile.ink,
                ));
            }
        }
        // Last the rules: one hairline per run of underlined tiles, a
        // pixel under the baseline in the run's ink.
        let rule_y = (top + baseline + 1.0).min(top + line - 1.0);
        let mut rule: Option<(usize, usize, Srgba)> = None;
        for (col, tile) in row.iter().enumerate() {
            let wants = tile.underline && !tile.is_blank();
            match (&mut rule, wants) {
                (Some((_, end, ink)), true) if *ink == tile.ink => *end = col + 1,
                _ => {
                    if let Some((start, end, ink)) = rule.take() {
                        rule_quad(start, end, ink, advance, x, rule_y, limit, list);
                    }
                    rule = wants.then_some((col, col + 1, tile.ink));
                }
            }
        }
        if let Some((start, end, ink)) = rule {
            rule_quad(start, end, ink, advance, x, rule_y, limit, list);
        }
    }
}

/// One run of tiles' ground, columns `start..end`, clipped at `limit`.
#[allow(clippy::too_many_arguments)]
fn ground_quad(
    start: usize,
    end: usize,
    ground: Srgba,
    advance: f32,
    line: f32,
    x: f32,
    top: f32,
    limit: f32,
    list: &mut DrawList,
) {
    #[allow(clippy::cast_precision_loss)]
    let (x0, x1) = (x + start as f32 * advance, x + end as f32 * advance);
    let x1 = x1.min(limit);
    if x1 > x0 {
        list.instances
            .push(Instance::rect(x0, top, x1 - x0, line, 0.0, ground));
    }
}

/// One run of tiles' underline, columns `start..end`, clipped at
/// `limit`.
#[allow(clippy::too_many_arguments)]
fn rule_quad(
    start: usize,
    end: usize,
    ink: Srgba,
    advance: f32,
    x: f32,
    y: f32,
    limit: f32,
    list: &mut DrawList,
) {
    #[allow(clippy::cast_precision_loss)]
    let (x0, x1) = (x + start as f32 * advance, x + end as f32 * advance);
    let x1 = x1.min(limit);
    if x1 > x0 {
        list.instances
            .push(Instance::rect(x0, y, x1 - x0, 1.0, 0.0, ink));
    }
}

/// One shaped row of ink on its baseline, clipped at `limit` the way
/// every painted run is: a glyph that would start past the box is
/// dropped, its advance kept.
#[allow(clippy::too_many_arguments)]
fn run(
    text: &mut TextEngine,
    voice: &crate::voice::Voice,
    slice: &str,
    x: f32,
    baseline: f32,
    limit: f32,
    ink: Srgba,
    list: &mut DrawList,
) {
    if slice.is_empty() {
        return;
    }
    let voiced = shape_voice(text, voice, slice);
    let set = voiced.set;
    for (glyph, offset) in voiced.placed() {
        let pen_x = x + offset;
        if pen_x >= limit {
            break;
        }
        let Some(placed) = text.glyph(glyph.font, glyph.glyph_id, set.size, pen_x) else {
            continue;
        };
        let entry = placed.entry;
        #[allow(clippy::cast_precision_loss)]
        list.instances.push(Instance::glyph(
            placed.pen + entry.left as f32,
            baseline - glyph.y - entry.top as f32,
            entry.width as f32,
            entry.height as f32,
            entry.uv,
            ink,
        ));
    }
}

#[cfg(test)]
#[path = "element_emit.tests.rs"]
mod tests;
