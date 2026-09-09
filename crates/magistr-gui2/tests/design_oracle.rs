//! Validate typography roles against the generated JSON token table.
//!
//! The extractor reads `design/almagest-seventh-edition.html` and generates Rust and
//! JSON tables. The crate includes the Rust table; these tests independently read JSON
//! to verify that each [`TextRole`] selects the intended [`Voice`] and resolves its
//! properties correctly.

use std::collections::BTreeMap;

use magistr_cockpit::layout::TextRole;
use magistr_gui2::{TypeScale, Voice};
use serde_json::Value;

/// The generated table, read as data.
fn table() -> Value {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/design/almagest7.tokens.json");
    let raw = std::fs::read_to_string(path).expect("the oracle's token table is tracked");
    serde_json::from_str(&raw).expect("the token table is JSON")
}

fn voices(table: &Value) -> BTreeMap<String, Value> {
    table["voices"]
        .as_object()
        .expect("the table carries voices")
        .iter()
        .map(|(name, value)| (name.clone(), value.clone()))
        .collect()
}

fn number(voice: &Value, field: &str) -> f32 {
    #[allow(clippy::cast_possible_truncation)]
    let value = voice[field]
        .as_f64()
        .unwrap_or_else(|| panic!("a voice with no {field}")) as f32;
    value
}

/// Every role resolves to the voice it names, field for field — the
/// crate's one hand-made decision about the table, checked against the
/// table itself.
#[test]
fn every_role_wears_the_voice_it_names() {
    let table = table();
    let voices = voices(&table);
    for role in TextRole::ALL {
        let Some(token) = Voice::token(role) else {
            // The grid's own voice: sized by the cell, so the volume
            // has no number for it and there is nothing to compare.
            assert_eq!(role, TextRole::Data);
            continue;
        };
        let declared = voices.get(token).unwrap_or_else(|| {
            panic!("{role:?} names {token}, which the volume no longer carries")
        });
        let voice = Voice::of(role);
        assert_eq!(voice.family, declared["family"], "{role:?} family");
        assert_eq!(voice.px, number(declared, "px"), "{role:?} size");
        assert_eq!(
            f32::from(voice.weight),
            number(declared, "weight"),
            "{role:?} weight"
        );
        assert_eq!(
            voice.tracking_em,
            number(declared, "tracking_em"),
            "{role:?} tracking"
        );
        assert_eq!(
            voice.line_height,
            number(declared, "line_height"),
            "{role:?} line height"
        );
        assert_eq!(
            voice.upper,
            declared["transform"] == "uppercase",
            "{role:?} transform"
        );
    }
}

/// The whole table is read, so a regenerated table that loses or mangles a voice fails
/// loudly — including the one voice no role wears yet.
#[test]
fn every_voice_the_oracle_derives_is_sane() {
    let table = table();
    let voices = voices(&table);
    // The table includes fourteen text voices, four control voices, two surface voices,
    // and one display voice.
    assert_eq!(voices.len(), 21, "the volume declares twenty-one voices");
    for (name, voice) in &voices {
        let family = voice["family"].as_str().expect("a family");
        assert!(
            family == "mono" || family == "sans" || family == "serif",
            "{name}: {family}"
        );
        let px = number(voice, "px");
        let scale = if family == "serif" {
            20.0..=24.0
        } else {
            9.0..=15.0
        };
        assert!(
            scale.contains(&px),
            "{name}: {px}px is outside the volume's scale"
        );
        let weight = number(voice, "weight");
        assert!((400.0..=600.0).contains(&weight), "{name}: weight {weight}");
        let tracking = number(voice, "tracking_em");
        assert!(
            (-0.01..=0.125).contains(&tracking),
            "{name}: tracking {tracking}"
        );
        let line = number(voice, "line_height");
        assert!((0.0..=1.75).contains(&line), "{name}: line height {line}");
        assert!(
            voice["transform"] != "uppercase" || tracking > 0.0,
            "{name}: untracked caps"
        );
    }
    let unworn: Vec<&String> = voices
        .keys()
        .filter(|name| {
            !TextRole::ALL
                .iter()
                .filter_map(|role| Voice::token(*role))
                .any(|token| token == name.as_str())
        })
        .collect();
    // Components select these voices directly, without a cockpit `TextRole`:
    // breadcrumbs, meter values, controls, receipts, status text, and the serif chapter
    // title.
    assert_eq!(
        unworn,
        vec![
            "button",
            "chapter_title",
            "crumb",
            "hint",
            "kbd",
            "meter_value",
            "phase",
            "receipt_key",
            "slot",
            "stat",
            "theme_label"
        ],
        "a voice gaining or losing a role is a decision, not a drift"
    );
}

/// : the serif display face appears only at ≥20 px on brand moments — report titles,
/// the project cover — and nowhere else. Read off the table: exactly one voice is
/// serif, it is the chapter title, it is set at 24 px, and it is the voice
/// [`Voice::DISPLAY`] wears. Every other voice is mono or sans, so the only way to set
/// serif on the window is through that one voice.
#[test]
fn the_serif_appears_on_exactly_one_voice_and_only_at_display_size() {
    let table = table();
    let voices = voices(&table);
    let serif: Vec<(&String, &Value)> = voices
        .iter()
        .filter(|(_, voice)| voice["family"] == "serif")
        .collect();
    assert_eq!(serif.len(), 1, "one serif voice: {serif:?}");
    let (name, declared) = serif[0];
    assert_eq!(name, "chapter_title");
    assert!(
        number(declared, "px") >= 20.0,
        "the serif is a display face"
    );
    assert_eq!(Voice::DISPLAY.family, "serif");
    assert_eq!(Voice::DISPLAY.px, number(declared, "px"));
    assert_eq!(Voice::DISPLAY.tracking_em, number(declared, "tracking_em"));
    assert!(Voice::DISPLAY.is_serif());
    for role in TextRole::ALL {
        assert!(!Voice::of(role).is_serif(), "{role:?} is never serif");
    }
}

/// The geometry the crate consumes is the volume's own, and so is the
/// window leading every unleaded voice is set on.
#[test]
fn the_geometry_and_the_leading_are_the_volumes_own() {
    let table = table();
    let geometry = &table["geometry"];
    assert_eq!(number(geometry, "rail_width"), 248.0);
    assert_eq!(number(geometry, "card_radius"), 11.0);
    assert_eq!(number(geometry, "turn_max_width"), 600.0);
    assert_eq!(number(geometry, "leading"), Voice::LEADING);
    let spacing = &table["spacing"];
    assert_eq!(
        number(spacing, "meter_track_height"),
        Voice::METER_TRACK_HEIGHT
    );
    assert_eq!(number(spacing, "meter_tick_width"), Voice::METER_TICK_WIDTH);
}

/// The type-scale constants the rest of the crate still names are the
/// voices they are aliases for — kept as names because a call site
/// reading `PROSE_PX` says what it means.
#[test]
fn the_type_scale_constants_are_their_own_voices() {
    assert_eq!(TypeScale::PROSE_PX, Voice::of(TextRole::Prose).px);
    assert_eq!(TypeScale::HEADING_PX, Voice::of(TextRole::Heading).px);
    assert_eq!(
        TypeScale::HEADING_WEIGHT.0,
        Voice::of(TextRole::Heading).weight
    );
    assert_eq!(TypeScale::LABEL_PX, Voice::of(TextRole::Label).px);
}
