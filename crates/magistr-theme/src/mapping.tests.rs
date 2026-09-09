use super::*;
use crate::palette::catppuccin_mocha;

#[test]
fn default_mapping_primary_is_blue() {
    let m = ThemeMapping::default();
    assert_eq!(m.primary, PaletteField::Blue);
}

#[test]
fn resolve_returns_correct_color() {
    let m = ThemeMapping::default();
    let p = catppuccin_mocha();
    let color = m.resolve(SemanticRole::Primary, &p);
    assert_eq!(color, p.blue);
}

#[test]
fn resolve_error_returns_red() {
    let m = ThemeMapping::default();
    let p = catppuccin_mocha();
    assert_eq!(m.resolve(SemanticRole::Error, &p), p.red);
}

#[test]
fn resolve_success_returns_green() {
    let m = ThemeMapping::default();
    let p = catppuccin_mocha();
    assert_eq!(m.resolve(SemanticRole::Success, &p), p.green);
}

#[test]
fn palette_field_roundtrip() {
    for field in PaletteField::ALL {
        let name = field.as_str();
        let parsed = PaletteField::from_name(name).unwrap();
        assert_eq!(parsed, field, "roundtrip failed for {name}");
    }
}

#[test]
fn palette_field_case_insensitive() {
    assert_eq!(PaletteField::from_name("Blue"), Some(PaletteField::Blue));
    assert_eq!(
        PaletteField::from_name("SURFACE0"),
        Some(PaletteField::Surface0)
    );
}

#[test]
fn palette_field_unknown() {
    assert!(PaletteField::from_name("notacolor").is_none());
}

#[test]
fn semantic_role_roundtrip() {
    for role in SemanticRole::ALL {
        let name = role.as_str();
        let parsed = SemanticRole::from_name(name).unwrap();
        assert_eq!(parsed, role, "roundtrip failed for {name}");
    }
}

#[test]
fn semantic_role_on_surface_variants() {
    assert_eq!(
        SemanticRole::from_name("on_surface"),
        Some(SemanticRole::OnSurface)
    );
    assert_eq!(
        SemanticRole::from_name("onsurface"),
        Some(SemanticRole::OnSurface)
    );
}

#[test]
fn toml_mapping_override() {
    let toml = TomlThemeMapping {
        primary: Some("mauve".to_string()),
        accent: Some("peach".to_string()),
        ..Default::default()
    };
    let m = toml.into_mapping();
    assert_eq!(m.primary, PaletteField::Mauve);
    assert_eq!(m.accent, PaletteField::Peach);
    // Non-overridden fields keep defaults
    assert_eq!(m.error, PaletteField::Red);
}

#[test]
fn toml_mapping_is_empty() {
    assert!(TomlThemeMapping::default().is_empty());
    assert!(
        !TomlThemeMapping {
            primary: Some("blue".to_string()),
            ..Default::default()
        }
        .is_empty()
    );
}

#[test]
fn all_palette_fields_count() {
    assert_eq!(PaletteField::ALL.len(), 17);
}

#[test]
fn all_semantic_roles_count() {
    assert_eq!(SemanticRole::ALL.len(), 11);
}
