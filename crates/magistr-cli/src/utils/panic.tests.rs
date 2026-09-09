use super::*;

#[test]
fn restore_sequence_disables_every_screen_mode() {
    let seq = std::str::from_utf8(RESTORE_SEQUENCE).unwrap();
    // Mouse tracking (incl. SGR and URXVT), paste, focus.
    for mode in [1000, 1002, 1003, 1006, 1015, 2004, 1004, 1049] {
        assert!(
            seq.contains(&format!("\x1b[?{mode}l")),
            "must disable mode {mode}"
        );
    }
    assert!(
        seq.contains("\x1b[<u"),
        "must pop the kitty keyboard protocol"
    );
    assert!(seq.contains("\x1b[?25h"), "must re-show the cursor");
}

#[test]
fn restore_sequence_leaves_the_alt_screen_after_kkp_pop() {
    // KKP state is per-screen on some terminals: pop it while still on the
    // alt screen, then leave, so the main screen's KKP state is untouched.
    let seq = std::str::from_utf8(RESTORE_SEQUENCE).unwrap();
    let kkp = seq.find("\x1b[<u").unwrap();
    let leave = seq.find("\x1b[?1049l").unwrap();
    assert!(
        kkp < leave,
        "KKP pop must come before leaving the alt screen"
    );
}

#[test]
fn emergency_restore_terminal_does_not_panic() {
    // Verify the emergency restore helper can be called without panicking.
    // In test context, stdout may not be a terminal — that's fine, errors
    // are silently ignored (best-effort restore).
    emergency_restore_terminal();
}
