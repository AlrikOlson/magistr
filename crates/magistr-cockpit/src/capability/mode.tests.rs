use super::*;

#[test]
fn graphics_has_pixels_and_no_banner_whatever_the_pointer_says() {
    for pointer_shapes in [true, false] {
        let mode = Mode::Graphics { pointer_shapes };
        assert!(mode.has_pixels());
        assert_eq!(mode.has_pointer_shapes(), pointer_shapes);
        assert_eq!(mode.banner(), None);
    }
}

#[test]
fn plain_text_has_neither_pixels_nor_pointer_shapes_and_every_reason_has_a_banner() {
    for reason in [
        TextReason::Forced,
        TextReason::ScreenReader,
        TextReason::TmuxNoPassthrough,
        TextReason::NoGraphics,
    ] {
        let mode = Mode::MinimalText(reason);
        assert!(!mode.has_pixels());
        assert!(!mode.has_pointer_shapes());
        let banner = mode.banner().expect("a plain-text session explains itself");
        assert!(!banner.is_empty());
    }
}

#[test]
fn the_banners_name_what_to_change() {
    assert!(
        Mode::MinimalText(TextReason::Forced)
            .banner()
            .is_some_and(|b| b.contains("MAGISTR_TUI_MODE"))
    );
    assert!(
        Mode::MinimalText(TextReason::TmuxNoPassthrough)
            .banner()
            .is_some_and(|b| b.contains("allow-passthrough"))
    );
    assert!(
        Mode::MinimalText(TextReason::ScreenReader)
            .banner()
            .is_some_and(|b| b.starts_with("Screen-reader mode"))
    );
}

#[test]
fn the_screen_reader_variable_is_spelled_once() {
    assert_eq!(Mode::SCREEN_READER_ENV, "MAGISTR_TUI_SCREEN_READER");
}
