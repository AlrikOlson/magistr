use super::*;
use crate::agent::PermissionKind;

#[test]
fn every_footer_context_is_a_subset_of_the_registry() {
    for context in [
        Binding::HINT_COMPLETION,
        Binding::HINT_PERMISSION,
        Binding::HINT_PROPOSAL,
        Binding::HINT_BLOCKER,
        Binding::HINT_VIEWER,
        Binding::HINT_PAUSED,
        Binding::HINT_LOOP_STREAMING,
        Binding::HINT_LOOP_IDLE,
        Binding::HINT_STREAMING,
        Binding::HINT_IDLE,
        Binding::HINT_HOME,
        Binding::HINT_SHEET,
    ] {
        for binding in context {
            assert!(
                Binding::ALL.contains(binding),
                "footer hint '{} {}' is not in Binding::ALL — the sheet would not show it",
                binding.keys,
                binding.does
            );
        }
    }
}

#[test]
fn hint_joins_keys_and_does_with_middots() {
    assert_eq!(
        Binding::hint(Binding::HINT_PERMISSION),
        "A allow · \u{21e7}A always · D deny"
    );
    assert_eq!(Binding::hint(Binding::HINT_BLOCKER), "c clear");
}

#[test]
fn the_permission_rows_match_the_permission_kinds() {
    // The dock's buttons derive their hints from PermissionKind; the
    // sheet's needs-you rows must not drift from them.
    for (kind, binding) in [
        (PermissionKind::AllowOnce, Binding::ALLOW),
        (PermissionKind::AllowAlways, Binding::ALWAYS),
        (PermissionKind::RejectOnce, Binding::DENY),
        (PermissionKind::RejectAlways, Binding::NEVER),
    ] {
        // The sheet prints what the card's key cap prints, so a
        // reader never learns a letter the button does not wear.
        assert_eq!(binding.keys, kind.key_cap());
    }
}

#[test]
fn launcher_accelerators_are_distinct() {
    let keys = [
        Binding::NEXT.keys,
        Binding::LOOP.keys,
        Binding::GATES.keys,
        Binding::ROADMAP.keys,
    ];
    let mut unique = keys.to_vec();
    unique.sort_unstable();
    unique.dedup();
    assert_eq!(unique.len(), keys.len());
}

#[test]
fn no_key_hint_literal_outside_the_registry() {
    // The acceptance: the sheet and the footer are generated from this
    // registry, never hand-written. Every `keys does` atom the registry
    // can render must appear as a literal ONLY here — a copy in view.rs
    // or update.rs would drift the moment a key is rebound.
    let sources = [
        ("view.rs", include_str!("view.rs")),
        ("update.rs", include_str!("update.rs")),
        ("model.rs", include_str!("model.rs")),
        ("dock.rs", include_str!("dock.rs")),
        ("rail.rs", include_str!("rail.rs")),
    ];
    for binding in Binding::ALL {
        let atom = format!("{} {}", binding.keys, binding.does);
        // A hint literal opens a string or follows a middot separator —
        // prose that happens to contain the words does not.
        for shape in [format!("\"{atom}"), format!("· {atom}")] {
            for (name, source) in sources {
                assert!(
                    !source.contains(&shape),
                    "{name} carries the hint literal {shape:?} — render it from Binding instead"
                );
            }
        }
    }
}

#[test]
fn the_sheet_lists_the_tech_trees_zoom_under_its_own_group() {
    // the acceptance is that the sheet carries them — which
    // it does by carrying them here, since it is generated from ALL.
    for binding in [Binding::ZOOM, Binding::ZOOM_FIT] {
        assert!(Binding::ALL.contains(&binding));
        assert_eq!(binding.group, "roadmap");
    }
}

/// The window's abort chord is the registry's but never the sheet's:
/// a terminal never reports `SUPER`, so its row would be a lie there,
/// while the terminal's own `esc` abort row stays.
#[test]
fn the_abort_chord_is_registered_off_the_sheet() {
    assert!(!Binding::ALL.contains(&Binding::ABORT_CHORD));
    assert!(Binding::ALL.contains(&Binding::ABORT));
    assert_eq!(Binding::ABORT_CHORD.mods, Modifiers::SUPER);
    assert_eq!(Binding::ABORT_CHORD.does, Binding::ABORT.does);
    assert_eq!(Binding::ABORT_CHORD.group, "loop");
}

/// The dock's chords are registered but
/// never the sheet's — a terminal never reports `SUPER` — while the
/// terminal's own `ctrl-t` row is; the chords share the key the fold
/// reads, so the cap and the fold cannot drift.
#[test]
fn the_dock_chords_are_registered_off_the_sheet() {
    assert!(Binding::ALL.contains(&Binding::DOCK));
    assert!(!Binding::ALL.contains(&Binding::DOCK_CHORD));
    assert!(!Binding::ALL.contains(&Binding::DOCK_MAX_CHORD));
    assert_eq!(Binding::DOCK.mods, Modifiers::CTRL);
    assert_eq!(Binding::DOCK_CHORD.code, KeyCode::Enter);
    assert_eq!(Binding::DOCK_CHORD.mods, Modifiers::SUPER);
    assert_eq!(Binding::DOCK_MAX_CHORD.code, KeyCode::Enter);
    assert_eq!(
        Binding::DOCK_MAX_CHORD.mods,
        Modifiers::SUPER | Modifiers::SHIFT
    );
    assert_eq!(Binding::DOCK_CHORD.does, Binding::DOCK.does);
    assert_eq!(Binding::DOCK.group, "stage");
}
