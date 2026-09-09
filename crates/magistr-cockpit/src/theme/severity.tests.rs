use super::*;

/// Worst first, and `Ord` says the same thing — so a deal can sort on
/// the band itself rather than on a table a second call site keeps.
#[test]
fn the_bands_are_ordered_worst_first() {
    let mut sorted = Severity::ALL;
    sorted.sort_unstable();
    assert_eq!(sorted, Severity::ALL);
    assert!(Severity::Alert < Severity::Attention);
    assert!(Severity::Good < Severity::Quiet);
}

/// The three signal bands resolve through the mapping; the quiet one
/// is not a signal and has no role to remap.
#[test]
fn only_the_quiet_band_has_no_signal_role() {
    let roles: Vec<Option<SemanticRole>> = Severity::ALL.into_iter().map(Severity::role).collect();
    assert_eq!(
        roles,
        vec![
            Some(SemanticRole::Error),
            Some(SemanticRole::Warning),
            Some(SemanticRole::Success),
            None,
        ]
    );
}

/// Every band names itself, and no two share a name.
#[test]
fn every_band_has_its_own_label() {
    let mut labels: Vec<&str> = Severity::ALL.into_iter().map(Severity::label).collect();
    labels.sort_unstable();
    labels.dedup();
    assert_eq!(labels.len(), Severity::ALL.len());
}

/// Every band draws its mark from the vocabulary in force, and no two
/// bands wear the same one — the cap is half the treatment, so two
/// bands sharing a glyph would be two bands sharing a reading.
#[test]
fn every_band_wears_its_own_mark_from_the_set_in_force() {
    use crate::theme::Icons;
    for icons in [&Icons::UNICODE, &Icons::NERD] {
        let mut marks: Vec<&str> = Severity::ALL
            .into_iter()
            .map(|band| band.mark(icons))
            .collect();
        marks.sort_unstable();
        marks.dedup();
        assert_eq!(marks.len(), Severity::ALL.len());
    }
}
