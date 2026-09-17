use super::*;

/// The chunk's own scale, spelled out: a table read off the volume is
/// only worth having if the numbers it hands back are the volume's.
#[test]
fn the_small_scale_is_the_volumes_own() {
    let px = |role| Voice::of(role).px;
    assert_eq!(px(TextRole::Badge), 9.0);
    assert_eq!(px(TextRole::Cap), 9.5);
    assert_eq!(px(TextRole::Fine), 10.0);
    assert_eq!(px(TextRole::Small), 10.5);
    assert_eq!(px(TextRole::Code), 10.5);
    assert_eq!(px(TextRole::Value), 11.0);
    assert_eq!(px(TextRole::Label), 12.0);
    assert_eq!(px(TextRole::Steer), 12.5);
    assert_eq!(px(TextRole::Prose), 13.0);
    assert_eq!(px(TextRole::Heading), 15.0);
}

/// A voice's family agrees with the role's own answer, so the face a
/// run is set in cannot depend on which of the two you asked.
#[test]
fn the_family_and_the_role_agree_about_the_face() {
    for role in TextRole::ALL {
        assert_eq!(
            Voice::of(role).is_mono(),
            role.is_mono(),
            "{role:?}: the table and the role disagree about the face"
        );
    }
}

/// [`Code`](TextRole::Code) differs from [`Small`](TextRole::Small) in
/// exactly one thing — the leading — which is the whole reason it is
/// its own voice rather than a second name for the same one.
#[test]
fn the_code_voice_is_the_small_one_at_the_code_leading() {
    let (small, code) = (Voice::of(TextRole::Small), Voice::of(TextRole::Code));
    assert_eq!(small.px, code.px);
    assert_eq!(small.family, code.family);
    assert_eq!(small.leading(), Voice::LEADING);
    assert_eq!(code.leading(), 1.75);
    assert_ne!(small.leading(), code.leading());
}

/// A voice with no declared line-height is set on the page's own
/// leading — the volume's `body` rule, not a constant beside it.
#[test]
fn an_undeclared_leading_is_the_pages_own() {
    assert_eq!(Voice::LEADING, 1.5);
    assert_eq!(Voice::of(TextRole::Prose).leading(), 1.65);
    for role in [TextRole::Cap, TextRole::Value, TextRole::Heading] {
        assert_eq!(Voice::of(role).leading(), Voice::LEADING, "{role:?}");
    }
}

/// Only the cap is uppercased, and only the two tracked voices track —
/// the volume's own rule that caps are never set untracked.
#[test]
fn only_the_cap_is_uppercased_and_untracked_voices_never_shout() {
    for role in TextRole::ALL {
        let voice = Voice::of(role);
        assert_eq!(voice.upper, role == TextRole::Cap, "{role:?}");
        assert!(!voice.upper || voice.tracking_em > 0.0, "{role:?}");
    }
    assert_eq!(Voice::of(TextRole::Cap).tracking_em, 0.12);
    assert_eq!(Voice::of(TextRole::Badge).tracking_em, 0.08);
}

/// The transform is applied where the volume applies it and nowhere
/// else, and an already-uppercase run borrows rather than allocating.
#[test]
fn the_transform_uppercases_only_a_cap() {
    let cap = Voice::of(TextRole::Cap);
    assert_eq!(cap.transform("Checkpoints"), "CHECKPOINTS");
    assert!(matches!(cap.transform("BUDGET"), Cow::Borrowed("BUDGET")));
    let prose = Voice::of(TextRole::Prose);
    assert!(matches!(
        prose.transform("Checkpoints"),
        Cow::Borrowed("Checkpoints")
    ));
}

/// The grid's voice is the one role with no token — it is sized by the
/// cell, so there is no number in the volume to read for it.
#[test]
fn the_data_voice_has_no_token_and_no_size() {
    assert_eq!(Voice::token(TextRole::Data), None);
    assert_eq!(Voice::of(TextRole::Data).px, 0.0);
    for role in TextRole::ALL {
        if role == TextRole::Data {
            continue;
        }
        assert!(Voice::token(role).is_some(), "{role:?} names no token");
        assert!(Voice::of(role).px > 0.0, "{role:?} has no size");
    }
}

/// Data is never set in the UI face, and the data voice is always
/// engraved: every mono voice — by role and by name — shapes with the
/// slashed zero and tabular figures, and every proportional voice is
/// set plain. The two answers come from the family, so a voice cannot
/// be mono and unengraved, or sans and engraved.
#[test]
fn the_data_voice_is_engraved_and_the_ui_voice_is_plain() {
    let engraved = Features {
        tabular_figures: true,
        slashed_zero: true,
    };
    for role in TextRole::ALL {
        let voice = Voice::of(role);
        let expected = if role.is_mono() {
            engraved
        } else {
            Features::default()
        };
        assert_eq!(voice.features(), expected, "{role:?}");
    }
    for voice in [
        Voice::CAP,
        Voice::CHIP,
        Voice::CRUMB,
        Voice::DIFFLINE,
        Voice::KBD,
        Voice::LEDGER_META,
        Voice::METER_VALUE,
        Voice::PHASE,
        Voice::RECEIPT_KEY,
        Voice::STAT,
        Voice::VALUE,
        Voice::BADGE,
        Voice::DATA,
    ] {
        assert!(voice.is_mono());
        assert_eq!(voice.features(), engraved);
    }
    for voice in [
        Voice::BUTTON,
        Voice::HINT,
        Voice::LEDGER_NAME,
        Voice::SLOT,
        Voice::STEER,
        Voice::THEME_LABEL,
    ] {
        assert!(!voice.is_mono());
        assert_eq!(voice.features(), Features::default());
    }
}
