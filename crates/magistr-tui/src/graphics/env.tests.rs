use super::*;

fn from_env(vars: &[(&str, &str)]) -> Env {
    Env::from_lookup(|name| {
        vars.iter()
            .find(|(key, _)| *key == name)
            .map(|(_, value)| (*value).to_owned())
    })
}

#[test]
fn text_mode_is_forced_only_by_the_word_text() {
    assert!(Env::new("xterm-kitty", false, Some("text")).forces_text());
    assert!(Env::new("xterm-kitty", false, Some(" TEXT ")).forces_text());
    assert!(!Env::new("xterm-kitty", false, Some("graphics")).forces_text());
    assert!(!Env::new("xterm-kitty", false, Some("")).forces_text());
    assert!(!Env::new("xterm-kitty", false, None).forces_text());
}

#[test]
fn passthrough_follows_tmux() {
    let inside = Env::new("tmux-256color", true, None);
    assert!(inside.tmux());
    assert_eq!(inside.passthrough(), Passthrough::Tmux);
    assert_eq!(inside.term(), "tmux-256color");
    let outside = Env::new("xterm-ghostty", false, None);
    assert_eq!(outside.passthrough(), Passthrough::Direct);
}

#[test]
fn an_empty_environment_has_no_terminal_capabilities() {
    assert_eq!(from_env(&[]), Env::new("", false, None));
}

#[test]
fn terminal_settings_are_read_from_the_supplied_environment() {
    let env = from_env(&[
        ("TERM", "xterm-256color"),
        ("TERM_PROGRAM", "Ghostty"),
        ("MAGISTR_TUI_MODE", "text"),
        ("MAGISTR_TUI_SCREEN_READER", "1"),
    ]);
    assert_eq!(env.term(), "xterm-256color");
    assert!(!env.tmux());
    assert!(env.forces_text());
    assert!(env.screen_reader());
    assert!(env.styled_underlines());
    assert!(env.progress, "TERM_PROGRAM enables progress support");
    assert!(
        !env.shows_progress(),
        "text sessions suppress progress bytes"
    );
}

#[test]
fn tmux_is_detected_from_its_variable_or_term_prefix() {
    for vars in [
        vec![("TERM", "xterm-kitty"), ("TMUX", "/tmp/tmux-fixture")],
        vec![("TERM", "tmux-256color")],
        vec![("TERM", "screen-256color")],
    ] {
        assert!(from_env(&vars).tmux(), "{vars:?}");
    }
    assert!(!from_env(&[("TERM", "xterm-kitty"), ("TMUX", "")]).tmux());
}

#[test]
fn progress_terminal_conventions_are_read_independently() {
    for vars in [
        vec![("TERM_PROGRAM", "WezTerm")],
        vec![("WT_SESSION", "fixture")],
        vec![("ConEmuANSI", "oN")],
    ] {
        assert!(from_env(&vars).shows_progress(), "{vars:?}");
    }
    assert!(!from_env(&[("WT_SESSION", ""), ("ConEmuANSI", "off")]).shows_progress());
}

#[test]
fn the_progress_allowlist_names_five_terminals_and_nothing_else() {
    for program in ["ghostty", "Ghostty", "WezTerm", "wezterm", "iTerm.app"] {
        assert!(
            Env::honours_progress(program, false, false),
            "{program} shows a progress bar"
        );
    }
    for program in ["Apple_Terminal", "vscode", "tmux", ""] {
        assert!(
            !Env::honours_progress(program, false, false),
            "{program} would print the sequence"
        );
    }
    assert!(Env::honours_progress("", true, false), "Windows Terminal");
    assert!(Env::honours_progress("", false, true), "ConEmu");
}

#[test]
fn a_described_environment_reports_no_progress_until_it_opts_in() {
    let quiet = Env::new("xterm-kitty", false, None);
    assert!(
        !quiet.shows_progress(),
        "every golden and byte-exact capture stays where it was"
    );
    assert!(quiet.with_progress(true).shows_progress());
}

#[test]
fn tmux_and_forced_text_both_silence_a_terminal_that_would_show_progress() {
    assert!(
        !Env::new("xterm-ghostty", true, None)
            .with_progress(true)
            .shows_progress(),
        "tmux does not forward an OSC it does not know"
    );
    assert!(
        !Env::new("xterm-ghostty", false, Some("text"))
            .with_progress(true)
            .shows_progress(),
        "a plain-text cockpit is a quiet one"
    );
}

#[test]
fn the_underline_allowlist_names_the_terminals_that_draw_four_colon_x() {
    for term in [
        "xterm-kitty",
        "xterm-ghostty",
        "wezterm",
        "foot",
        "foot-extra",
        "alacritty",
    ] {
        assert!(
            Env::new(term, false, None).styled_underlines(),
            "{term} draws a curly underline"
        );
    }
    for term in ["xterm-256color", "screen", "vt100", "dumb", ""] {
        assert!(
            !Env::new(term, false, None).styled_underlines(),
            "{term} would print the sub-parameter"
        );
    }
}

#[test]
fn a_term_program_vouches_for_a_terminal_its_term_does_not() {
    assert!(Env::honours_underline_styles("xterm-256color", "Ghostty"));
    assert!(Env::honours_underline_styles("xterm-256color", "WezTerm"));
    assert!(!Env::honours_underline_styles("xterm-256color", "vscode"));
}

#[test]
fn tmux_gets_the_plain_rule() {
    assert!(
        !Env::new("xterm-kitty", true, None).styled_underlines(),
        "what tmux does with a sub-parameter it does not know depends on its version"
    );
}

#[test]
fn a_screen_reader_session_is_opt_in_and_reads_one_variable() {
    let quiet = Env::new("xterm-kitty", false, None);
    assert!(!quiet.screen_reader(), "the cell cockpit is the default");
    assert!(quiet.with_screen_reader(true).screen_reader());
    assert_eq!(Mode::SCREEN_READER_ENV, "MAGISTR_TUI_SCREEN_READER");
}

#[test]
fn a_variable_asks_unless_it_denies() {
    for value in ["1", "true", "yes", " on ", "please"] {
        assert!(Env::asks(value), "{value:?} asks");
    }
    for value in ["", "  ", "0", "false", "No", "OFF"] {
        assert!(!Env::asks(value), "{value:?} does not ask");
    }
}

#[test]
fn a_flat_session_is_a_quiet_one() {
    assert!(
        !Env::new("xterm-ghostty", false, None)
            .with_progress(true)
            .with_screen_reader(true)
            .shows_progress(),
        "a user reading the session as text asked for no progress bytes either"
    );
}
