use super::*;

#[test]
fn parse_names_both_modes() {
    assert_eq!(IconMode::parse("nerd"), Some(IconMode::Nerd));
    assert_eq!(IconMode::parse("NERD-Font"), Some(IconMode::Nerd));
    assert_eq!(IconMode::parse(" unicode "), Some(IconMode::Unicode));
    assert_eq!(IconMode::parse("plain"), Some(IconMode::Unicode));
    assert_eq!(IconMode::parse("emoji"), None);
}

#[test]
fn the_env_var_wins_over_the_terminal() {
    assert_eq!(
        IconMode::detect(Some("unicode"), Some("WezTerm")),
        IconMode::Unicode
    );
    assert_eq!(
        IconMode::detect(Some("nerd"), Some("Apple_Terminal")),
        IconMode::Nerd
    );
}

#[test]
fn a_terminal_that_bundles_the_symbols_gets_them() {
    assert_eq!(IconMode::detect(None, Some("WezTerm")), IconMode::Nerd);
    assert_eq!(IconMode::detect(None, Some("ghostty")), IconMode::Nerd);
}

#[test]
fn anything_else_falls_back_to_unicode() {
    assert_eq!(IconMode::detect(None, None), IconMode::Unicode);
    assert_eq!(
        IconMode::detect(None, Some("Apple_Terminal")),
        IconMode::Unicode
    );
    assert_eq!(IconMode::detect(Some("shrug"), None), IconMode::Unicode);
}

#[test]
fn each_mode_serves_its_set() {
    assert_eq!(IconMode::Nerd.icons(), &Icons::NERD);
    assert_eq!(IconMode::Unicode.icons(), &Icons::UNICODE);
}

#[test]
fn the_default_mode_needs_no_patched_font() {
    assert_eq!(IconMode::default(), IconMode::Unicode);
}
