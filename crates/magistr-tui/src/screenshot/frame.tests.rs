use super::*;

use std::time::Duration;

use crate::graphics::{Kitty, Passthrough};
use magistr_cockpit::cockpit::{Size, StageTab};
use magistr_cockpit::engine::EngineEvent;
use magistr_cockpit::theme::{Theme, ThemeMode};

/// The clock every still is rasterized against.
fn clock() -> FrameClock {
    FrameClock::new(7, Duration::from_millis(1500))
}

/// Kitty, unwrapped — the protocol a pixel profile composites through.
const KITTY: Protocol = Protocol::Kitty(Passthrough::Direct);

/// Pixel layers cannot bleed through the palette's opaque cell background.
#[test]
fn the_palette_masks_underlying_graphs_and_panel_rings() {
    use magistr_cockpit::harness::Scenes;
    use magistr_cockpit::layout::{Mark, marks};
    let mut model = Scenes::command_palette(Size::new(80, 24));
    update(&mut model, Msg::Mode(Decision::Graphics(KITTY).mode()));
    let root = view(&model, &clock());
    let resolved = layout(&model, &clock(), Profile::Kitty).expect("layout");
    let panel = marks(&root, &resolved)
        .into_iter()
        .rev()
        .find_map(|(rect, mark)| matches!(mark, Mark::Panel { .. }).then_some(rect))
        .expect("palette panel");
    let (_, images) = compose(&model, &clock(), KITTY).expect("frame");
    // The background in the panel's unused bottom padding must be solid. Old
    // graphs/rings crossing this rectangle would add a second colour.
    let [r, g, b] = magistr_cockpit::render::resolve(
        magistr_cockpit::theme::Pen::for_theme(&model.theme())
            .stage
            .bg,
        &model.theme().palette(),
        false,
    );
    let y = u32::from(panel.bottom() - 2) * 20 + 15;
    let image = images
        .iter()
        .find(|p| u32::from(p.row) * 20 <= y && y < u32::from(p.row + p.rows) * 20)
        .expect("strip");
    let local_y = y - u32::from(image.row) * 20;
    for x in u32::from(panel.x + 2) * 10..u32::from(panel.right() - 2) * 10 {
        let at = ((local_y * image.width + x) * 4) as usize;
        assert_eq!(
            &image.rgba[at..at + 4],
            &[r, g, b, 255],
            "opaque palette ground at {x},{y}"
        );
    }
}

#[test]
fn a_proportional_gauge_label_has_space_beside_its_bar() {
    let model = roadmapped();
    let resolved = layout(&model, &clock(), Profile::Kitty).expect("pixel layout");
    let placed = resolved.placed();
    let at = placed
        .iter()
        .position(|p| p.id == Some(magistr_cockpit::cockpit::Widget::RailGauge.id()))
        .expect("gauge");
    let bar = placed[at + 1].px;
    let label = placed[at + 2].px;
    assert!(
        placed[at].px.height >= label.height,
        "{:?}",
        &placed[at..at + 3]
    );
    assert!(label.width >= 25.0, "{:?}", &placed[at..at + 3]);
    assert!(label.x >= bar.right(), "the label clears the bar");
    assert!(label.height >= 20.0, "{:?}", &placed[at..at + 3]);
    let shot = frame(&model, &clock(), Profile::Kitty).expect("frame");
    let background = shot
        .pixel(label.x as u32, label.y as u32)
        .expect("background");
    assert!(
        (label.y as u32..label.bottom() as u32).any(|y| {
            (label.x as u32..label.right() as u32).any(|x| shot.pixel(x, y) != Some(background))
        }),
        "visible label: {:?}",
        &placed[at..at + 3]
    );
}

fn model() -> Model {
    Model::new(Size::new(120, 40)).with_theme(Theme::forced(ThemeMode::Dark))
}

/// The same cockpit on the Roadmap tab with a seeded store, so the
/// tech-tree and the rail's mini-DAG both have something to draw.
fn roadmapped() -> Model {
    let mut model = model().with_tab(StageTab::Roadmap);
    update(
        &mut model,
        Msg::Engine(EngineEvent::Seed(magistr_engine::NativeSnapshot {
            progress: magistr_cockpit::harness::fixtures::progress(1, 3),
            chunks: magistr_cockpit::harness::fixtures::chunks_from(vec![
                serde_json::json!({"id": "acp-2", "name": "ACP transport", "status": "done",
                    "priority": 100, "group": "Engine"}),
                serde_json::json!({"id": "tui2-2-rail", "name": "Rail and dock", "status": "in_progress",
                    "priority": 100, "group": "TUI rewrite", "deps": ["acp-2"]}),
                serde_json::json!({"id": "tui2-3-roadmap", "name": "Roadmap view", "status": "pending",
                    "priority": 200, "group": "TUI rewrite", "deps": ["tui2-2-rail"]}),
            ]),
            signals: (0..1)
                .map(|n| {
                    magistr_cockpit::harness::fixtures::signal_from(
                        serde_json::json!({"id": format!("s{n}"), "status": "new"}),
                    )
                })
                .collect(),
            ..magistr_engine::NativeSnapshot::default()
        })),
    );
    model
}

#[test]
fn a_cell_profile_places_nothing_and_a_kitty_profile_places_something() {
    let model = roadmapped();
    assert_eq!(
        Profile::Cells.protocol(),
        None,
        "the cell profile has no protocol to composite through"
    );
    let (_, images) = compose(&model, &clock(), KITTY).expect("layout succeeds");
    assert!(
        !images.is_empty(),
        "a kitty cockpit puts pixels on screen; nothing was decoded"
    );
}

/// The whole point of decoding rather than re-drawing: what the still
/// composites is what the frame actually wrote to the terminal. Every
/// placement is under the text, at the z the protocol names.
#[test]
fn every_placement_is_under_the_text_at_the_protocols_own_z() {
    let (_, images) = compose(&roadmapped(), &clock(), KITTY).expect("layout succeeds");
    for image in &images {
        assert_eq!(
            image.z,
            Kitty::Z_UNDER_TEXT,
            "a surface placed anywhere but under the text"
        );
    }
}

/// A placement's origin comes from the cursor the encoder moved. The
/// decoder refuses one it cannot locate, so a frame whose placements
/// all survived is the proof that the move is really always emitted —
/// the assumption the module's own doc rests on.
#[test]
fn every_surface_the_frame_shows_is_decoded() {
    let (_, bytes) = wire(&roadmapped(), &clock(), KITTY).expect("layout succeeds");
    let images = kitty::decode(&bytes);
    let placed = bytes.windows(6).filter(|w| *w == b"\x1b_Ga=p").count();
    assert!(placed > 0, "the frame placed nothing to count");
    assert_eq!(
        images.len(),
        placed,
        "a placement the decoder could not locate was dropped"
    );
}

/// Every decoded image really is `s = v = 4` bytes of straight RGBA,
/// which is what the compositing arithmetic indexes by.
#[test]
fn a_decoded_image_carries_exactly_its_own_pixels() {
    let (_, images) = compose(&roadmapped(), &clock(), KITTY).expect("layout succeeds");
    for image in &images {
        assert_eq!(
            image.rgba.len(),
            (image.width as usize) * (image.height as usize) * 4,
            "an image whose payload does not match its declared size"
        );
    }
}

/// The cell path is a shipped path, so a profile that has no protocol
/// must change nothing about it — the 38 checked-in stills say so, and
/// this says it without them.
#[test]
fn the_cell_profile_rasterizes_the_buffer_the_view_painted_and_no_more() {
    let model = roadmapped();
    let painted = cells(&model, &clock()).expect("layout succeeds");
    let still = frame(&model, &clock(), Profile::Cells).expect("layout succeeds");
    let direct = rasterize(
        &painted,
        &model.theme().palette(),
        &mut Typeface::new(),
        &[],
    );
    assert_eq!(still.data(), direct.data(), "no image path was taken");
}

/// A pixel still differs from its cell twin — otherwise the profile
/// would be decoration. The surfaces are placed over cells the view
/// deliberately left blank, so the difference is the pixels themselves.
#[test]
fn a_kitty_still_is_not_the_cell_still() {
    let model = roadmapped();
    let cells = frame(&model, &clock(), Profile::Cells).expect("layout succeeds");
    let pixels = frame(&model, &clock(), Profile::Kitty).expect("layout succeeds");
    assert_eq!(cells.width(), pixels.width(), "same frame, same size");
    assert_ne!(
        cells.data(),
        pixels.data(),
        "the kitty profile rasterized the cell fallback"
    );
}

/// The two passes exist because the tech-trees report their rect back
/// and the tab's layout depends on it. A third pass would report the
/// same, so composing twice is composing once — which is what makes a
/// still reproducible rather than dependent on how many frames ran.
#[test]
fn composing_is_settled_after_the_reporting_pass() {
    let model = roadmapped();
    let (first, one) = compose(&model, &clock(), KITTY).expect("layout succeeds");
    let (second, two) = compose(&model, &clock(), KITTY).expect("layout succeeds");
    assert_eq!(first.cells(), second.cells(), "the cells settled");
    assert_eq!(one, two, "the placements settled");
}

/// Which surfaces a launcher-sized kitty cockpit really places, as
/// their cell rects — a roll-call, so a surface that silently stops
/// being placed (the brand mark standing down, the gauge losing its
/// row) is a diff here rather than a still nobody re-reads.
#[test]
fn the_launcher_places_the_brand_and_the_roadmap_gauge() {
    let (cells, images) = compose(&roadmapped(), &clock(), KITTY).expect("layout succeeds");
    assert!(
        cells.cells().iter().all(|cell| cell.symbol == " "),
        "graphics typography leaves no terminal glyphs"
    );
    let rects: Vec<_> = images
        .iter()
        .map(|i| (i.col, i.row, i.cols, i.rows))
        .collect();
    let expected: Vec<_> = (0..40).step_by(4).map(|y| (0, y, 120, 4)).collect();
    assert_eq!(
        rects, expected,
        "every viewport row is covered exactly once"
    );
    assert!(
        images
            .iter()
            .all(|image| image.rgba.chunks_exact(4).all(|p| p[3] == 255)),
        "each strip is opaque"
    );
}
