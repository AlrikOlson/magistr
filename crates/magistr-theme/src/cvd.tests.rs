use super::*;
use crate::mapping::ThemeMapping;

#[test]
fn parse_cvd_modes() {
    assert_eq!(
        CvdMode::from_name("deuteranopia"),
        Some(CvdMode::Deuteranopia)
    );
    assert_eq!(CvdMode::from_name("Protanopia"), Some(CvdMode::Protanopia));
    assert_eq!(CvdMode::from_name("TRITANOPIA"), Some(CvdMode::Tritanopia));
    assert_eq!(CvdMode::from_name("unknown"), None);
}

#[test]
fn cvd_mode_roundtrip() {
    for mode in [
        CvdMode::Deuteranopia,
        CvdMode::Protanopia,
        CvdMode::Tritanopia,
    ] {
        assert_eq!(CvdMode::from_name(mode.as_str()), Some(mode));
    }
}

#[test]
fn cvd_cycle() {
    assert_eq!(CvdMode::Deuteranopia.next(), CvdMode::Protanopia);
    assert_eq!(CvdMode::Protanopia.next(), CvdMode::Tritanopia);
    assert_eq!(CvdMode::Tritanopia.next(), CvdMode::Deuteranopia);
}

#[test]
fn default_mapping_is_cvd_safe() {
    assert!(is_cvd_safe(&ThemeMapping::default()));
}

#[test]
fn deuteranopia_mapping_is_cvd_safe() {
    let m = CvdMode::Deuteranopia.apply_to_mapping(&ThemeMapping::default());
    assert!(is_cvd_safe(&m));
}

#[test]
fn protanopia_mapping_is_cvd_safe() {
    let m = CvdMode::Protanopia.apply_to_mapping(&ThemeMapping::default());
    assert!(is_cvd_safe(&m));
}

#[test]
fn tritanopia_mapping_is_cvd_safe() {
    let m = CvdMode::Tritanopia.apply_to_mapping(&ThemeMapping::default());
    assert!(is_cvd_safe(&m));
}

#[test]
fn signal_icons_exist_for_signal_roles() {
    assert!(signal_icon(SemanticRole::Error).is_some());
    assert!(signal_icon(SemanticRole::Warning).is_some());
    assert!(signal_icon(SemanticRole::Success).is_some());
    assert!(signal_icon(SemanticRole::Info).is_some());
}

#[test]
fn signal_icons_none_for_non_signal_roles() {
    assert!(signal_icon(SemanticRole::Primary).is_none());
    assert!(signal_icon(SemanticRole::Muted).is_none());
}

#[test]
fn deuteranopia_remaps_error_and_success() {
    let base = ThemeMapping::default();
    let remapped = CvdMode::Deuteranopia.apply_to_mapping(&base);
    assert_eq!(remapped.error, PaletteField::Peach);
    assert_eq!(remapped.success, PaletteField::Blue);
    assert_eq!(remapped.info, PaletteField::Lavender);
    // Warning unchanged
    assert_eq!(remapped.warning, base.warning);
}

#[test]
fn tritanopia_remaps_warning_and_info() {
    let base = ThemeMapping::default();
    let remapped = CvdMode::Tritanopia.apply_to_mapping(&base);
    assert_eq!(remapped.warning, PaletteField::Peach);
    assert_eq!(remapped.info, PaletteField::Mauve);
    // Error and success unchanged
    assert_eq!(remapped.error, base.error);
    assert_eq!(remapped.success, base.success);
}
