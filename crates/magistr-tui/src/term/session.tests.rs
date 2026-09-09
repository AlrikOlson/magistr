use super::*;

#[test]
fn enter_is_pinned() {
    let mut out = Vec::new();
    Session::cockpit().enter(&mut out);
    assert_eq!(
        out,
        b"\x1b[?1049h\x1b[?25l\x1b[?1000h\x1b[?1002h\x1b[?1003h\x1b[?1006h\x1b[?2004h\x1b[?1004h\x1b[>3u"
    );
}

#[test]
fn leave_is_the_reverse_after_a_pen_reset() {
    let mut out = Vec::new();
    Session::cockpit().leave(&mut out);
    assert_eq!(
        out,
        b"\x1b[0m\x1b[<u\x1b[?1004l\x1b[?2004l\x1b[?1006l\x1b[?1003l\x1b[?1002l\x1b[?1000l\x1b[?25h\x1b[?1049l"
    );
}

#[test]
fn leave_pops_the_keyboard_before_the_mouse_and_the_screen_last() {
    let mut out = Vec::new();
    Session::cockpit().leave(&mut out);
    let text = String::from_utf8(out).expect("ascii");
    let kkp = text.find("\x1b[<u").expect("pop");
    let mouse = text.find("?1000l").expect("mouse off");
    let screen = text.find("?1049l").expect("alt screen off");
    assert!(kkp < mouse && mouse < screen);
    assert!(text.ends_with("\x1b[?1049l"));
}

#[test]
fn queries_ask_for_background_keyboard_flags_and_pixel_sizes() {
    let mut out = Vec::new();
    Session::cockpit().queries(&mut out);
    assert_eq!(out, b"\x1b]11;?\x1b\\\x1b[?u\x1b[16t\x1b[14t");
    assert_eq!(Session::cockpit().keyboard_flags(), 3);
}

#[test]
fn a_progress_terminal_has_its_bar_cleared_first_and_every_other_leave_is_unchanged() {
    let mut bare = Vec::new();
    Session::cockpit().leave(&mut bare);
    assert!(!Session::cockpit().shows_progress());
    let mut out = Vec::new();
    let session = Session::cockpit().with_progress(true);
    assert!(session.shows_progress());
    session.leave(&mut out);
    assert_eq!(
        out,
        [b"\x1b]9;4;0;0\x1b\\".as_slice(), &bare].concat(),
        "the bar goes out before the pen reset, and nothing else moves"
    );
}

#[test]
fn progress_never_shows_up_in_the_take_over_sequence() {
    let mut plain = Vec::new();
    Session::cockpit().enter(&mut plain);
    let mut reporting = Vec::new();
    Session::cockpit().with_progress(true).enter(&mut reporting);
    assert_eq!(plain, reporting);
}

#[test]
fn a_flat_session_never_takes_the_terminal_over() {
    let mut out = Vec::new();
    let session = Session::flat();
    assert!(session.is_flat());
    assert!(!Session::cockpit().is_flat());
    session.enter(&mut out);
    assert_eq!(
        out, b"\x1b[?2004h\x1b[>3u",
        "bracketed paste and the keyboard flags, and nothing else"
    );
}

#[test]
fn a_flat_leave_undoes_exactly_what_its_enter_set() {
    let mut out = Vec::new();
    Session::flat().leave(&mut out);
    assert_eq!(out, b"\x1b[0m\x1b[<u\x1b[?2004l");
}
