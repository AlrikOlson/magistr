use magistr_cockpit::layout::TextRole;
use magistr_cockpit::render::Style;

use crate::metrics::CellMetrics;
use crate::text::TextEngine;
use crate::type_scale::TypeScale;

fn engine() -> (TextEngine, TypeScale) {
    let mut text = TextEngine::embedded();
    let metrics = text
        .metrics(CellMetrics::DEFAULT_PX)
        .expect("the embedded mono face yields a grid");
    let scale = TypeScale::new(&metrics);
    (text, scale)
}

/// An untracked voice is the shaper's own answer, byte for byte: the
/// tracked path must cost the ordinary run nothing.
#[test]
fn an_untracked_voice_is_the_shapers_own_answer() {
    let (mut text, scale) = engine();
    let run = scale.shape(&mut text, TextRole::Prose, &Style::default(), "checkpoints");
    assert_eq!(run.tracking, 0.0);
    assert_eq!(run.width(), run.run.width);
    for (glyph, x) in run.placed() {
        assert_eq!(x, glyph.x);
    }
}

/// A tracked voice is wider than its ink by one gap per cluster,
/// trailing gap included — the browser's own arithmetic.
#[test]
fn tracking_adds_one_gap_per_cluster_including_the_last() {
    let (mut text, scale) = engine();
    let run = scale.shape(&mut text, TextRole::Cap, &Style::default(), "BUDGET");
    let track = scale.tracking(TextRole::Cap);
    assert!(track > 0.0, "the cap voice tracks");
    assert_eq!(run.clusters(), 6);
    #[allow(clippy::float_cmp)]
    let expected = run.run.width + track * 6.0;
    assert!((run.width() - expected).abs() < 1e-4);
}

/// Every glyph after the first is pushed right by its own share of the
/// tracking, so the run spreads rather than shifting.
#[test]
fn each_cluster_carries_its_own_share_of_the_tracking() {
    let (mut text, scale) = engine();
    let run = scale.shape(&mut text, TextRole::Cap, &Style::default(), "BUDGET");
    let track = scale.tracking(TextRole::Cap);
    for (index, (glyph, x)) in run.placed().enumerate() {
        #[allow(clippy::cast_precision_loss)]
        let expected = glyph.x + track * index as f32;
        assert!((x - expected).abs() < 1e-4, "glyph {index}");
    }
}

/// The cap voice uppercases before it shapes, so the painted text and
/// the measured text are the same string — which is the whole reason
/// the transform lives here and not at a call site.
#[test]
fn a_cap_is_shaped_from_the_uppercased_text() {
    let (mut text, scale) = engine();
    let run = scale.shape(&mut text, TextRole::Cap, &Style::default(), "Checkpoints");
    assert_eq!(run.text, "CHECKPOINTS");
    let shouted = scale.shape(&mut text, TextRole::Cap, &Style::default(), "CHECKPOINTS");
    #[allow(clippy::float_cmp)]
    {
        assert_eq!(run.width(), shouted.width());
    }
}

/// A voice with no transform leaves the text exactly as it was, so a
/// path, an id and a diff line survive being set.
#[test]
fn an_untransformed_voice_leaves_the_text_alone() {
    let (mut text, scale) = engine();
    for role in [TextRole::Code, TextRole::Value, TextRole::Prose] {
        let run = scale.shape(&mut text, role, &Style::default(), "cockpit/Surface.rs");
        assert_eq!(run.text, "cockpit/Surface.rs", "{role:?}");
    }
}
