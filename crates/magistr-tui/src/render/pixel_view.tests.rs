use super::*;

#[test]
fn translated_prose_keeps_its_baseline_under_a_viewport_clip() {
    let mut painter = PixelView::default();
    let palette = magistr_cockpit::theme::Theme::default().palette();
    let render = |painter: &mut PixelView, offset: f32| {
        let root = Node::column().viewport().child(
            Node::text("The saved result")
                .text_role(TextRole::Prose)
                .height(magistr_cockpit::layout::Length::Px(20.0))
                .shrink(0.0)
                .offset_y(offset)
                .style(Style::new().fg(Color::Rgb(255, 255, 255))),
        );
        let resolved = painter
            .layout(&mut LayoutTree::new(), &root, 20, 2, CellSize::DEFAULT)
            .unwrap();
        let placed = &resolved.placed()[1];
        assert_eq!(placed.bounds.y, offset);
        assert_eq!(placed.px.y, offset.max(0.0));
        painter
            .layers(&root, &resolved, 20, 2, CellSize::DEFAULT, &palette)
            .1
    };
    let full = render(&mut painter, 0.0);
    let clipped = render(&mut painter, -7.0);
    for y in 0..13usize {
        let stride = 200 * 4;
        assert_eq!(
            &clipped.data()[y * stride..(y + 1) * stride],
            &full.data()[(y + 7) * stride..(y + 8) * stride]
        );
    }
}

use magistr_cockpit::render::Color;

#[test]
fn wrapped_unicode_editor_keeps_its_end_caret_visible_inside_the_viewport() {
    let mut painter = PixelView::default();
    let mut tree = LayoutTree::new();
    let draft = "small words ".repeat(40) + " café e\u{301} 漢字";
    let root = Node::editor(&draft, draft.len()).style(
        Style::new()
            .fg(Color::Rgb(255, 255, 255))
            .bg(Color::Rgb(0, 0, 0)),
    );
    let resolved = painter
        .layout(&mut tree, &root, 12, 2, CellSize::DEFAULT)
        .expect("layout");
    let (_, ink) = painter.layers(
        &root,
        &resolved,
        12,
        2,
        CellSize::DEFAULT,
        &magistr_cockpit::theme::Theme::default().palette(),
    );
    // A 2px × 16px caret must survive on the final visible row.
    let caret = (0..119).any(|x| {
        (22..38).all(|y| {
            let at = (y * 120 + x) * 4;
            ink.data()[at + 3] == 255 && ink.data()[at + 7] == 255
        })
    });
    assert!(
        caret,
        "the editor scrolled to its cursor, not its first lines"
    );
}

fn reference_glyph_paint(
    canvas: &mut Pixmap,
    face: &mut PixelType,
    text: &str,
    boxes: (PxRect, PxRect),
    style: &Style,
    role: TextRole,
    palette: &magistr_theme::Palette,
) {
    let (rect, clip) = boxes;
    let (fg, _) = colors(style, palette);
    let baseline = rect.y + face.baseline(role);
    let mut x = rect.x;
    for ch in text.chars() {
        let counted = role
            .is_counted()
            .then(|| face.advance(ch.encode_utf8(&mut [0; 4]), role));
        let (metrics, bitmap) = face.glyph(ch, role, style.attrs.contains(Attrs::BOLD));
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
                let at = (py as usize * canvas.width() as usize + px as usize) * 4;
                let dst = &mut canvas.data_mut()[at..at + 4];
                let a = u32::from(bitmap[gy * metrics.width + gx]);
                for c in 0..3 {
                    dst[c] =
                        ((u32::from(fg[c]) * a + u32::from(dst[c]) * (255 - a) + 127) / 255) as u8;
                }
                dst[3] = (a + (u32::from(dst[3]) * (255 - a) + 127) / 255) as u8;
            }
        }
        x += counted.unwrap_or(metrics.advance_width);
    }
}

#[test]
fn scanline_glyph_paint_is_byte_identical_at_fractional_clips_and_font_sizes() {
    let palette = magistr_cockpit::theme::Theme::default().palette();
    let text = "Saved café e\u{301} 漢字 WWW iii";
    for cell in [
        CellSize::new(7, 17),
        CellSize::DEFAULT,
        CellSize::new(13, 29),
    ] {
        let mut painter = PixelView::default();
        let mut previous = PixelType::default();
        painter.face.set_cell(cell);
        previous.set_cell(cell);
        for role in [TextRole::Prose, TextRole::Code, TextRole::Data] {
            for attrs in [
                Attrs::NONE,
                Attrs::BOLD,
                Attrs::ITALIC,
                Attrs::BOLD | Attrs::ITALIC,
                Attrs::DIM,
                Attrs::REVERSE,
            ] {
                let style = Style::new()
                    .fg(Color::Rgb(177, 231, 149))
                    .bg(Color::Rgb(31, 19, 51))
                    .attrs(attrs);
                for rect in [
                    PxRect::new(3.7, 4.2, 220.0, 40.0),
                    PxRect::new(-5.3, -7.2, 220.0, 40.0),
                ] {
                    for clip in [
                        PxRect::new(0.0, 0.0, 200.0, 60.0),
                        PxRect::new(13.25, 6.75, 88.2, 18.5),
                    ] {
                        let mut expected = Pixmap::new(200, 60).unwrap();
                        expected.fill(tiny_skia::Color::from_rgba8(35, 91, 117, 163));
                        let mut actual = expected.clone();
                        reference_glyph_paint(
                            &mut expected,
                            &mut previous,
                            text,
                            (rect, clip),
                            &style,
                            role,
                            &palette,
                        );
                        painter.text_clipped(&mut actual, text, rect, clip, &style, role, &palette);
                        let different = actual
                            .data()
                            .iter()
                            .zip(expected.data())
                            .position(|(a, b)| a != b);
                        assert_eq!(
                            different, None,
                            "{cell:?} {role:?} {attrs:?} {rect:?} {clip:?}"
                        );
                    }
                }
            }
        }
    }
}

#[test]
#[ignore = "release-mode glyph blend comparison"]
fn report_glyph_blend_cost() {
    use std::time::Instant;

    let palette = magistr_cockpit::theme::Theme::default().palette();
    let text =
        "A worker resumes the saved retry schedule after a restart. Saved café e\u{301} 漢字.";
    let style = Style::new().fg(Color::Rgb(177, 231, 149));
    let mut painter = PixelView::default();
    let mut previous = PixelType::default();
    let mut actual = Pixmap::new(1440, 1000).unwrap();
    let mut expected = actual.clone();
    let clip = PxRect::new(20.5, 50.5, 920.5, 900.5);
    let mut times = [Vec::new(), Vec::new()];
    for frame in 0..220 {
        for reference in [false, true] {
            let start = Instant::now();
            for line in 0..40 {
                let rect = PxRect::new(24.7, 40.3 + line as f32 * 22.0, 930.0, 22.0);
                if reference {
                    reference_glyph_paint(
                        &mut expected,
                        &mut previous,
                        text,
                        (rect, clip),
                        &style,
                        TextRole::Prose,
                        &palette,
                    );
                } else {
                    painter.text_clipped(
                        &mut actual,
                        text,
                        rect,
                        clip,
                        &style,
                        TextRole::Prose,
                        &palette,
                    );
                }
            }
            if frame >= 20 {
                times[usize::from(reference)].push(start.elapsed());
            }
        }
    }
    assert_eq!(actual.data(), expected.data());
    for (name, times) in ["scanline", "reference"].into_iter().zip(&mut times) {
        times.sort_unstable();
        println!(
            "{name}: p50 {:.1} µs, p95 {:.1} µs",
            times[100].as_secs_f64() * 1e6,
            times[190].as_secs_f64() * 1e6,
        );
    }
}

/// Reports the production pixel text/background path separately from model
/// updates and cached graph/mark surfaces. Run optimized, without parallel tests.
#[test]
#[ignore = "release-mode graphics phase report"]
fn report_graphics_frame_phase_costs() {
    use crate::graphics::{Passthrough, Protocol};
    use crate::pixel::{Compression, IdSpace};
    use crate::render::Screen;
    use magistr_cockpit::agent::AgentEvent;
    use magistr_cockpit::capability::Mode;
    use magistr_cockpit::cockpit::{FrameClock, Msg, Size, update, view};
    use magistr_cockpit::harness::Scenes;
    use std::time::{Duration, Instant};

    const WARMUP: usize = 10;
    const SAMPLES: usize = 120;
    for size in [Size::new(144, 50), Size::new(128, 40)] {
        let cell = CellSize::DEFAULT;
        let mut model = Scenes::streaming(size);
        update(&mut model, Msg::Agent(AgentEvent::Text(
            "\n\nEach delivery keeps its own saved retry budget. A worker resumes the saved schedule after a restart, and the recovery checks confirm that customer limits stay separate.".repeat(80),
        )));
        update(
            &mut model,
            Msg::Mode(Mode::Graphics {
                pointer_shapes: true,
            }),
        );
        if !model.reasoning_expanded() {
            update(
                &mut model,
                Msg::Mouse {
                    event: magistr_cockpit::input::MouseEvent::new(
                        magistr_cockpit::input::MouseKind::Down(
                            magistr_cockpit::input::Button::Left,
                        ),
                        0,
                        0,
                    ),
                    target: Some(magistr_cockpit::cockpit::Widget::ReasoningToggle.id()),
                },
            );
        }
        Scenes::settle(&mut model);
        let palette = model.theme().palette();
        let mut painter = PixelView::default();
        let mut tree = LayoutTree::new();
        let mut screen = Screen::new(size.cols, size.rows);
        screen.enable_graphics(
            Protocol::Kitty(Passthrough::Direct),
            Compression::DEFAULT,
            IdSpace::FIRST,
        );
        let mut output = Vec::new();
        let mut samples: [Vec<Duration>; 4] = std::array::from_fn(|_| Vec::new());
        let mut bytes = 0;
        for index in 0..WARMUP + SAMPLES {
            model.reveal_conversation((index % 30) * 2);
            let start = Instant::now();
            let root = view(&model, &FrameClock::new(0, Duration::ZERO));
            let built = Instant::now();
            let resolved = painter
                .layout(&mut tree, &root, size.cols, size.rows, cell)
                .unwrap();
            let laid_out = Instant::now();
            let (ground, ink) =
                painter.layers(&root, &resolved, size.cols, size.rows, cell, &palette);
            let painted = Instant::now();
            output.clear();
            screen.begin_pixels(ground);
            screen.finish_pixels(&ink, cell);
            screen.present(&mut output);
            let finished = Instant::now();
            if index >= WARMUP {
                for (times, elapsed) in samples.iter_mut().zip([
                    built - start,
                    laid_out - built,
                    painted - laid_out,
                    finished - painted,
                ]) {
                    times.push(elapsed);
                }
                bytes += output.len();
            }
        }
        println!(
            "graphics {}x{} bytes/frame {}",
            u32::from(size.cols) * u32::from(cell.width),
            u32::from(size.rows) * u32::from(cell.height),
            bytes / SAMPLES
        );
        for (phase, times) in ["view", "layout", "text paint", "compose+encode"]
            .into_iter()
            .zip(&mut samples)
        {
            times.sort_unstable();
            println!(
                "{phase:>16}: p50 {:>7.1} µs, p95 {:>7.1} µs",
                times[SAMPLES / 2].as_secs_f64() * 1e6,
                times[SAMPLES * 95 / 100].as_secs_f64() * 1e6
            );
        }
    }
    report_large_trace_frame_costs();
}

fn report_large_trace_frame_costs() {
    use crate::graphics::{Passthrough, Protocol};
    use crate::pixel::{Compression, IdSpace};
    use crate::render::Screen;
    use crate::runtime::Surfaces;
    use magistr_cockpit::agent::AgentEvent;
    use magistr_cockpit::capability::Mode;
    use magistr_cockpit::cockpit::{FrameClock, Msg, Size, Widget, update, view};
    use magistr_cockpit::harness::{Scenes, fixtures};
    use magistr_cockpit::input::{Button, MouseEvent, MouseKind};
    use magistr_engine::NativeSnapshot;
    use std::collections::BTreeSet;
    use std::time::{Duration, Instant};

    const WARMUP: usize = 10;
    const SAMPLES: usize = 120;
    let steps = (0..2000)
        .map(|index| {
            let number = 1000 + index * 13;
            let mut step = fixtures::step(number);
            step.purpose = format!("Check saved delivery {}", index + 1);
            step.thought = "Check the recorded receipt before retrying the delivery.".into();
            step.outcome = "The receipt preserves the next attempt.".into();
            step.rationale = "The retry schedule and receipt are saved together.".into();
            let offset = index % 256;
            if (1..=240).contains(&offset) {
                let branch = index / 256 * 2 + offset % 2;
                step.branch_id = Some(format!("recovery-{branch}"));
                step.branch_name = Some(format!(
                    "Does restart case {} keep its receipt?",
                    branch + 1
                ));
                step.branch_from = Some(1000 + (index / 256 * 256) * 13);
            }
            step
        })
        .collect::<Vec<_>>();
    let branches = steps
        .iter()
        .filter_map(|step| step.branch_id.as_deref())
        .collect::<BTreeSet<_>>()
        .len();
    assert_eq!(branches, 16);
    let snapshot = NativeSnapshot {
        steps,
        ..NativeSnapshot::default()
    };

    for size in [Size::new(144, 50), Size::new(128, 40)] {
        let mut seeded = Scenes::streaming(size).with_snapshot(&snapshot);
        update(&mut seeded, Msg::Agent(AgentEvent::Text(
            "\n\nEach delivery keeps its own saved retry budget. A worker resumes the saved schedule after a restart, and the recovery checks confirm that customer limits stay separate.".repeat(80),
        )));
        for branch in 0..14 {
            Scenes::engine_frame(
                &mut seeded,
                "think",
                "branch_status_changed",
                serde_json::json!({
                    "branch_id":format!("recovery-{branch}"),
                    "status":if branch == 3 { "abandoned" } else { "merged" },
                }),
            );
        }
        update(
            &mut seeded,
            Msg::Mode(Mode::Graphics {
                pointer_shapes: true,
            }),
        );
        assert_eq!(seeded.trace().steps().len(), 2000);
        for expanded in [true, false] {
            let mut model = seeded.clone();
            if model.reasoning_expanded() != expanded {
                update(
                    &mut model,
                    Msg::Mouse {
                        event: MouseEvent::new(MouseKind::Down(Button::Left), 0, 0),
                        target: Some(Widget::ReasoningToggle.id()),
                    },
                );
            }
            Scenes::settle(&mut model);
            let mut surfaces = Surfaces::default();
            let mut tree = LayoutTree::new();
            let mut screen = Screen::new(size.cols, size.rows);
            screen.enable_graphics(
                Protocol::Kitty(Passthrough::Direct),
                Compression::DEFAULT,
                IdSpace::FIRST,
            );
            let mut output = Vec::new();
            let mut samples: [Vec<Duration>; 5] = std::array::from_fn(|_| Vec::new());
            let mut bytes = 0;
            for index in 0..WARMUP + SAMPLES {
                model.reveal_conversation((index % 30) * 2);
                let start = Instant::now();
                let root = view(&model, &FrameClock::new(0, Duration::ZERO));
                let built = Instant::now();
                let resolved = surfaces.layout(&root, &model, &mut tree).unwrap();
                let laid_out = Instant::now();
                magistr_cockpit::render::paint(&root, &resolved, screen.buffer());
                surfaces.show(&root, &mut model, &resolved, &mut screen);
                let painted = Instant::now();
                output.clear();
                screen.present(&mut output);
                let finished = Instant::now();
                if index >= WARMUP {
                    for (times, elapsed) in samples.iter_mut().zip([
                        built - start,
                        laid_out - built,
                        painted - laid_out,
                        finished - painted,
                        finished - start,
                    ]) {
                        times.push(elapsed);
                    }
                    bytes += output.len();
                }
            }
            println!(
                "full graphics {}x{} steps={} branches={branches} reasoning={} bytes/frame={}",
                u32::from(size.cols) * u32::from(model.cell_size().width),
                u32::from(size.rows) * u32::from(model.cell_size().height),
                model.trace().steps().len(),
                if expanded { "expanded" } else { "collapsed" },
                bytes / SAMPLES,
            );
            for (phase, times) in ["view", "layout", "paint+encode", "present", "whole frame"]
                .into_iter()
                .zip(&mut samples)
            {
                times.sort_unstable();
                println!(
                    "{phase:>16}: p50 {:>7.1} µs, p95 {:>7.1} µs",
                    times[SAMPLES / 2].as_secs_f64() * 1e6,
                    times[SAMPLES * 95 / 100].as_secs_f64() * 1e6,
                );
            }
        }
    }
}

#[test]
fn labels_use_fractional_pixel_rects_instead_of_counted_columns() {
    let mut painter = PixelView::default();
    let root = Node::row()
        .child(Node::text("iii"))
        .child(Node::text("WWW"));
    let resolved = painter
        .layout(&mut LayoutTree::new(), &root, 40, 2, CellSize::DEFAULT)
        .expect("layout");
    let placed = resolved.placed();
    assert!(placed[1].px.width < placed[2].px.width);
    assert_ne!(placed[1].px.width % 10.0, 0.0);
}
