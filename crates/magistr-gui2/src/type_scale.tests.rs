use super::*;

use crate::text::TextEngine;

fn scale() -> (TypeScale, CellMetrics) {
    let mut text = TextEngine::embedded();
    let metrics = text
        .metrics(CellMetrics::DEFAULT_PX)
        .expect("the embedded mono face yields a grid");
    (TypeScale::new(&metrics), metrics)
}

#[test]
fn data_is_the_mono_face_at_the_cell_advance() {
    let (scale, metrics) = scale();
    let resolved = scale.resolve(TextRole::Data, &Style::default());
    assert_eq!(resolved.font, metrics.font());
    assert_eq!(resolved.size, metrics.size());
}

#[test]
fn the_three_proportional_roles_take_their_own_tokens() {
    let (scale, _) = scale();
    let px = |role| scale.resolve(role, &Style::default()).size.to_f32();
    assert_eq!(px(TextRole::Label), TypeScale::LABEL_PX);
    assert_eq!(px(TextRole::Prose), TypeScale::PROSE_PX);
    assert_eq!(px(TextRole::Heading), TypeScale::HEADING_PX);
    for role in [TextRole::Label, TextRole::Prose, TextRole::Heading] {
        assert_ne!(
            scale.resolve(role, &Style::default()).font,
            EmbeddedFonts::REGULAR,
            "{role:?} must not be set in the mono face"
        );
    }
}

/// A heading is weight 600 without being asked: `ui/lg` carries the
/// weight, so a heading pen that never sets `BOLD` still reads as one.
#[test]
fn a_heading_is_semibold_by_its_token_and_not_by_its_pen() {
    let (scale, _) = scale();
    let plain = scale.resolve(TextRole::Heading, &Style::default());
    assert_eq!(plain.font, EmbeddedFonts::UI_SEMIBOLD);
    assert_eq!(
        scale
            .resolve(TextRole::Label, &Style::default().attrs(Attrs::BOLD))
            .font,
        EmbeddedFonts::UI_SEMIBOLD,
        "a bold label reaches the same one weight break"
    );
}

/// Both text families provide an italic face, matching the terminal treatment.
#[test]
fn italic_leans_in_both_voices() {
    let (scale, _) = scale();
    let italic = Style::default().attrs(Attrs::ITALIC);
    assert_eq!(
        scale.resolve(TextRole::Data, &italic).font,
        EmbeddedFonts::MONO_ITALIC
    );
    assert_eq!(
        scale.resolve(TextRole::Prose, &italic).font,
        EmbeddedFonts::UI_ITALIC
    );
    // Neither family ships a bold italic; the lean wins, because a
    // synthesized bold is the trick this path exists to stop doing.
    assert_eq!(
        scale
            .resolve(
                TextRole::Prose,
                &Style::default().attrs(Attrs::ITALIC | Attrs::BOLD)
            )
            .font,
        EmbeddedFonts::UI_ITALIC
    );
}

/// The scale is anchored to the grid, so a bigger cell grows the
/// proportional type by exactly the same factor as the mono type.
#[test]
fn the_scale_follows_the_cell_so_hidpi_grows_both_voices_together() {
    let mut text = TextEngine::embedded();
    let big = text
        .metrics(CellMetrics::DEFAULT_PX * 2.0)
        .expect("the embedded mono face yields a grid at 2x");
    let scale = TypeScale::new(&big);
    assert_eq!(
        scale
            .resolve(TextRole::Prose, &Style::default())
            .size
            .to_f32(),
        TypeScale::PROSE_PX * 2.0
    );
}

/// The resolved set carries the voice's engraving: a mono role shapes
/// under `zero` + `tnum` whatever its attributes, a sans role under
/// nothing — so a measured width and a painted glyph come from one
/// shaped run.
#[test]
fn the_resolved_set_carries_the_voices_engraving() {
    let (scale, _) = scale();
    let engraved = qs_text::Features {
        tabular_figures: true,
        slashed_zero: true,
    };
    for style in [
        Style::default(),
        Style::default().attrs(Attrs::BOLD),
        Style::default().attrs(Attrs::ITALIC),
    ] {
        assert_eq!(scale.resolve(TextRole::Data, &style).features, engraved);
        assert_eq!(scale.resolve(TextRole::Value, &style).features, engraved);
        assert_eq!(
            scale.resolve(TextRole::Prose, &style).features,
            qs_text::Features::default()
        );
    }
}
