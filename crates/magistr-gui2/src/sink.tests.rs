use std::path::PathBuf;

use magistr_cockpit::golden::Golden;
use magistr_cockpit::input::{KeyEvent, MouseEvent};
use tiny_skia::Pixmap;

use super::*;
use crate::atom_still::atom_still;

/// A viewport taller than the page, so a test click needs no scroll
/// arithmetic — view coordinates are content coordinates.
const VIEW: (f32, f32) = (1180.0, 6000.0);

fn engine() -> TextEngine {
    TextEngine::embedded()
}

/// Where `id` sits on the laid-out page — the synthetic pointer's aim.
fn center_of(sink: &Sink, id: ElementId, text: &mut TextEngine) -> (u32, u32) {
    let tree = sink.view(VIEW, text);
    let layout = ElementLayout::compute(&tree, VIEW.0, VIEW.1, text).expect("layout");
    let mut found = None;
    tree.each(&mut |index, element| {
        if element.id == Some(id) {
            found = Some(index);
        }
    });
    let [x, y, w, h] = layout.rect(found.unwrap_or_else(|| panic!("{id:?} is on the page")));
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    ((x + w / 2.0) as u32, (y + h / 2.0) as u32)
}

fn mouse(sink: &mut Sink, kind: MouseKind, at: (u32, u32), text: &mut TextEngine) -> bool {
    sink.fold(
        &Msg::Mouse {
            event: MouseEvent::new(kind, 0, 0).at_px(at.0, at.1),
            target: None,
        },
        VIEW,
        text,
    )
}

/// A full synthetic click: move over the element, press, release.
fn click(sink: &mut Sink, id: ElementId, text: &mut TextEngine) {
    let at = center_of(sink, id, text);
    mouse(sink, MouseKind::Move, at, text);
    mouse(sink, MouseKind::Down(Button::Left), at, text);
    mouse(sink, MouseKind::Up(Button::Left), at, text);
}

fn tick(sink: &mut Sink, times: u64, text: &mut TextEngine) {
    for _ in 0..times {
        sink.fold(&Msg::Tick, VIEW, text);
    }
}

/// The pointer toggles a switch, and Enter on the focus it left
/// toggles it back — one Msg loop, both routes.
#[test]
fn a_click_toggles_a_switch_and_enter_toggles_it_back() {
    let mut text = engine();
    let mut sink = Sink::new(Medium::Night);
    assert!(!sink.switch_b);
    click(&mut sink, Sink::SWITCH_B, &mut text);
    assert!(sink.switch_b, "the click toggled it on");
    assert_eq!(
        sink.state.focus(),
        Some(Sink::SWITCH_B),
        "the press focused it"
    );
    sink.fold(&Msg::Key(KeyEvent::new(KeyCode::Enter)), VIEW, &mut text);
    assert!(
        !sink.switch_b,
        "Enter on the focused switch toggled it back"
    );
}

/// A press dragged off its control releases into nothing — the click
/// semantics every toolkit keeps.
#[test]
fn a_drag_off_the_control_cancels_the_click() {
    let mut text = engine();
    let mut sink = Sink::new(Medium::Night);
    let at = center_of(&sink, Sink::SWITCH_B, &mut text);
    mouse(&mut sink, MouseKind::Move, at, &mut text);
    mouse(&mut sink, MouseKind::Down(Button::Left), at, &mut text);
    mouse(
        &mut sink,
        MouseKind::Drag(Button::Left),
        (at.0 + 300, at.1),
        &mut text,
    );
    mouse(
        &mut sink,
        MouseKind::Up(Button::Left),
        (at.0 + 300, at.1),
        &mut text,
    );
    assert!(!sink.switch_b, "released elsewhere, the switch stays put");
}

/// Segments select, theme cards choose, and the chosen state is what
/// the next frame paints.
#[test]
fn segments_select_and_theme_cards_choose() {
    let mut text = engine();
    let mut sink = Sink::new(Medium::Night);
    assert_eq!(sink.segment, 2);
    click(&mut sink, ElementId(Sink::SEGMENTS.0 + 1), &mut text);
    assert_eq!(sink.segment, 1, "the middle segment is chosen");
    assert_eq!(sink.theme, 0);
    click(&mut sink, Sink::THEME_OS, &mut text);
    assert_eq!(sink.theme, 1, "the follow-OS card is chosen");
}

/// The Day segment flips the whole page's medium.
#[test]
fn the_day_segment_flips_the_medium() {
    let mut text = engine();
    let mut sink = Sink::new(Medium::Night);
    click(&mut sink, ElementId(Sink::MEDIUM.0 + 1), &mut text);
    assert_eq!(sink.medium(), Medium::Day);
    click(&mut sink, Sink::MEDIUM, &mut text);
    assert_eq!(sink.medium(), Medium::Night);
}

/// Tab enters the focus ring at the page's first focusable element.
#[test]
fn tab_enters_the_focus_ring() {
    let mut text = engine();
    let mut sink = Sink::new(Medium::Night);
    sink.fold(&Msg::Key(KeyEvent::new(KeyCode::Tab)), VIEW, &mut text);
    assert_eq!(
        sink.state.focus(),
        Some(Sink::MEDIUM),
        "the ring starts at the medium segments"
    );
}

/// A scroll moves the page and clamps at both ends.
#[test]
fn scroll_moves_the_page_and_clamps() {
    let mut text = engine();
    let mut sink = Sink::new(Medium::Night);
    let window = (900.0, 400.0);
    let up = Msg::Mouse {
        event: MouseEvent::new(MouseKind::ScrollUp, 0, 0).at_px(100, 100),
        target: None,
    };
    let down = Msg::Mouse {
        event: MouseEvent::new(MouseKind::ScrollDown, 0, 0).at_px(100, 100),
        target: None,
    };
    assert!(!sink.fold(&up, window, &mut text), "already at the top");
    assert_eq!(sink.scroll, 0.0);
    assert!(sink.fold(&down, window, &mut text), "the page moved");
    assert!(sink.scroll > 0.0);
    for _ in 0..400 {
        sink.fold(&down, window, &mut text);
    }
    let limit = sink.content_height(window.0, &mut text) - window.1;
    assert!(
        (sink.scroll - limit).abs() < 0.5,
        "clamped at the bottom: {} vs {limit}",
        sink.scroll
    );
    assert!(!sink.fold(&down, window, &mut text), "and it stays there");
}

/// The meter's sweep is a pure function of the folded ticks: it starts
/// at zero on the trigger, rises monotonically, lands exactly on the
/// end frame, and two sinks folded identically agree to the bit.
#[test]
fn the_meter_refill_is_a_pure_function_of_ticks() {
    let mut text = engine();
    let mut sink = Sink::new(Medium::Night);
    assert!((sink.fill_progress() - 1.0).abs() < f32::EPSILON, "at rest");
    assert!(!sink.animating());
    click(&mut sink, Sink::REFILL, &mut text);
    assert!(
        sink.fill_progress().abs() < f32::EPSILON,
        "the sweep starts"
    );
    assert!(sink.animating());
    let mut last = 0.0;
    for _ in 0..Sink::FILL_STEPS {
        tick(&mut sink, 1, &mut text);
        let now = sink.fill_progress();
        assert!(now >= last, "monotone: {now} after {last}");
        last = now;
    }
    assert!((last - 1.0).abs() < f32::EPSILON, "the end frame");
    assert!(!sink.animating(), "and the loop can sleep");

    let mut twin = Sink::new(Medium::Night);
    let mut twin_text = engine();
    click(&mut twin, Sink::REFILL, &mut twin_text);
    tick(&mut twin, 5, &mut twin_text);
    let mut third = Sink::new(Medium::Night);
    let mut third_text = engine();
    click(&mut third, Sink::REFILL, &mut third_text);
    tick(&mut third, 5, &mut third_text);
    assert!(
        (twin.fill_progress() - third.fill_progress()).abs() < f32::EPSILON,
        "same folds, same frame"
    );
}

/// The deal reveals the rise cards one at a time, on the tick count
/// alone.
#[test]
fn the_deal_reveals_cards_on_ticks() {
    let mut text = engine();
    let mut sink = Sink::new(Medium::Night);
    assert!(sink.dealt(0) && sink.dealt(2), "at rest the table is full");
    click(&mut sink, Sink::DEAL, &mut text);
    assert!(sink.dealt(0), "the first card lands with the click");
    assert!(!sink.dealt(1) && !sink.dealt(2));
    tick(&mut sink, Sink::DEAL_STEP, &mut text);
    assert!(sink.dealt(1) && !sink.dealt(2));
    tick(&mut sink, Sink::DEAL_STEP, &mut text);
    assert!(sink.dealt(2), "the table fills");
    assert!(!sink.animating());
}

/// Reduced motion is structural: every transient renders its end frame
/// the moment it is triggered, and the loop never asks for a tick.
#[test]
fn reduced_motion_renders_every_end_frame() {
    let mut text = engine();
    let mut sink = Sink::new(Medium::Night);
    click(&mut sink, Sink::MOTION, &mut text);
    assert!(sink.reduced_motion);
    click(&mut sink, Sink::REFILL, &mut text);
    click(&mut sink, Sink::DEAL, &mut text);
    assert!(!sink.animating(), "nothing to sample");
    assert!(
        (sink.fill_progress() - 1.0).abs() < f32::EPSILON,
        "the meter is on its end frame"
    );
    assert!(
        sink.dealt(0) && sink.dealt(1) && sink.dealt(2),
        "every card is on the table"
    );
}

/// A pointer move reuses the cached scene — the revision holds still —
/// while everything that reshapes the page bumps it: a click, a
/// scroll, a mid-transient tick. This is the window's cue for when its
/// own scaled layout is stale, and the reason a hovered frame costs an
/// emit rather than a layout.
#[test]
fn hover_keeps_the_revision_and_page_changes_bump_it() {
    let mut text = engine();
    let mut sink = Sink::new(Medium::Night);
    let at = center_of(&sink, Sink::NAV_CHIP, &mut text);
    let settled = sink.revision();
    mouse(&mut sink, MouseKind::Move, at, &mut text);
    mouse(&mut sink, MouseKind::Move, (at.0 + 2, at.1), &mut text);
    assert_eq!(sink.revision(), settled, "hover is an emit, not a layout");
    assert!(sink.scene.is_some(), "and the scene stayed cached");

    click(&mut sink, Sink::SWITCH_B, &mut text);
    assert!(sink.revision() > settled, "a click reshapes the page");

    let scrolled = sink.revision();
    let down = Msg::Mouse {
        event: MouseEvent::new(MouseKind::ScrollDown, 0, 0).at_px(100, 100),
        target: None,
    };
    sink.fold(&down, (900.0, 400.0), &mut text);
    assert!(sink.revision() > scrolled, "a scroll moves the tree");

    let dealt = sink.revision();
    click(&mut sink, Sink::DEAL, &mut text);
    let triggered = sink.revision();
    assert!(triggered > dealt);
    tick(&mut sink, 1, &mut text);
    assert!(
        sink.revision() > triggered,
        "a mid-transient tick redraws a different page"
    );
}

/// Hovering an exhibit is registered against the placed tree — the
/// state the emitted faces are read from.
#[test]
fn the_pointer_hovers_the_exhibit_it_is_over() {
    let mut text = engine();
    let mut sink = Sink::new(Medium::Night);
    let at = center_of(&sink, Sink::NAV_CHIP, &mut text);
    mouse(&mut sink, MouseKind::Move, at, &mut text);
    assert_eq!(sink.state().hover(), Some(Sink::NAV_CHIP));
}

// ── The goldens: the whole page, and the transients as filmstrips ──

/// The sink's goldens, in this crate's own tree beside the native and
/// motion ones — a picture of the element substrate must not be able
/// to re-bless a cockpit still.
fn golden() -> Golden {
    Golden::new(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("golden")
            .join("sink"),
    )
}

/// `make`'s exhibit rasterized at its own natural height over `width`.
///
/// The exhibit goes inside a start-aligned wrapper for both the probe
/// and the picture: the probe viewport is arbitrarily tall, and an
/// exhibit that aligns its own children to its end would otherwise be
/// measured at the probe's bottom rather than at its natural extent.
fn exhibit_still(
    make: impl Fn() -> Element,
    medium: Medium,
    width: u32,
    text: &mut TextEngine,
) -> Pixmap {
    let height = exhibit_height(&make, width, text);
    exhibit_still_at(make, medium, width, height)
}

/// The natural height `make`'s exhibit lays out at over `width`.
fn exhibit_height(make: impl Fn() -> Element, width: u32, text: &mut TextEngine) -> u32 {
    let wrapped = Element::surface().align_start().child(make());
    #[allow(clippy::cast_precision_loss)]
    let layout = ElementLayout::compute(&wrapped, width as f32, 65536.0, text).expect("layout");
    let [_, y, _, h] = layout.rect(1);
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let height = (y + h).ceil() as u32;
    height.max(1)
}

/// `make`'s exhibit at a caller-fixed height — what a filmstrip whose
/// frames grow (the deal) renders every panel at, so the strip is one
/// picture rather than a crop of its tallest frame.
fn exhibit_still_at(make: impl Fn() -> Element, medium: Medium, width: u32, height: u32) -> Pixmap {
    let wrapped = Element::surface().align_start().child(make());
    atom_still(&wrapped, &ElementState::default(), medium, width, height)
        .expect("the still renders")
}

/// `panels` (all one size) side by side over a faint divider — the
/// same reviewed-as-one-picture form the motion strips take.
fn join(panels: &[Pixmap], lex: &Lexicon) -> Pixmap {
    let first = panels.first().expect("a strip is never empty");
    let (width, height) = (first.width(), first.height());
    for panel in panels {
        assert_eq!(
            (panel.width(), panel.height()),
            (width, height),
            "a strip's panels are one size — a bigger frame would be cropped quietly"
        );
    }
    let gutter = 8;
    #[allow(clippy::cast_possible_truncation)]
    let count = panels.len() as u32;
    let total = width * count + gutter * (count - 1);
    let mut out = Pixmap::new(total, height).expect("the strip allocates");
    let (r, g, b) = Lexicon::ink_bytes(lex.ov0);
    out.fill(tiny_skia::Color::from_rgba8(r, g, b, 255));
    for (index, panel) in panels.iter().enumerate() {
        #[allow(clippy::cast_possible_truncation)]
        let left = (width + gutter) * index as u32;
        for row in 0..height {
            let src = (row * width) as usize;
            let dst = (row * total + left) as usize;
            out.pixels_mut()[dst..dst + width as usize]
                .copy_from_slice(&panel.pixels()[src..src + width as usize]);
        }
    }
    out
}

/// The whole page, night and day — the executable specimen sheet as
/// two stills a reviewer can hold beside the volume.
#[test]
fn the_whole_sink_is_a_golden_in_both_media() {
    for medium in Medium::ALL {
        let mut text = engine();
        let sink = Sink::new(medium);
        const WIDTH: u32 = 1100;
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let height = sink.content_height(1100.0, &mut text) as u32;
        let still = atom_still(
            &sink.content(),
            &ElementState::default(),
            medium,
            WIDTH,
            height,
        )
        .expect("the sink renders");
        golden().assert_matches(&format!("sink-{medium}"), &still);
    }
}

/// The working breath as a filmstrip: four panels five ticks apart on the GLOW_STEPS
/// period — foot, rising, crest, falling — every adjacent pair a different frame, and
/// the first the rest frame the settled stills pin.
#[test]
fn the_working_breath_is_a_filmstrip_golden() {
    use magistr_cockpit::cockpit::{Msg, update};

    use crate::comp_feed::FeedComposition;
    let mut text = engine();
    let lex = Lexicon::of(Medium::Night);
    let mut model = Fixture::working(Size::new(120, 40));
    let mut panels = Vec::new();
    for panel in 0..4 {
        if panel > 0 {
            for _ in 0..5 {
                let _ = update(&mut model, Msg::Tick);
            }
        }
        panels.push(exhibit_still(
            || {
                Element::surface()
                    .width(560.0)
                    .align_start()
                    .child(FeedComposition::el(&model, Medium::Night, (9.0, 20.0)))
            },
            Medium::Night,
            560,
            &mut text,
        ));
    }
    for (index, pair) in panels.windows(2).enumerate() {
        assert_ne!(
            pair[0].data(),
            pair[1].data(),
            "panels {index} and {} are the same frame",
            index + 1
        );
    }
    golden().assert_matches("breathe-night", &join(&panels, &lex));
}

/// The meter's refill as a filmstrip: four panels stepped four ticks
/// apart, every adjacent pair a different frame — the sweep is on the
/// strip, not asserted from memory.
#[test]
fn the_meter_refill_is_a_filmstrip_golden() {
    let mut text = engine();
    let lex = Lexicon::of(Medium::Night);
    let mut sink = Sink::new(Medium::Night);
    click(&mut sink, Sink::REFILL, &mut text);
    let mut panels = Vec::new();
    for panel in 0..4 {
        if panel > 0 {
            tick(&mut sink, 4, &mut text);
        }
        panels.push(exhibit_still(
            || sink.meter_exhibit(&lex),
            Medium::Night,
            340,
            &mut text,
        ));
    }
    for (index, pair) in panels.windows(2).enumerate() {
        assert_ne!(
            pair[0].data(),
            pair[1].data(),
            "panels {index} and {} are the same frame",
            index + 1
        );
    }
    golden().assert_matches("meter-fill-night", &join(&panels, &lex));
}

/// The deal as a filmstrip: one card per [`Sink::DEAL_STEP`] ticks,
/// silhouettes standing where the cards will land.
#[test]
fn the_deal_is_a_filmstrip_golden() {
    let mut text = engine();
    let lex = Lexicon::of(Medium::Night);
    let mut sink = Sink::new(Medium::Night);
    // The settled table is the tallest frame — every panel renders at
    // its height, so the strip shows the cards arriving into place.
    let height = exhibit_height(|| sink.dealt_cards(&lex), 490, &mut text);
    click(&mut sink, Sink::DEAL, &mut text);
    let mut panels = Vec::new();
    for panel in 0..3 {
        if panel > 0 {
            tick(&mut sink, Sink::DEAL_STEP, &mut text);
        }
        panels.push(exhibit_still_at(
            || sink.dealt_cards(&lex),
            Medium::Night,
            490,
            height,
        ));
    }
    for (index, pair) in panels.windows(2).enumerate() {
        assert_ne!(
            pair[0].data(),
            pair[1].data(),
            "panels {index} and {} are the same frame",
            index + 1
        );
    }
    golden().assert_matches("deal-night", &join(&panels, &lex));
}

/// A finished transient and a reduced-motion trigger both land on the
/// bytes of the rest frame — "reduced motion renders every end frame"
/// measured on the pixels, not the state.
#[test]
fn the_end_frame_is_the_rest_frame_to_the_byte() {
    let lex = Lexicon::of(Medium::Night);
    let mut text = engine();
    let rest = Sink::new(Medium::Night);
    let rest_meter = exhibit_still(|| rest.meter_exhibit(&lex), Medium::Night, 340, &mut text);
    let rest_cards = exhibit_still(|| rest.dealt_cards(&lex), Medium::Night, 400, &mut text);

    let mut swept = Sink::new(Medium::Night);
    click(&mut swept, Sink::REFILL, &mut text);
    click(&mut swept, Sink::DEAL, &mut text);
    tick(&mut swept, Sink::FILL_STEPS, &mut text);
    assert_eq!(
        exhibit_still(|| swept.meter_exhibit(&lex), Medium::Night, 340, &mut text).data(),
        rest_meter.data(),
        "the sweep lands on the rest frame"
    );
    assert_eq!(
        exhibit_still(|| swept.dealt_cards(&lex), Medium::Night, 400, &mut text).data(),
        rest_cards.data(),
        "the deal lands on the rest frame"
    );

    let mut reduced = Sink::new(Medium::Night);
    click(&mut reduced, Sink::MOTION, &mut text);
    click(&mut reduced, Sink::REFILL, &mut text);
    click(&mut reduced, Sink::DEAL, &mut text);
    assert_eq!(
        exhibit_still(
            || reduced.meter_exhibit(&lex),
            Medium::Night,
            340,
            &mut text
        )
        .data(),
        rest_meter.data(),
        "reduced motion is the end frame at once"
    );
    assert_eq!(
        exhibit_still(|| reduced.dealt_cards(&lex), Medium::Night, 400, &mut text).data(),
        rest_cards.data(),
        "the deal too"
    );
}

/// Every section names at least one module and every entry carries a
/// non-empty exhibit — the catalog the roster test walks is real.
#[test]
fn the_catalog_is_sectioned_and_named() {
    let sink = Sink::new(Medium::Night);
    let catalog = sink.catalog();
    assert_eq!(
        catalog.len(),
        7,
        "the specimen sheet's six sections plus the a5 compositions"
    );
    for section in &catalog {
        assert!(!section.entries.is_empty(), "{} is empty", section.title);
        for (modules, tree) in &section.entries {
            assert!(!modules.is_empty());
            assert!(tree.count() > 1, "{modules:?} exhibits nothing");
        }
    }
}
