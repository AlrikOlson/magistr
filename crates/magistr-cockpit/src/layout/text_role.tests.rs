use super::*;

#[test]
fn a_leaf_is_data_until_it_says_otherwise() {
    assert_eq!(TextRole::default(), TextRole::Data);
}

/// Every voice is enumerated, so a variant added without a row here is
/// a failing test rather than a role no sweep ever visits.
#[test]
fn every_voice_is_in_the_vocabulary() {
    for role in [
        TextRole::Data,
        TextRole::Badge,
        TextRole::Cap,
        TextRole::Fine,
        TextRole::Small,
        TextRole::Code,
        TextRole::Value,
        TextRole::Label,
        TextRole::Steer,
        TextRole::Prose,
        TextRole::Heading,
    ] {
        assert!(TextRole::ALL.contains(&role), "{role:?} is not in ALL");
    }
    assert_eq!(TextRole::ALL.len(), 11);
}

/// The mono half of the vocabulary is the small scale plus the cell's
/// own data voice; everything else is proportional.
#[test]
fn the_small_scale_is_mono_and_the_sans_voices_are_not() {
    for role in [
        TextRole::Data,
        TextRole::Badge,
        TextRole::Cap,
        TextRole::Fine,
        TextRole::Small,
        TextRole::Code,
        TextRole::Value,
    ] {
        assert!(role.is_mono(), "{role:?} is set in the mono face");
    }
    for role in [
        TextRole::Label,
        TextRole::Steer,
        TextRole::Prose,
        TextRole::Heading,
    ] {
        assert!(!role.is_mono(), "{role:?} is proportional");
    }
}

/// The two questions `is_mono` used to answer at once are now apart:
/// six voices are set in the mono face and *shaped at their own size*,
/// and only [`TextRole::Data`] is counted in columns off the cell.
#[test]
fn only_data_is_counted_in_columns() {
    assert!(TextRole::Data.is_counted());
    for role in TextRole::ALL {
        if role == TextRole::Data {
            continue;
        }
        assert!(!role.is_counted(), "{role:?} is shaped, never counted");
    }
}
