//! Pixel layout and typography; the graphics and screenshot paths share this painter.

use super::pixel_type::PixelType;
use magistr_cockpit::layout::{
    Basis, LayoutError, LayoutTree, Measure, Node, Placed, PxRect, Resolved, TextRole,
};
use magistr_cockpit::pixel::CellSize;
use magistr_cockpit::render::{Attrs, Style, resolve};
use magistr_theme::Palette;
use tiny_skia::{BlendMode, Paint, Pixmap, Rect, Transform};

/// Cached type measurement and the two layers around the vector/image surfaces.
#[derive(Debug, Default)]
pub(crate) struct PixelView {
    face: PixelType,
}

#[cfg(test)]
#[path = "pixel_view.tests.rs"]
mod tests;

impl PixelView {
    pub(crate) fn layout(
        &mut self,
        tree: &mut LayoutTree,
        root: &Node,
        cols: u16,
        rows: u16,
        cell: CellSize,
    ) -> Result<Resolved, LayoutError> {
        self.face.set_cell(cell);
        let basis = Basis::new(f32::from(cell.width), f32::from(cell.height));
        tree.compute_on(
            root,
            basis.across(cols),
            basis.down(rows),
            basis,
            &mut self.face,
        )
    }

    pub(crate) fn layers(
        &mut self,
        root: &Node,
        resolved: &Resolved,
        cols: u16,
        rows: u16,
        cell: CellSize,
        palette: &Palette,
    ) -> (Pixmap, Pixmap) {
        let (width, height) = cell.pixels(cols, rows);
        let mut ground = Pixmap::new(u32::from(width.max(1)), u32::from(height.max(1)))
            .expect("bounded pixel frame");
        let mut ink = ground.clone();
        let style = root.cell_style().cloned().unwrap_or_default();
        let mut placed = resolved.placed().iter();
        self.node(root, &mut placed, &style, &mut ground, &mut ink, palette);
        (ground, ink)
    }

    fn node(
        &mut self,
        node: &Node,
        placed: &mut std::slice::Iter<'_, Placed>,
        inherited: &Style,
        ground: &mut Pixmap,
        ink: &mut Pixmap,
        palette: &Palette,
    ) {
        let Some(place) = placed.next() else {
            return;
        };
        let cell = self.face.cell();
        let rect = place.px;
        let bounds = place.bounds;
        let style = node.cell_style().unwrap_or(inherited);
        let (_, bg) = colors(style, palette);
        if node.cell_style().is_some() && node.text_metrics().is_none() {
            fill(ground, rect, bg, BlendMode::Source);
            // Pure pixel overlays bring their own opaque ground into the final
            // ink layer, above every graph/image surface composed between layers.
            // Framed terminal panels retain the mark compositor's border mask.
            let cover = if place.layer > 0 && node.frame_spec().is_none() {
                bg
            } else {
                [0, 0, 0, 0]
            };
            fill(ink, rect, cover, BlendMode::Source);
        }
        if let Some(frame) = node.frame_spec() {
            let title = frame.title_text().unwrap_or_default();
            if !title.is_empty() {
                let title = format!(" {title} ");
                let width = self
                    .face
                    .intrinsic(&magistr_cockpit::layout::TextMetrics::new(&title))
                    .0
                    .min((rect.width - f32::from(cell.width) * 4.0).max(0.0));
                let title_rect = PxRect::new(
                    rect.x + f32::from(cell.width) * 2.0,
                    rect.y,
                    width,
                    f32::from(cell.height),
                );
                fill(ink, title_rect, bg, BlendMode::SourceOver);
                self.text(
                    ink,
                    &title,
                    title_rect,
                    frame.heading_style(),
                    TextRole::Heading,
                    palette,
                );
            }
            if let Some(meta) = frame.meta_text() {
                let width = self.face.advance(meta, TextRole::Label);
                let meta_rect = PxRect::new(
                    (rect.right() - f32::from(cell.width) * 2.0 - width).max(rect.x),
                    rect.y,
                    width.min(rect.width),
                    f32::from(cell.height),
                );
                self.text(
                    ink,
                    meta,
                    meta_rect,
                    frame.border_style(),
                    TextRole::Label,
                    palette,
                );
            }
        }
        if let Some(cells) = node.cell_row() {
            for (offset, value) in cells.iter().enumerate() {
                if value.is_continuation() {
                    continue;
                }
                let x = bounds.x + offset as f32 * f32::from(cell.width);
                let span = f32::from(value.width().max(1)) * f32::from(cell.width);
                let box_bounds = PxRect::new(x, bounds.y, span, f32::from(cell.height));
                let box_rect = box_bounds.intersection(rect);
                let (_, bg) = colors(&value.style, palette);
                fill(ground, box_rect, bg, BlendMode::Source);
                self.text_clipped(
                    ink,
                    &value.symbol,
                    box_bounds,
                    box_rect,
                    &value.style,
                    TextRole::Code,
                    palette,
                );
            }
        }
        if let Some(text) = node.text_metrics() {
            let line_height = f32::from(cell.height);
            if let Some(cursor) = text.cursor() {
                self.editor(ink, text.text(), cursor, rect, style, palette);
            } else {
                for (row, line) in self
                    .face
                    .wrap(text, bounds.width + 0.01)
                    .into_iter()
                    .enumerate()
                {
                    let line_bounds = PxRect::new(
                        bounds.x,
                        bounds.y + row as f32 * line_height,
                        bounds.width,
                        line_height,
                    );
                    let line_rect = line_bounds.intersection(rect);
                    if line_rect.is_empty() {
                        continue;
                    }
                    if node.cell_style().is_some() {
                        fill(ground, line_rect, bg, BlendMode::SourceOver);
                    }
                    self.text_clipped(
                        ink,
                        line,
                        line_bounds,
                        line_rect,
                        style,
                        text.role(),
                        palette,
                    );
                }
            }
        }
        for child in node.child_nodes() {
            self.node(child, placed, style, ground, ink, palette);
        }
    }

    /// Grapheme-safe soft wrapping and cursor-following scrolling use the same pixel ruler.
    fn editor(
        &mut self,
        canvas: &mut Pixmap,
        text: &str,
        cursor: usize,
        rect: PxRect,
        style: &Style,
        palette: &Palette,
    ) {
        let line_height = f32::from(self.face.cell().height);
        use unicode_segmentation::UnicodeSegmentation as _;
        let mut rows = Vec::new();
        let mut start = 0;
        let mut used = 0.0;
        let mut word_break = None;
        let limit = (rect.width - 3.0).max(1.0);
        for (at, grapheme) in text.grapheme_indices(true) {
            if grapheme == "\n" || grapheme == "\r\n" {
                rows.push((start, at));
                start = at + grapheme.len();
                used = 0.0;
                word_break = None;
                continue;
            }
            let width = self.face.advance(grapheme, TextRole::Steer);
            if used > 0.0 && used + width > limit {
                let (end, before) = word_break.take().unwrap_or((at, used));
                rows.push((start, end));
                start = end;
                used -= before;
            }
            used += width;
            if grapheme.chars().all(char::is_whitespace) {
                word_break = Some((at + grapheme.len(), used));
            }
        }
        rows.push((start, text.len()));
        let selected = rows
            .iter()
            .rposition(|(start, _)| *start <= cursor)
            .unwrap_or(0);
        let visible = (rect.height / line_height).floor().max(1.0) as usize;
        let top = (selected + 1).saturating_sub(visible);
        for (offset, &(start, end)) in rows.iter().skip(top).take(visible).enumerate() {
            let line = PxRect::new(
                rect.x,
                rect.y + offset as f32 * line_height,
                rect.width,
                line_height,
            )
            .intersection(rect);
            self.text(
                canvas,
                &text[start..end],
                line,
                style,
                TextRole::Steer,
                palette,
            );
            if top + offset == selected {
                let x = rect.x
                    + self
                        .face
                        .advance(&text[start..cursor.min(end)], TextRole::Steer);
                let (fg, _) = colors(style, palette);
                fill(
                    canvas,
                    PxRect::new(
                        x.round().min(rect.right() - 2.0),
                        line.y + 2.0,
                        2.0,
                        (line.height - 4.0).max(0.0),
                    ),
                    fg,
                    BlendMode::SourceOver,
                );
            }
        }
    }

    fn text(
        &mut self,
        canvas: &mut Pixmap,
        text: &str,
        rect: PxRect,
        style: &Style,
        role: TextRole,
        palette: &Palette,
    ) {
        self.text_clipped(canvas, text, rect, rect, style, role, palette);
    }

    #[allow(clippy::too_many_arguments)]
    fn text_clipped(
        &mut self,
        canvas: &mut Pixmap,
        text: &str,
        rect: PxRect,
        clip: PxRect,
        style: &Style,
        role: TextRole,
        palette: &Palette,
    ) {
        if clip.is_empty() {
            return;
        }
        let (fg, _) = colors(style, palette);
        let baseline = rect.y + self.face.baseline(role);
        let mut x = rect.x;
        for ch in text.chars() {
            let counted = role
                .is_counted()
                .then(|| self.face.advance(ch.encode_utf8(&mut [0; 4]), role));
            let (metrics, bitmap) = self.face.glyph(ch, role, style.attrs.contains(Attrs::BOLD));
            let left = x.round() as i32 + metrics.xmin;
            let top = baseline.round() as i32 - metrics.height as i32 - metrics.ymin;
            for gy in 0..metrics.height {
                for gx in 0..metrics.width {
                    let py = top + gy as i32;
                    let shear = if style.attrs.contains(Attrs::ITALIC) {
                        ((baseline - py as f32) * 0.25).round() as i32
                    } else {
                        0
                    };
                    let px = left + gx as i32 + shear;
                    if px < 0
                        || py < 0
                        || px >= canvas.width() as i32
                        || py >= canvas.height() as i32
                        || !clip.contains(f64::from(px), f64::from(py))
                    {
                        continue;
                    }
                    let coverage = bitmap[gy * metrics.width + gx];
                    let at = (py as usize * canvas.width() as usize + px as usize) * 4;
                    let dst = &mut canvas.data_mut()[at..at + 4];
                    let a = u32::from(coverage);
                    for c in 0..3 {
                        dst[c] = ((u32::from(fg[c]) * a + u32::from(dst[c]) * (255 - a) + 127)
                            / 255) as u8;
                    }
                    dst[3] = (a + (u32::from(dst[3]) * (255 - a) + 127) / 255) as u8;
                }
            }
            x += counted.unwrap_or(metrics.advance_width);
        }
        if style.attrs.underline().is_some() || style.link.is_some() {
            let color = match style.underline_color {
                magistr_cockpit::render::Color::Default => fg,
                color => {
                    let [r, g, b] = resolve(color, palette, true);
                    [r, g, b, 255]
                }
            };
            use magistr_cockpit::render::Underline;
            let underline = style.attrs.underline().unwrap_or(Underline::Single);
            let mid = (baseline + 2.0).round().min(rect.bottom() - 2.0);
            for offset in 0..(x - rect.x).min(rect.width).ceil() as u32 {
                let ys: &[f32] = match underline {
                    Underline::Single => &[0.0],
                    Underline::Double => &[-1.0, 1.0],
                    Underline::Curly => match offset % 4 {
                        0 => &[-1.0],
                        2 => &[1.0],
                        _ => &[0.0],
                    },
                    Underline::Dotted if offset % 2 == 0 => &[0.0],
                    Underline::Dashed if offset % 6 < 4 => &[0.0],
                    _ => &[],
                };
                for y in ys {
                    fill(
                        canvas,
                        PxRect::new(rect.x + offset as f32, mid + y, 1.0, 1.0).intersection(clip),
                        color,
                        BlendMode::SourceOver,
                    );
                }
            }
        }
        if style.attrs.contains(Attrs::STRIKETHROUGH) {
            fill(
                canvas,
                PxRect::new(
                    rect.x,
                    rect.y + rect.height * 0.52,
                    (x - rect.x).min(rect.width),
                    1.0,
                ),
                fg,
                BlendMode::SourceOver,
            );
        }
    }
}

fn colors(style: &Style, palette: &Palette) -> ([u8; 4], [u8; 4]) {
    let mut fg = resolve(style.fg, palette, true);
    let mut bg = resolve(style.bg, palette, false);
    if style.attrs.contains(Attrs::REVERSE) {
        std::mem::swap(&mut fg, &mut bg);
    }
    if style.attrs.contains(Attrs::DIM) {
        for i in 0..3 {
            fg[i] = ((u16::from(fg[i]) * 45 + u16::from(bg[i]) * 55) / 100) as u8;
        }
    }
    ([fg[0], fg[1], fg[2], 255], [bg[0], bg[1], bg[2], 255])
}

fn fill(canvas: &mut Pixmap, rect: PxRect, rgba: [u8; 4], blend_mode: BlendMode) {
    if let Some(rect) = Rect::from_xywh(rect.x, rect.y, rect.width, rect.height) {
        let mut paint = Paint {
            blend_mode,
            ..Paint::default()
        };
        paint.set_color_rgba8(rgba[0], rgba[1], rgba[2], rgba[3]);
        canvas.fill_rect(rect, &paint, Transform::identity(), None);
    }
}
