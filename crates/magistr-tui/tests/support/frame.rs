//! A cockpit-shaped frame shared by the benchmark, the performance gate and
//! the end-to-end test: header, rail (24 cols of text), stage (growing,
//! wrapping stream text), dock (30 %), footer — laid out, painted and
//! presented every frame. One painter ([`paint_frame`]) serves both the
//! frame that is presented and the buffer a test expects a terminal to show.

use magistr_cockpit::WidgetId;
use magistr_cockpit::layout::{LayoutTree, Length, Node, Placed, Resolved};
use magistr_cockpit::render::{Buffer, Color, Style};
use magistr_tui::render::Screen;

/// Columns of the benchmark screen.
pub const WIDTH: u16 = 120;
/// Rows of the benchmark screen.
pub const HEIGHT: u16 = 40;

const STREAM: &str = "The agent reads the roadmap chunk, records a think step, plans the ship tasks and starts editing; \
every tool call streams back here as it happens, wrapped to the stage width.";

/// The cockpit tree for `frame`, with a counter in the header and one
/// stream line (the first, so it is on screen at every size) that changes
/// every frame so the diff is never empty. The stream is taller than any
/// stage: the body row gets `min_height(0)` so flexbox may shrink it to the
/// screen instead of pushing the footer off the bottom (CSS `min-size:
/// auto` on a flex item is its content size — 60 wrapped rows here).
pub fn cockpit(frame: u64) -> Node {
    let rail = Node::column()
        .id(WidgetId::new(1))
        .width(Length::Cells(24))
        .shrink(0.0)
        .padding(1)
        .children((0..8).map(|n| {
            Node::text(format!("chunk-{n}: tui2-0a{n} layout")).id(WidgetId::new(100 + n))
        }));
    let stage = Node::column()
        .id(WidgetId::new(2))
        .grow(1.0)
        .padding(1)
        .children((0..20).map(|n| {
            let line = if n == 0 {
                format!("frame {frame}: {STREAM}")
            } else {
                format!("{n:02} {STREAM}")
            };
            Node::text(line).id(WidgetId::new(200 + n))
        }));
    let dock =
        Node::column()
            .id(WidgetId::new(3))
            .width(Length::Percent(0.3))
            .shrink(0.0)
            .padding(1)
            .children((0..8).map(|n| {
                Node::text(format!("gate {n}: PASS in 0.{n}s")).id(WidgetId::new(300 + n))
            }));
    Node::column()
        .child(
            Node::text(format!("magistr — frame {frame}"))
                .id(WidgetId::new(0))
                .height(Length::Cells(1)),
        )
        .child(
            Node::row()
                .grow(1.0)
                .viewport()
                .child(rail)
                .child(stage)
                .child(dock),
        )
        .child(
            Node::text("q quit · tab focus · ? help")
                .id(WidgetId::new(4))
                .height(Length::Cells(1)),
        )
}

fn paint(
    node: &Node,
    placed: &mut std::slice::Iter<'_, Placed>,
    buffer: &mut Buffer,
    fill: &str,
    style: &Style,
) {
    let Some(place) = placed.next() else {
        return;
    };
    let rect = place.rect;
    if let Some(text) = node.text_metrics() {
        for (row, line) in text
            .wrap(rect.width)
            .into_iter()
            .take(usize::from(rect.height))
            .enumerate()
        {
            let row = u16::try_from(row).expect("rows fit u16");
            buffer.put_str(rect.x, rect.y + row, line, style);
        }
    } else if place.id.is_some() {
        let run = &fill[..usize::from(rect.width).min(fill.len())];
        for row in rect.y..rect.bottom() {
            buffer.put_str(rect.x, row, run, style);
        }
    }
    for child in node.child_nodes() {
        paint(child, placed, buffer, fill, style);
    }
}

/// Lay `root` out at `buffer`'s size and paint it there: wrapped text rows
/// for text leaves, a `·` fill for id-carrying containers, one pen for all.
/// Returns the layout so a caller can hit-test the very rects it painted.
pub fn paint_frame(tree: &mut LayoutTree, root: &Node, buffer: &mut Buffer) -> Resolved {
    let (width, height) = (buffer.width(), buffer.height());
    let resolved = tree.compute(root, width, height).expect("layout succeeds");
    let fill = "·".repeat(usize::from(width));
    let style = Style::new().fg(Color::Rgb(200, 200, 210));
    paint(root, &mut resolved.placed().iter(), buffer, &fill, &style);
    resolved
}

/// Lay out, paint and present one frame; returns the bytes emitted.
pub fn render_frame(
    tree: &mut LayoutTree,
    screen: &mut Screen,
    out: &mut Vec<u8>,
    frame: u64,
) -> usize {
    let root = cockpit(frame);
    paint_frame(tree, &root, screen.buffer());
    out.clear();
    screen.present(out);
    out.len()
}
