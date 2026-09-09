use magistr_cockpit::cockpit::Msg;
use magistr_cockpit::input::{Button, MouseEvent, MouseKind};
use magistr_theme::Palette;
use qs_gpu::cpu_raster::CpuRasterizer;
use qs_gpu::frame::PrimKind;

use super::*;
use crate::element_id::ElementId;
use crate::elevation::Elevation;
use crate::voice::Voice;

const W: f32 = 240.0;
const H: f32 = 120.0;

fn palette() -> Palette {
    magistr_cockpit::theme::Theme::forced(magistr_cockpit::theme::ThemeMode::Dark).palette()
}

fn label() -> Voice {
    Voice {
        family: "sans",
        px: 12.0,
        weight: 550,
        tracking_em: 0.0,
        line_height: 0.0,
        upper: false,
    }
}

/// The acceptance's minimal specimen: one card, one button, one text
/// run — every primitive class the substrate emits, on one small tree.
fn specimen() -> Element {
    let palette = palette();
    let ground = |c: magistr_theme::ThemeColor| {
        Srgba::new(
            f32::from(c.r) / 255.0,
            f32::from(c.g) / 255.0,
            f32::from(c.b) / 255.0,
            1.0,
        )
    };
    let base = ground(palette.surface0);
    let hover = ground(palette.surface1);
    let press = ground(palette.surface2);
    let ink = ground(palette.text);
    let accent = ground(palette.blue);
    Element::surface()
        .pad(16.0)
        .fill(ground(palette.base))
        .child(
            Element::surface()
                .pad(12.0)
                .gap(8.0)
                .fill(base)
                .radius(11.0)
                .elevation(Elevation::row())
                .child(Element::text(label(), ink, "one card, one button"))
                .child(
                    Element::row()
                        .width(96.0)
                        .height(28.0)
                        .center()
                        .justify_center()
                        .gradient(accent, press)
                        .hover_fill(hover)
                        .press_fill(press)
                        .radius(8.0)
                        .elevation(Elevation::raised())
                        .id(ElementId(1))
                        .focusable()
                        .focus_ring(accent)
                        .child(Element::text(label(), ink, "Run")),
                ),
        )
}

/// One full pass: fresh engine, layout, emission, a finished list.
fn built(state: &ElementState) -> (qs_gpu::frame::DrawList, TextEngine) {
    let mut text = TextEngine::embedded();
    let tree = specimen();
    let layout = ElementLayout::compute(&tree, W, H, &mut text).expect("layout");
    let mut list = qs_gpu::frame::DrawList::default();
    text.begin_frame();
    list.reset([W as u32, H as u32], Srgba::new(0.0, 0.0, 0.0, 1.0), 1);
    emit(&tree, &layout, state, &mut text, &mut list, 0..tree.count());
    list.end_batch(None, true);
    list.finish();
    (list, text)
}

/// Emission over a DFS index window draws exactly that subtree —
/// [`Element::each`]'s numbering is contiguous per subtree, which is
/// what lets the frame put the stage in its own scissored batch.
#[test]
fn a_dfs_range_emits_exactly_its_subtree() {
    let state = ElementState::default();
    let (whole, _) = built(&state);
    let mut text = TextEngine::embedded();
    let tree = specimen();
    let layout = ElementLayout::compute(&tree, W, H, &mut text).expect("layout");
    let mut list = qs_gpu::frame::DrawList::default();
    text.begin_frame();
    list.reset([W as u32, H as u32], Srgba::new(0.0, 0.0, 0.0, 1.0), 1);
    // The specimen's root is one flat fill; index 1 opens the card's
    // subtree, which is everything else.
    emit(
        &tree,
        &layout,
        &state,
        &mut text,
        &mut list,
        1..tree.count(),
    );
    list.end_batch(None, true);
    list.finish();
    assert_eq!(
        list.instances.len(),
        whole.instances.len() - 1,
        "the window left out exactly the root's own fill"
    );
}

fn kinds(list: &qs_gpu::frame::DrawList) -> Vec<PrimKind> {
    list.instances
        .iter()
        .filter_map(|i| PrimKind::from_raw(i.kind))
        .collect()
}

fn count(list: &qs_gpu::frame::DrawList, kind: PrimKind) -> usize {
    kinds(list).into_iter().filter(|k| *k == kind).count()
}

/// Every field of every instance, in order — byte identity for a frame
/// that is a draw list.
fn bytes(list: &qs_gpu::frame::DrawList) -> Vec<u32> {
    list.instances
        .iter()
        .flat_map(|i| {
            i.rect
                .iter()
                .chain(i.uv.iter())
                .chain([i.radius, i.param].iter())
                .map(|f| f.to_bits())
                .chain([i.color, i.kind])
                .collect::<Vec<u32>>()
        })
        .collect()
}

fn hover_at_button() -> ElementState {
    let mut text = TextEngine::embedded();
    let tree = specimen();
    let layout = ElementLayout::compute(&tree, W, H, &mut text).expect("layout");
    let [x, y, ..] = layout.rect(3);
    let mut state = ElementState::default();
    let over = Msg::Mouse {
        event: MouseEvent::new(MouseKind::Move, 0, 0).at_px(x as u32 + 4, y as u32 + 4),
        target: None,
    };
    assert!(state.fold(&over, &tree, &layout));
    state
}

#[test]
fn the_specimen_emits_every_primitive_class_it_declares() {
    let (list, _) = built(&ElementState::default());
    assert!(count(&list, PrimKind::Rect) >= 2, "the canvas and the card");
    assert!(count(&list, PrimKind::Gradient) == 1, "the one lit exit");
    assert!(count(&list, PrimKind::Glow) >= 2, "two shadow layers fall");
    assert!(count(&list, PrimKind::Glyph) > 0, "the runs are real ink");
}

#[test]
fn hover_and_press_change_the_face_and_only_the_face() {
    let rest = built(&ElementState::default()).0;
    let hover = built(&hover_at_button()).0;
    // The button wears a gradient at rest, so hovering swaps it for the
    // hover fill — one gradient fewer, one rect more.
    assert_eq!(count(&hover, PrimKind::Gradient), 0);
    assert_eq!(
        count(&hover, PrimKind::Rect),
        count(&rest, PrimKind::Rect) + 1
    );
    assert_eq!(
        count(&hover, PrimKind::Glyph),
        count(&rest, PrimKind::Glyph),
        "the ink does not move for a pointer"
    );
}

#[test]
fn focus_draws_its_ring_and_blur_removes_it() {
    let mut text = TextEngine::embedded();
    let tree = specimen();
    let layout = ElementLayout::compute(&tree, W, H, &mut text).expect("layout");
    let [x, y, ..] = layout.rect(3);
    let mut state = ElementState::default();
    let down = Msg::Mouse {
        event: MouseEvent::new(MouseKind::Down(Button::Left), 0, 0)
            .at_px(x as u32 + 4, y as u32 + 4),
        target: None,
    };
    state.fold(&down, &tree, &layout);
    let focused = built(&state).0;
    let rest = built(&ElementState::default()).0;
    assert_eq!(
        focused.instances.len(),
        rest.instances.len() + 1,
        "the ring is exactly one more instance"
    );
    let up = Msg::Mouse {
        event: MouseEvent::new(MouseKind::Up(Button::Left), 0, 0).at_px(x as u32 + 4, y as u32 + 4),
        target: None,
    };
    state.fold(&up, &tree, &layout);
    let away = Msg::Mouse {
        event: MouseEvent::new(MouseKind::Down(Button::Left), 0, 0).at_px(2, 2),
        target: None,
    };
    state.fold(&away, &tree, &layout);
    assert_eq!(state.focus(), None, "a click on nothing blurs");
}

/// A box is painted on the device grid the way the browser paints one: layout runs at
/// fractional pixels — the header's 51.75 puts every box under it on a.75 edge — and
/// the capture shows each ring, hairline and track crisp on whole rows because Chrome
/// snaps a painted box's edges to the nearest pixel. Here: the origin rounds, the far
/// edge rounds, the size is their difference; a hairline on a.75 edge lands on one row,
/// never smeared over two; and a box thinner than half a pixel keeps one pixel rather
/// than vanishing. Text is not touched — its baseline is already the engine's own whole
/// pixel.
#[test]
fn a_painted_box_snaps_to_the_device_grid() {
    let fill = Srgba::new(0.2, 0.3, 0.4, 1.0);
    let ring = Srgba::new(0.5, 0.6, 0.7, 1.0);
    let tree = Element::surface()
        .pad_y(10.75)
        .pad_x(3.25)
        .align_start()
        .child(
            Element::surface()
                .width(41.5)
                .height(17.5)
                .fill(fill)
                .stroke(ring, 1.0),
        )
        .child(Element::surface().width(100.0).height(1.0).fill(ring))
        .child(Element::surface().width(100.0).height(0.4).fill(fill))
        .child(
            Element::surface()
                .width(15.5)
                .height(15.5)
                .radius(7.75)
                .fill(fill)
                .exact(),
        );
    let mut text = TextEngine::embedded();
    let layout = ElementLayout::compute(&tree, W, H, &mut text).expect("layout");
    assert_eq!(
        layout.rect(1),
        [3.25, 10.75, 41.5, 17.5],
        "layout stays fractional"
    );
    let mut list = qs_gpu::frame::DrawList::default();
    text.begin_frame();
    list.reset([W as u32, H as u32], Srgba::new(0.0, 0.0, 0.0, 1.0), 1);
    emit(
        &tree,
        &layout,
        &ElementState::default(),
        &mut text,
        &mut list,
        0..tree.count(),
    );
    list.end_batch(None, true);
    list.finish();
    let rects: Vec<[f32; 4]> = list
        .instances
        .iter()
        .filter(|i| PrimKind::from_raw(i.kind) == Some(PrimKind::Rect))
        .map(|i| i.rect)
        .collect();
    assert_eq!(
        rects,
        [
            [3.0, 11.0, 42.0, 17.0],
            [3.0, 28.0, 100.0, 1.0],
            [3.0, 29.0, 100.0, 1.0],
            [3.25, 29.65, 15.5, 15.5],
        ],
        "the box (3.25→3, 44.75→45; 10.75→11, 28.25→28), the hairline at 28.25 on row 28 alone, the sliver at 29.25 kept as one row, and the exact circle where layout put it"
    );
    let stroke = list
        .instances
        .iter()
        .find(|i| PrimKind::from_raw(i.kind) == Some(PrimKind::Stroke))
        .expect("the ring");
    assert_eq!(
        stroke.rect,
        [3.0, 11.0, 42.0, 17.0],
        "the ring rides the snapped box"
    );
}

/// A surface's fill arrives in its instance verbatim, elevation or
/// not: the volume's palette bakes the lift into its own colors, so a
/// card filled with `mantle` must put down exactly `mantle` — the
/// property every a3 parity measurement stands on.
#[test]
fn a_fill_is_painted_exactly_even_at_elevation() {
    let fill = Srgba::new(0.2, 0.3, 0.4, 1.0);
    let card = Element::surface()
        .width(64.0)
        .height(32.0)
        .fill(fill)
        .radius(13.0)
        .elevation(Elevation::popover());
    let mut text = TextEngine::embedded();
    let layout = ElementLayout::compute(&card, 64.0, 32.0, &mut text).expect("layout");
    let mut list = qs_gpu::frame::DrawList::default();
    text.begin_frame();
    list.reset([64, 32], Srgba::new(0.0, 0.0, 0.0, 1.0), 1);
    emit(
        &card,
        &layout,
        &ElementState::default(),
        &mut text,
        &mut list,
        0..card.count(),
    );
    list.end_batch(None, true);
    list.finish();
    let painted = list
        .instances
        .iter()
        .find(|i| PrimKind::from_raw(i.kind) == Some(PrimKind::Rect))
        .expect("the card's band");
    assert_eq!(
        painted.color,
        fill.to_premul_linear_rgba8(),
        "the fill is the fill, at any elevation"
    );
}

#[test]
fn two_fresh_builds_are_byte_identical_on_the_list_and_on_the_cpu_tier() {
    let (first, mut text_a) = built(&ElementState::default());
    let (second, mut text_b) = built(&ElementState::default());
    assert_eq!(bytes(&first), bytes(&second), "the list is deterministic");
    let raster = |list: &qs_gpu::frame::DrawList, text: &mut TextEngine| {
        let mut cpu = CpuRasterizer::with_colour_page(
            W as u32,
            H as u32,
            text.atlas_size(),
            text.colour_size(),
        )
        .expect("the cpu tier accepts the viewport");
        cpu.upload_glyphs(&text.take_uploads());
        cpu.upload_images(&text.take_image_uploads());
        cpu.render(list);
        cpu.to_srgb_rgba8()
    };
    let a = raster(&first, &mut text_a);
    let b = raster(&second, &mut text_b);
    assert_eq!(a, b, "the pixels are deterministic");
    assert!(a.iter().any(|byte| *byte != 0), "and not a blank canvas");
}

/// A Cells leaf paints the way a terminal does: one quad per run of
/// cells sharing a ground, glyphs for every written cell — on the bold
/// cut when asked — a one-pixel rule per run of underlined cells, and
/// nothing at all for the quiet cells the program never wrote to.
#[test]
fn cells_emit_ground_runs_glyphs_and_rules() {
    use crate::pane_tile::PaneTile;
    let ink = Srgba::new(0.9, 0.9, 0.9, 1.0);
    let red = Srgba::new(0.8, 0.2, 0.2, 1.0);
    let cell = |t: &str, ground: Option<Srgba>, bold: bool, underline: bool| PaneTile {
        text: t.to_owned(),
        ink,
        ground,
        bold,
        italic: false,
        underline,
    };
    // Row 0: three cells on one red ground, then a plain one, then two
    // quiet spaces. Row 1: two underlined bold cells, then quiet.
    let rows = vec![
        vec![
            cell("a", Some(red), false, false),
            cell("b", Some(red), false, false),
            cell("c", Some(red), false, false),
            cell("d", None, false, false),
            cell(" ", None, false, false),
            cell(" ", None, false, false),
        ],
        vec![
            cell("e", None, true, true),
            cell("f", None, true, true),
            cell(" ", None, false, false),
        ],
    ];
    let mut text = TextEngine::embedded();
    let tree = Element::surface()
        .align_start()
        .child(Element::grid(rows, 11.0));
    let layout = ElementLayout::compute(&tree, W, H, &mut text).expect("layout");
    let mut list = qs_gpu::frame::DrawList::default();
    text.begin_frame();
    list.reset([W as u32, H as u32], Srgba::new(0.0, 0.0, 0.0, 1.0), 1);
    emit(
        &tree,
        &layout,
        &ElementState::default(),
        &mut text,
        &mut list,
        0..tree.count(),
    );
    list.end_batch(None, true);
    list.finish();
    let metrics = text.metrics(11.0).expect("metrics");
    let rects: Vec<_> = list
        .instances
        .iter()
        .filter(|i| PrimKind::from_raw(i.kind) == Some(PrimKind::Rect))
        .collect();
    let grounds: Vec<_> = rects
        .iter()
        .filter(|i| i.color == red.to_premul_linear_rgba8())
        .collect();
    assert_eq!(grounds.len(), 1, "one quad for the run of three red cells");
    assert_eq!(grounds[0].rect[2], 3.0 * metrics.width());
    assert_eq!(grounds[0].rect[3], metrics.height());
    let rules: Vec<_> = rects
        .iter()
        .filter(|i| i.rect[3] == 1.0 && i.color == ink.to_premul_linear_rgba8())
        .collect();
    assert_eq!(rules.len(), 1, "one rule under the two underlined cells");
    assert_eq!(rules[0].rect[2], 2.0 * metrics.width());
    let glyphs = count(&list, PrimKind::Glyph);
    assert_eq!(glyphs, 6, "a b c d e f — and nothing for the quiet spaces");
}
