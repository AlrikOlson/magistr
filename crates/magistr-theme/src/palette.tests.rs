use super::*;
use crate::color::ThemeColor;

#[test]
fn catppuccin_mocha_name() {
    assert_eq!(catppuccin_mocha().name, "catppuccin-mocha");
}

#[test]
fn all_builtin_count() {
    assert_eq!(all_builtin_palettes().len(), 13);
}

#[test]
fn all_builtin_unique_names() {
    let palettes = all_builtin_palettes();
    let mut names: Vec<&str> = palettes.iter().map(|p| p.name).collect();
    names.sort();
    names.dedup();
    assert_eq!(names.len(), palettes.len(), "duplicate palette names");
}

#[test]
fn owned_from_ref_roundtrip() {
    let p = catppuccin_mocha();
    let owned = OwnedPalette::from_ref(&p);
    assert_eq!(owned.name, "catppuccin-mocha");
    assert_eq!(owned.base, p.base);
    assert_eq!(owned.text, p.text);
    assert_eq!(owned.blue, p.blue);
}

#[test]
fn toml_palette_parse() {
    let toml = TomlPalette {
        name: "test".to_string(),
        base: "#1e1e2e".to_string(),
        surface0: "#313244".to_string(),
        surface1: "#45475a".to_string(),
        surface2: "#585b70".to_string(),
        overlay0: "#6c7086".to_string(),
        overlay1: "#7f849c".to_string(),
        text: "#cdd6f4".to_string(),
        subtext0: "#a6adc8".to_string(),
        subtext1: "#bac2de".to_string(),
        blue: "#89b4fa".to_string(),
        sky: "#89dceb".to_string(),
        green: "#a6e3a1".to_string(),
        red: "#f38ba8".to_string(),
        yellow: "#f9e2af".to_string(),
        peach: "#fab387".to_string(),
        mauve: "#cba6f7".to_string(),
        lavender: "#b4befe".to_string(),
    };
    let p = toml.into_palette().unwrap();
    assert_eq!(p.name, "test");
    assert_eq!(p.base, ThemeColor::new(30, 30, 46));
}

#[test]
fn toml_palette_invalid_hex() {
    let toml = TomlPalette {
        name: "bad".to_string(),
        base: "nothex".to_string(),
        surface0: "#000000".to_string(),
        surface1: "#000000".to_string(),
        surface2: "#000000".to_string(),
        overlay0: "#000000".to_string(),
        overlay1: "#000000".to_string(),
        text: "#000000".to_string(),
        subtext0: "#000000".to_string(),
        subtext1: "#000000".to_string(),
        blue: "#000000".to_string(),
        sky: "#000000".to_string(),
        green: "#000000".to_string(),
        red: "#000000".to_string(),
        yellow: "#000000".to_string(),
        peach: "#000000".to_string(),
        mauve: "#000000".to_string(),
        lavender: "#000000".to_string(),
    };
    assert!(toml.into_palette().is_none());
}

#[test]
fn minimal_palette_derives_missing() {
    let minimal = MinimalTomlPalette {
        name: "minimal-test".to_string(),
        base: "#1e1e2e".to_string(),
        text: "#cdd6f4".to_string(),
        blue: "#89b4fa".to_string(),
        green: "#a6e3a1".to_string(),
        red: "#f38ba8".to_string(),
        yellow: "#f9e2af".to_string(),
        mauve: "#cba6f7".to_string(),
        surface0: None,
        surface1: None,
        surface2: None,
        overlay0: None,
        overlay1: None,
        subtext0: None,
        subtext1: None,
        sky: None,
        peach: None,
        lavender: None,
    };
    let p = minimal.into_palette().unwrap();
    assert_eq!(p.name, "minimal-test");
    // Derived colors should exist and be non-zero
    assert_ne!(p.surface0, ThemeColor::new(0, 0, 0));
    assert_ne!(p.sky, ThemeColor::new(0, 0, 0));
    assert_ne!(p.peach, ThemeColor::new(0, 0, 0));
}

#[test]
fn owned_palette_to_toml_roundtrip() {
    let p = catppuccin_mocha();
    let toml_str = p.to_toml_string();
    assert!(toml_str.contains("catppuccin-mocha"));
    assert!(toml_str.contains("#1e1e2e")); // base color
}

#[test]
fn apply_overrides_changes_name_and_colors() {
    let mut owned = OwnedPalette::from_ref(&catppuccin_mocha());
    let overrides = MinimalTomlPalette {
        name: "custom-override".to_string(),
        base: "#000000".to_string(),
        text: "#ffffff".to_string(),
        blue: "#0000ff".to_string(),
        green: "#00ff00".to_string(),
        red: "#ff0000".to_string(),
        yellow: "#ffff00".to_string(),
        mauve: "#ff00ff".to_string(),
        surface0: None,
        surface1: None,
        surface2: None,
        overlay0: None,
        overlay1: None,
        subtext0: None,
        subtext1: None,
        sky: None,
        peach: None,
        lavender: None,
    };
    owned.apply_overrides(&overrides);
    assert_eq!(owned.name, "custom-override");
    assert_eq!(owned.base, ThemeColor::new(0, 0, 0));
    assert_eq!(owned.text, ThemeColor::new(255, 255, 255));
    // Unspecified optionals should keep original values
    assert_eq!(owned.surface0, ThemeColor::new(49, 50, 68));
}

#[test]
fn validate_mocha_passes_wcag_aa() {
    let p = catppuccin_mocha();
    let violations = p.validate();
    assert!(
        violations.is_empty(),
        "catppuccin-mocha WCAG AA violations: {violations:?}"
    );
}

#[test]
fn validate_reports_latte_accent_failures() {
    let violations = catppuccin_latte().accent_violations();
    let pairs: Vec<&str> = violations.iter().map(|v| v.pair.as_str()).collect();
    // The two the cockpit's Pen was designed around.
    assert!(
        pairs.contains(&"accent lavender on surface0"),
        "latte accent report: {pairs:?}"
    );
    assert!(
        pairs.contains(&"accent peach on base"),
        "latte accent report: {pairs:?}"
    );
    assert!(
        violations.iter().all(|v| v.required == 3.0),
        "accent pairs are the large-text floor"
    );
}

#[test]
fn accent_violations_are_part_of_validate() {
    let latte = catppuccin_latte();
    let all: Vec<String> = latte.validate().into_iter().map(|v| v.pair).collect();
    for violation in latte.accent_violations() {
        assert!(all.contains(&violation.pair), "{} missing", violation.pair);
    }
}

#[test]
fn clean_palettes_are_the_known_ones() {
    // validate() is a report, not a promise: an accent is a colour
    // identity, and most palettes put at least one of theirs below the
    // 3:1 floor on some surface. This pins which built-ins are wholly
    // clean so a palette edit cannot quietly darken one of them, and so
    // a new palette has to declare which side of the line it lands on.
    let clean: Vec<&str> = all_builtin_palettes()
        .iter()
        .filter(|p| p.validate().is_empty())
        .map(|p| p.name)
        .collect();
    assert_eq!(clean, KNOWN_CLEAN, "clean built-in palettes changed");
}

#[test]
fn cvd_remapped_signals_are_covered_by_the_accent_report() {
    use crate::cvd::CvdMode;
    use crate::mapping::{SemanticRole, ThemeMapping};

    let modes = [
        CvdMode::Deuteranopia,
        CvdMode::Protanopia,
        CvdMode::Tritanopia,
    ];
    let signals = [
        SemanticRole::Error,
        SemanticRole::Warning,
        SemanticRole::Success,
        SemanticRole::Info,
    ];
    for palette in all_builtin_palettes() {
        let reported: Vec<String> = palette
            .accent_violations()
            .into_iter()
            .map(|v| v.pair)
            .collect();
        for mode in modes {
            let mapping = mode.apply_to_mapping(&ThemeMapping::default());
            for role in signals {
                let field = mapping.field_for(role);
                let color = field.resolve(&palette);
                for (ground, bg) in [("base", palette.base), ("surface0", palette.surface0)] {
                    if crate::math::contrast_ratio(color, bg) < 3.0 {
                        let pair = format!("accent {} on {ground}", field.as_str());
                        assert!(
                            reported.contains(&pair),
                            "{} {mode:?} {role:?}: {pair} unreported",
                            palette.name
                        );
                    }
                }
            }
        }
    }
}

/// The built-in palettes whose every measured pair — text, UI and
/// accent — clears its WCAG AA floor. The rest report violations by
/// design; a surface that needs one of their accents legible runs it
/// through `math::ensure_contrast`.
const KNOWN_CLEAN: &[&str] = &["catppuccin-mocha", "cvd-optimized"];
