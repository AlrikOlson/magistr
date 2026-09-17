use super::*;

/// placement law, as a property of the role set rather than as a check at a call site:
/// a control that also carries a word *is* an icon inside a text-verb button, and no
/// role is one.
#[test]
fn no_role_is_both_a_control_and_labelled() {
    let offenders: Vec<&str> = IconRole::ALL
        .into_iter()
        .filter(|role| role.is_control() && role.takes_label())
        .map(IconRole::name)
        .collect();
    assert!(
        offenders.is_empty(),
        "Labelled buttons must not contain icons: {offenders:?}"
    );
}

/// The law's other half: an icon-only button is permitted, and only for
/// utilities.
#[test]
fn the_only_icon_only_control_is_the_utility() {
    let controls: Vec<IconRole> = IconRole::ALL
        .into_iter()
        .filter(|role| role.is_control())
        .collect();
    assert_eq!(controls, vec![IconRole::Utility]);
    assert!(!IconRole::Utility.takes_label());
}

/// The three places a mark may appear without being a control are the three the design
/// names, and there is no fourth: an `Action` role is the one this enum exists to
/// withhold.
#[test]
fn the_law_names_kinds_navigation_and_empty_states_and_nothing_else() {
    let places: Vec<&str> = IconRole::ALL
        .into_iter()
        .filter(|role| !role.is_control())
        .map(IconRole::name)
        .collect();
    assert_eq!(places, vec!["kind", "navigation", "empty state"]);
    assert_eq!(
        IconRole::ALL.len(),
        4,
        "a fifth role is a change to the law"
    );
}
