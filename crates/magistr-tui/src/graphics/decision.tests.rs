use super::*;
use crate::graphics::Passthrough;

fn reply(kitty: bool, sixel: bool) -> ProbeReply {
    ProbeReply {
        text_sizing: false,
        kitty,
        sixel,
        complete: true,
    }
}

#[test]
fn text_is_forced_over_everything() {
    let env = Env::new("xterm-kitty", false, Some("text"));
    assert_eq!(
        Decision::for_terminal(&env, &reply(true, true)),
        Decision::MinimalText(TextReason::Forced)
    );
}

#[test]
fn kitty_wins_and_carries_the_passthrough() {
    let direct = Env::new("xterm-ghostty", false, None);
    assert_eq!(
        Decision::for_terminal(&direct, &reply(true, true)),
        Decision::Graphics(Protocol::Kitty(Passthrough::Direct))
    );
    let tmux = Env::new("tmux-256color", true, None);
    assert_eq!(
        Decision::for_terminal(&tmux, &reply(true, false)),
        Decision::Graphics(Protocol::Kitty(Passthrough::Tmux))
    );
}

#[test]
fn tmux_without_a_reply_is_plain_text_even_with_sixel() {
    let tmux = Env::new("tmux-256color", true, None);
    assert_eq!(
        Decision::for_terminal(&tmux, &reply(false, true)),
        Decision::MinimalText(TextReason::TmuxNoPassthrough)
    );
    assert_eq!(
        Decision::for_terminal(&tmux, &ProbeReply::default()),
        Decision::MinimalText(TextReason::TmuxNoPassthrough)
    );
}

#[test]
fn sixel_outside_tmux_is_graphics() {
    let env = Env::new("foot", false, None);
    assert_eq!(
        Decision::for_terminal(&env, &reply(false, true)),
        Decision::Graphics(Protocol::Sixel)
    );
}

#[test]
fn no_reply_at_all_is_plain_text() {
    let env = Env::new("xterm-256color", false, None);
    assert_eq!(
        Decision::for_terminal(&env, &ProbeReply::default()),
        Decision::MinimalText(TextReason::NoGraphics)
    );
}

#[test]
fn only_kitty_translates_to_a_mode_with_pointer_shapes() {
    for passthrough in [Passthrough::Direct, Passthrough::Tmux] {
        assert_eq!(
            Decision::Graphics(Protocol::Kitty(passthrough)).mode(),
            Mode::Graphics {
                pointer_shapes: true
            }
        );
    }
    assert_eq!(
        Decision::Graphics(Protocol::Sixel).mode(),
        Mode::Graphics {
            pointer_shapes: false
        }
    );
    for reason in [
        TextReason::Forced,
        TextReason::ScreenReader,
        TextReason::TmuxNoPassthrough,
        TextReason::NoGraphics,
    ] {
        assert_eq!(
            Decision::MinimalText(reason).mode(),
            Mode::MinimalText(reason)
        );
    }
}

#[test]
fn banners_name_a_capable_terminal_and_the_tmux_option() {
    assert_eq!(Decision::Graphics(Protocol::Sixel).mode().banner(), None);
    for reason in [
        TextReason::Forced,
        TextReason::TmuxNoPassthrough,
        TextReason::NoGraphics,
    ] {
        let banner = Mode::MinimalText(reason).banner().expect("a banner");
        for terminal in ["ghostty", "kitty", "WezTerm", "iTerm2"] {
            assert!(banner.contains(terminal), "{reason:?}: {banner}");
        }
        assert!(!banner.contains('\n'));
    }
    let tmux = Mode::MinimalText(TextReason::TmuxNoPassthrough)
        .banner()
        .expect("banner");
    assert!(tmux.contains("allow-passthrough on"));
    let forced = Mode::MinimalText(TextReason::Forced)
        .banner()
        .expect("banner");
    assert!(forced.contains("MAGISTR_TUI_MODE"));
}

#[test]
fn a_screen_reader_session_is_linear_text_over_everything() {
    let env = Env::new("xterm-kitty", false, None).with_screen_reader(true);
    assert_eq!(
        Decision::for_terminal(&env, &reply(true, true)),
        Decision::MinimalText(TextReason::ScreenReader),
        "a kitty reply cannot put pixels on a flat cockpit"
    );
    let banner = Mode::MinimalText(TextReason::ScreenReader)
        .banner()
        .expect("a banner");
    assert!(banner.contains("Screen-reader mode"), "{banner}");
    assert!(
        !banner.contains("ghostty"),
        "naming a terminal would be advice a flat session cannot take"
    );
    assert!(!banner.contains('\n'));
}
