//! Putting a laid-out view into the cell buffer.

use crate::layout::{Frame, Node, Placed, Rect, Resolved};
use crate::render::{Buffer, Cell, Style};

/// Paint `root` into `buffer` at the rects `resolved` gave its nodes
/// (the result of laying `root` out at the buffer's size): the buffer
/// filled with blanks in the root's style when it has one — the theme's
/// base — then every framed container's rounded border ring (title
/// embedded in the top border, meta right-aligned), children over it
/// inside the ring the layout engine reserved, every cell band written
/// cell for cell across its rect, and every text leaf as
/// wrapped rows clipped to its rect, in the nearest style on its path
/// from the root (the default pen when there is none).
pub fn paint(root: &Node, resolved: &Resolved, buffer: &mut Buffer) {
    let style = root.cell_style().cloned().unwrap_or_default();
    if root.cell_style().is_some() {
        buffer.fill(&Cell::new(" ", style.clone()));
    }
    let mut placed = resolved.placed().iter();
    paint_node(root, &mut placed, buffer, &style);
}

fn paint_node(
    node: &Node,
    placed: &mut std::slice::Iter<'_, Placed>,
    buffer: &mut Buffer,
    inherited: &Style,
) {
    let Some(place) = placed.next() else {
        return;
    };
    let style = node.cell_style().unwrap_or(inherited);
    // A container that carries its own style is a band: its whole rect
    // wears the background, gaps and tails included — floating text is
    // a leaf's business, never a bar's. Text leaves keep painting only
    // their glyph rows, so a styled code row does not smear its surface
    // across the stage.
    if node.cell_style().is_some() && node.text_metrics().is_none() {
        fill_rect(place.rect, buffer, style);
    }
    if let Some(frame) = node.frame_spec() {
        paint_frame(frame, place.rect, buffer);
    }
    if let Some(cells) = node.cell_row() {
        let rect = place.rect;
        for (offset, cell) in (0..rect.width).zip(cells) {
            if !cell.is_continuation() {
                buffer.put_str_in(
                    Rect::new(rect.x + offset, rect.y, rect.width - offset, 1),
                    &cell.symbol,
                    &cell.style,
                );
            }
        }
    }
    if let Some(text) = node.text_metrics() {
        let rect = place.rect;
        let rows = text
            .wrap(rect.width)
            .into_iter()
            .take(usize::from(rect.height));
        for (offset, line) in (0..).zip(rows) {
            buffer.put_str_in(
                Rect::new(rect.x, rect.y + offset, rect.width, 1),
                line,
                style,
            );
        }
    }
    for child in node.child_nodes() {
        paint_node(child, placed, buffer, style);
    }
}

/// Every cell of `rect` blanked in `style`.
fn fill_rect(rect: Rect, buffer: &mut Buffer, style: &Style) {
    if rect.width == 0 {
        return;
    }
    let blanks = " ".repeat(usize::from(rect.width));
    for row in rect.y..rect.bottom() {
        buffer.put_str(rect.x, row, &blanks, style);
    }
}

/// The rounded ring on `rect`: corners, edges, the title embedded after
/// the top-left corner, the meta right-aligned before the top-right one.
/// A rect under 2×2 has no ring to draw and is left alone; a title or
/// meta that does not fit is clipped, the ring never is. A
/// [`drawn`](Frame::is_drawn) frame leaves the ring's cells blank for
/// the pixel layer and writes only its title and meta.
fn paint_frame(frame: &Frame, rect: Rect, buffer: &mut Buffer) {
    if rect.width < 2 || rect.height < 2 {
        return;
    }
    let style = frame.border_style();
    let right = rect.right() - 1;
    let bottom = rect.bottom() - 1;
    if !frame.is_drawn() {
        let run = "─".repeat(usize::from(rect.width) - 2);
        buffer.put_str(rect.x, rect.y, &format!("╭{run}╮"), style);
        buffer.put_str(rect.x, bottom, &format!("╰{run}╯"), style);
        for row in rect.y + 1..bottom {
            buffer.put_str(rect.x, row, "│", style);
            buffer.put_str(right, row, "│", style);
        }
    }
    // The slot between `╭─` and `─╮`: the title from the left, the meta
    // from the right, each wrapped in one space and cut to what fits.
    let slot = usize::from(rect.width).saturating_sub(4);
    let mut used = 0;
    if let Some((col, cols)) = frame.title_span(rect.width) {
        let text: String = frame
            .title_text()
            .unwrap_or_default()
            .chars()
            .take(usize::from(cols) - 2)
            .collect();
        used = 2 + buffer.put_str(
            rect.x + col,
            rect.y,
            &format!(" {text} "),
            frame.heading_style(),
        );
    }
    if let Some(meta) = frame.meta_text().filter(|m| !m.is_empty()) {
        let free = slot.saturating_sub(usize::from(used));
        if free > 2 {
            let text: String = meta.chars().take(free - 2).collect();
            let columns = u16::try_from(text.chars().count() + 2).unwrap_or(u16::MAX);
            buffer.put_str(right - 1 - columns, rect.y, &format!(" {text} "), style);
        }
    }
}

#[cfg(test)]
#[path = "paint.tests.rs"]
mod tests;
