use super::*;
use crate::mapping::ThemeMapping;
use crate::palette::catppuccin_mocha;

#[test]
fn css_contains_palette_colors() {
    let css = palette_to_css(&catppuccin_mocha(), &ThemeMapping::default());
    assert!(css.contains("--magistr-base: #1e1e2e;"));
    assert!(css.contains("--magistr-blue: #89b4fa;"));
    assert!(css.contains("--magistr-text: #cdd6f4;"));
    assert!(css.contains("--magistr-red: #f38ba8;"));
}

#[test]
fn css_contains_semantic_tokens() {
    let css = palette_to_css(&catppuccin_mocha(), &ThemeMapping::default());
    // Default mapping: primary = blue
    assert!(css.contains("--magistr-primary: #89b4fa;"));
    // Default mapping: error = red
    assert!(css.contains("--magistr-error: #f38ba8;"));
    // Default mapping: success = green
    assert!(css.contains("--magistr-success: #a6e3a1;"));
}

#[test]
fn css_root_wraps_in_selector() {
    let css = palette_to_css_root(&catppuccin_mocha(), &ThemeMapping::default());
    assert!(css.starts_with(":root {"));
    assert!(css.ends_with("}\n"));
}

#[test]
fn css_all_palette_fields_present() {
    let css = palette_to_css(&catppuccin_mocha(), &ThemeMapping::default());
    for field in PaletteField::ALL {
        let prop = format!("--magistr-{}:", field.as_str());
        assert!(
            css.contains(&prop),
            "missing CSS property for {}",
            field.as_str()
        );
    }
}

#[test]
fn css_all_semantic_roles_present() {
    let css = palette_to_css(&catppuccin_mocha(), &ThemeMapping::default());
    for role in SemanticRole::ALL {
        let prop = format!("--magistr-{}:", role.as_str());
        assert!(
            css.contains(&prop),
            "missing CSS property for {}",
            role.as_str()
        );
    }
}

#[test]
fn owned_palette_css_matches() {
    let palette = catppuccin_mocha();
    let owned = OwnedPalette::from_ref(&palette);
    let mapping = ThemeMapping::default();
    let css1 = palette_to_css(&palette, &mapping);
    let css2 = owned_palette_to_css(&owned, &mapping);
    assert_eq!(css1, css2);
}

#[test]
fn css_with_custom_mapping() {
    use crate::mapping::PaletteField;
    let mapping = ThemeMapping {
        primary: PaletteField::Mauve,
        ..ThemeMapping::default()
    };
    let css = palette_to_css(&catppuccin_mocha(), &mapping);
    // Primary should now resolve to mauve's color
    assert!(css.contains("--magistr-primary: #cba6f7;"));
}

/// Pull one custom property's colour back out of a generated block.
fn prop(css: &str, name: &str) -> ThemeColor {
    let needle = format!("--magistr-{name}: ");
    let line = css
        .lines()
        .find_map(|l| l.trim().strip_prefix(&needle))
        .unwrap_or_else(|| panic!("no --magistr-{name} in the emitted CSS"));
    let hex = line.trim_end_matches(';');
    ThemeColor::from_hex(hex).unwrap_or_else(|| panic!("--magistr-{name} is not a hex colour"))
}

#[test]
fn latte_text_slots_clear_the_floor_in_the_emitted_css() {
    // The five pairs Palette::validate reports on Catppuccin Latte. The
    // palette still carries them; what the GUI is handed does not.
    use crate::math::contrast_ratio;
    use crate::palette::catppuccin_latte;

    let latte = catppuccin_latte();
    let css = palette_to_css(&latte, &ThemeMapping::default());
    let base = prop(&css, "base");
    assert_eq!(base, latte.base, "the ground was moved");

    for (name, floor) in [
        ("subtext0", 4.5),
        ("blue", 4.5),
        ("green", 4.5),
        ("yellow", 4.5),
        ("overlay0", 3.0),
    ] {
        let raw = contrast_ratio(
            crate::mapping::PaletteField::from_name(name)
                .expect("known field")
                .resolve(&latte),
            latte.base,
        );
        assert!(raw < floor, "--magistr-{name} no longer fails raw");
        let got = contrast_ratio(prop(&css, name), base);
        assert!(
            got >= floor,
            "--magistr-{name} is {got:.2} on base, needs {floor}"
        );
    }
}

#[test]
fn latte_semantic_tokens_clear_the_floor_in_the_emitted_css() {
    // The semantic family resolves through the same contract, so a token
    // can never disagree with the slot it points at.
    use crate::math::contrast_ratio;
    use crate::palette::catppuccin_latte;

    let latte = catppuccin_latte();
    let css = palette_to_css(&latte, &ThemeMapping::default());
    let base = prop(&css, "base");
    for (role, floor) in [
        ("primary", 4.5),
        ("secondary", 4.5),
        ("on_surface", 4.5),
        ("error", 4.5),
        ("warning", 4.5),
        ("success", 4.5),
        ("info", 3.0),
        ("muted", 3.0),
        ("accent", 3.0),
    ] {
        let got = contrast_ratio(prop(&css, role), base);
        assert!(
            got >= floor,
            "--magistr-{role} is {got:.2} on base, needs {floor}"
        );
    }
}

#[test]
fn a_ground_token_is_the_palette_s_own_colour() {
    // `surface` and `highlight` name backgrounds: they are emitted as the
    // palette names them, whatever the light palette's contrast is.
    use crate::palette::catppuccin_latte;

    let latte = catppuccin_latte();
    let css = palette_to_css(&latte, &ThemeMapping::default());
    assert_eq!(prop(&css, "surface"), latte.surface0);
    assert_eq!(prop(&css, "highlight"), latte.surface2);
    for (name, want) in [
        ("base", latte.base),
        ("surface0", latte.surface0),
        ("surface1", latte.surface1),
        ("surface2", latte.surface2),
    ] {
        assert_eq!(prop(&css, name), want, "--magistr-{name} was moved");
    }
}
