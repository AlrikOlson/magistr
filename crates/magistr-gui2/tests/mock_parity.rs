//! Compare native fixture renders with reference captures in both themes, using each
//! view's mask and difference threshold.
//!
//! These parity and structural checks are ignored in the standard suite because known
//! visual differences remain. `scripts/mock-parity-gate.sh` runs them explicitly with
//! `--run-ignored all --no-capture` and prints per-view measurements.
//!
//! Captures live in `tests/mocks/{night,day}-NN.png`, with masks in
//! `tests/mocks/NN.mask.json`. Failures write a `.diff.png`: grey marks unchanged
//! pixels, red differences, and blue excluded regions. Passing comparisons remove stale
//! difference images.

use std::path::{Path, PathBuf};

use magistr_cockpit::cockpit::{FrameClock, Model, Size};
use magistr_cockpit::theme::Theme;
use magistr_gui2::fixture::Fixture;
use magistr_gui2::{Frame, Mask, Medium, Parity, Verdict};
use qs_gpu::frame::{Instance, PrimKind};
use tiny_skia::Pixmap;

/// Where the captures, the masks and the diffs live.
fn mocks() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("mocks")
}

/// The clock every still is drawn at: frame 0, and the wall clock
/// pinned at the fixtures' own now, so a capture's ages are measured
/// and not left to the day the gate runs.
fn clock() -> FrameClock {
    FrameClock::new(0, std::time::Duration::ZERO).with_epoch_secs(Fixture::NOW_SECS)
}

/// `model` in `medium`'s palette, told it has a pixel layer, rendered
/// at `width` × `height` on the CPU tier.
fn render(frame: &mut Frame, model: Model, medium: Medium, width: u32, height: u32) -> Pixmap {
    let mut model = Fixture::with_pixels(model.with_theme(Theme::forced(medium.mode())));
    frame
        .still(&mut model, &clock(), width, height)
        .expect("the frame renders")
}

/// One row of the gate's table, with a sub-row per named region — the
/// primary signal, so a residual names a component or an arrangement
/// rather than one number per view.
struct Row {
    view: &'static str,
    medium: Medium,
    status: &'static str,
    detail: String,
    regions: Vec<String>,
}

impl Row {
    fn print(&self) {
        println!(
            "{:<4} {:<6} {:<5} {}",
            self.view, self.medium, self.status, self.detail
        );
        for region in &self.regions {
            println!("                {region}");
        }
    }
}

/// The gate's own test: every view id, in both media, against its
/// capture. Prints the whole table, then fails naming the views over
/// their threshold.
#[test]
#[ignore = "the mock-parity gate's test: FAILS by design until the views land; scripts/mock-parity-gate.sh runs it with --run-ignored"]
fn every_view_with_a_fixture_matches_its_capture() {
    let mut frame = Frame::new().expect("the embedded face yields a grid");
    let cell = frame.metrics();
    let size = Fixture::capture_grid((cell.width(), cell.height()));
    let (width, height) = Fixture::CAPTURE;
    let mut rows = Vec::new();
    let mut failed = Vec::new();
    for view in Fixture::VIEWS {
        let mask = Mask::load(&mocks().join(format!("{view}.mask.json")))
            .unwrap_or_else(|e| panic!("{e}"));
        for medium in Medium::ALL {
            let diff_path = mocks().join(format!("{}-{view}.diff.png", medium.prefix()));
            let Some(model) = Fixture::mock(view, size) else {
                forget(&diff_path);
                rows.push(Row {
                    view,
                    medium,
                    status: "SKIP",
                    detail: "no fixture yet (Fixture::mock answers None)".to_owned(),
                    regions: Vec::new(),
                });
                continue;
            };
            let capture = load_capture(&mocks().join(format!("{}-{view}.png", medium.prefix())));
            let still = render(&mut frame, model, medium, width, height);
            let verdict: Verdict = Parity::compare(&capture, &still, &mask, medium);
            if verdict.passes() {
                forget(&diff_path);
            } else {
                Parity::diff(&capture, &still, &mask, medium)
                    .save_png(&diff_path)
                    .unwrap_or_else(|e| panic!("writing {}: {e}", diff_path.display()));
                // The still itself, beside the diff — what the view
                // actually painted, measurable on its own pixels.
                let still_path = mocks().join(format!("{}-{view}.still.png", medium.prefix()));
                still
                    .save_png(&still_path)
                    .unwrap_or_else(|e| panic!("writing {}: {e}", still_path.display()));
                failed.push(format!(
                    "{view} {medium}: {verdict} → {}",
                    diff_path.display()
                ));
            }
            let masked = mask.exclusions(medium).count();
            rows.push(Row {
                view,
                medium,
                status: verdict.status(),
                detail: format!(
                    "{:.2}% of {} unmasked px differ (threshold {:.2}%); {masked} masked rect(s)",
                    verdict.share() * 100.0,
                    verdict.unmasked,
                    verdict.threshold * 100.0
                ),
                regions: verdict
                    .regions
                    .iter()
                    .map(|r| format!("└ {:<4} {r}", if r.passes() { "ok" } else { "MISS" }))
                    .collect(),
            });
        }
    }
    println!();
    println!(
        "mock parity — {}x{} px stills on a {}x{} cell grid, tolerance {}/255 per channel",
        width,
        height,
        size.cols,
        size.rows,
        Parity::TOLERANCE
    );
    println!("view medium status measured difference");
    for row in &rows {
        row.print();
    }
    println!();
    println!("notes:");
    println!("  - a view with no fixture is SKIP by name, never a pass");
    println!(
        "  - a └ sub-row is a named region measured on its own against its budget — the primary signal; the frame threshold is the backstop"
    );
    println!(
        "  - a diff is written beside the capture a view missed: tests/mocks/<medium>-NN.diff.png (grey same · red differs · blue masked)"
    );
    println!(
        "  - the CPU tier draws no Glow/Rim (Fidelity::Enhanced), so night halos are masked because the CPU tier does not render them"
    );
    println!();
    assert!(
        failed.is_empty(),
        "{} view still(s) over threshold:\n  {}",
        failed.len(),
        failed.join("\n  ")
    );
}

// ── The structural probes ──────────────────

/// A prohibition found on a frame.
struct Finding {
    surface: String,
    probe: &'static str,
    count: usize,
    first: Instance,
}

/// Every surface the probes sweep: the seven native fixtures at the
/// stills' grid, plus every mock view at the capture's grid — in both
/// media.
fn surfaces(frame: &mut Frame) -> Vec<(String, Medium, Pixmap, Vec<Instance>)> {
    type Surface = (&'static str, fn(Size) -> Model);
    const FIXTURES: [Surface; 7] = [
        ("home", Fixture::home),
        ("turn", Fixture::turn),
        ("screenshot", Fixture::screenshot),
        ("changes", Fixture::changed),
        ("trace", Fixture::traced),
        ("roadmap", Fixture::roadmapped),
        ("sheet", Fixture::sheeted),
    ];
    let cell = frame.metrics();
    let mut out = Vec::new();
    for medium in Medium::ALL {
        for (name, make) in FIXTURES {
            let size = Size::new(80, 24);
            let w = (f32::from(size.cols) * cell.width()) as u32;
            let h = (f32::from(size.rows) * cell.height()) as u32;
            let pixmap = render(frame, make(size), medium, w, h);
            out.push((
                name.to_owned(),
                medium,
                pixmap,
                frame.list().instances.clone(),
            ));
        }
        for view in Fixture::VIEWS {
            let size = Fixture::capture_grid((cell.width(), cell.height()));
            let Some(model) = Fixture::mock(view, size) else {
                continue;
            };
            let (w, h) = Fixture::CAPTURE;
            let pixmap = render(frame, model, medium, w, h);
            out.push((
                format!("mock-{view}"),
                medium,
                pixmap,
                frame.list().instances.clone(),
            ));
        }
    }
    out
}

/// Check native draw lists for prohibited visual treatments.
///
/// Reject colored severity strips and thin borders on every card. Allow at most one
/// filled primary action per frame, and reject chromatic glow in the light theme.
///
/// Icon placement is checked by the `IconRole` tests. Dashed outlines belong only to
/// empty slots.
#[test]
#[ignore = "the mock-parity gate's structural probes: run by scripts/mock-parity-gate.sh with --run-ignored; known visual differences may fail these probes"]
fn the_design_prohibitions_hold_on_every_native_frame() {
    let mut frame = Frame::new().expect("the embedded face yields a grid");
    let cell = frame.metrics();
    let (cw, ch) = (cell.width(), cell.height());
    let mut findings: Vec<Finding> = Vec::new();
    let mut swept = 0;
    for (name, medium, _pixmap, instances) in surfaces(&mut frame) {
        swept += 1;
        let surface = format!("{name}/{medium}");
        let mut probe = |probe: &'static str, hit: &dyn Fn(&Instance) -> bool| {
            let hits: Vec<&Instance> = instances.iter().filter(|i| hit(i)).collect();
            if let Some(first) = hits.first() {
                findings.push(Finding {
                    surface: surface.clone(),
                    probe,
                    count: hits.len(),
                    first: **first,
                });
            }
        };
        probe("§10 coloured left-border strip", &|i| {
            is(i, PrimKind::Rect) && i.rect[2] <= 4.0 && i.rect[3] >= 2.0 * ch && chromatic(i.color)
        });
        // A *card* is an elevated surface — it casts a shadow (a black
        // Glow dropped under the same box). The diff block's hairline
        // ring is a flat block boundary and legitimately 1px
        // (`DiffBlock`'s own doc), so the probe pairs the stroke with
        // the shadow rather than firing on every large ring.
        probe("§10 1px border on a card", &|i| {
            is(i, PrimKind::Stroke)
                && i.param <= 1.5
                && i.rect[2] >= 2.0 * cw
                && i.rect[3] >= 2.0 * ch
                && instances.iter().any(|j| casts_shadow_of(j, i))
        });
        // One lit exit: a lit exit is a *chromatic* gradient (the
        // green primary, the mauve accent). The volume's `.btn7`
        // secondaries are neutral ramps (white → paper by day) and are
        // not exits, so they do not count. The probe fires on the
        // second chromatic gradient, so a frame with one raised
        // primary is clean and one with two names the surplus.
        probe("§5 more than one lit exit", &|i| {
            let lit = |i: &Instance| is(i, PrimKind::Gradient) && chromatic(i.color);
            lit(i) && instances.iter().filter(|j| lit(j)).count() > 1
        });
        if medium == Medium::Day {
            probe("§4 ink twinkles (chromatic glow by day)", &|i| {
                is(i, PrimKind::Glow) && chromatic(i.color)
            });
        }
    }
    println!();
    println!("structural probes — {swept} native surfaces swept in both media");
    println!(
        "  measured: §10 no coloured left-border strip · §10 no 1px card border · §5 one lit exit · §4 no chromatic glow by day"
    );
    println!(
        "  by type:  §7 placement law (icon_role::tests::no_role_is_both_a_control_and_labelled)"
    );
    println!(
        "  by type:  §5 dashed outline only on slots (paint::tests::only_the_slot_wears_a_dashed_outline; per-view join pending)"
    );
    for f in &findings {
        let [x, y, w, h] = f.first.rect;
        println!(
            "  FAIL {:<18} {:<40} {} instance(s), first at ({x:.0},{y:.0}) {w:.0}x{h:.0}",
            f.surface, f.probe, f.count
        );
    }
    if findings.is_empty() {
        println!("  OK   no prohibition found");
    }
    println!();
    assert!(
        findings.is_empty(),
        "{} prohibition(s) found on native frames (listed above)",
        findings.len()
    );
}

fn is(instance: &Instance, kind: PrimKind) -> bool {
    instance.kind == kind as u32
}

/// Whether `glow` is an elevation shadow of the box `stroke` rings: a
/// dark Glow on the same box, dropped a few pixels — the shape
/// [`Elevation::shadows`] emits under every raised surface.
fn casts_shadow_of(glow: &Instance, stroke: &Instance) -> bool {
    let byte = |shift: u32| ((glow.color >> shift) & 0xFF) as u8;
    let dark = byte(0) < 16 && byte(8) < 16 && byte(16) < 16;
    is(glow, PrimKind::Glow)
        && dark
        && (glow.rect[0] - stroke.rect[0]).abs() <= 2.0
        && (glow.rect[2] - stroke.rect[2]).abs() <= 2.0
        && glow.rect[1] >= stroke.rect[1]
        && glow.rect[1] - stroke.rect[1] <= 12.0
}

/// Whether a packed premultiplied linear RGBA8 colour has a hue —
/// its channels spread by more than a shade.
fn chromatic(color: u32) -> bool {
    let byte = |shift: u32| ((color >> shift) & 0xFF) as u8;
    let (r, g, b) = (byte(0), byte(8), byte(16));
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    max - min > 40
}

fn load_capture(path: &Path) -> Pixmap {
    Pixmap::load_png(path).unwrap_or_else(|e| panic!("reading {}: {e}", path.display()))
}

fn forget(path: &Path) {
    let _ = std::fs::remove_file(path);
}
