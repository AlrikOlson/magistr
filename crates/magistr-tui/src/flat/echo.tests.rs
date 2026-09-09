use super::*;

fn sync(echo: &mut Echo, prompt: &Prompt) -> Vec<u8> {
    let mut bytes = Vec::new();
    echo.sync(prompt, 80, &mut bytes);
    bytes
}

#[test]
fn typed_characters_and_combining_marks_are_written_only_once() {
    let mut echo = Echo::default();
    let mut prompt = Prompt::new();
    sync(&mut echo, &prompt);
    for ch in ["c", "a", "f", "e"] {
        prompt.insert(ch);
        assert_eq!(sync(&mut echo, &prompt), ch.as_bytes());
    }
    prompt.insert("\u{301}");
    let accent = sync(&mut echo, &prompt);
    assert!(accent.starts_with("\u{301}".as_bytes()));
    assert!(
        !accent.contains(&b'e'),
        "the combining base is never replayed"
    );
    prompt.backspace();
    assert_eq!(prompt.text(), "caf");
    assert_eq!(sync(&mut echo, &prompt), b"\x08\x1b[K");
    assert!(sync(&mut echo, &prompt).is_empty());
}

#[test]
fn a_leading_combining_mark_can_be_echoed_and_deleted_without_changing_the_label() {
    let mut echo = Echo::default();
    let mut prompt = Prompt::new();
    sync(&mut echo, &prompt);
    prompt.insert("\u{301}");
    assert!(sync(&mut echo, &prompt).starts_with("◌\u{301}".as_bytes()));
    prompt.backspace();
    assert_eq!(sync(&mut echo, &prompt), b"\x08\x1b[K");
}

#[test]
fn a_wide_or_joined_grapheme_is_one_delete_without_stale_cells() {
    for grapheme in ["界", "👨‍👩‍👧‍👦", "👍🏽"] {
        let mut echo = Echo::default();
        let mut prompt = Prompt::new();
        let mut terminal = vt100::Parser::new(24, 80, 0);
        prompt.insert("a");
        terminal.process(&sync(&mut echo, &prompt));
        prompt.insert(grapheme);
        let bytes = sync(&mut echo, &prompt);
        assert!(bytes.starts_with(grapheme.as_bytes()));
        terminal.process(&bytes);
        prompt.backspace();
        assert_eq!(prompt.text(), "a");
        terminal.process(&sync(&mut echo, &prompt));
        assert_eq!(terminal.screen().contents(), "input: a");
        assert_eq!(terminal.screen().cursor_position(), (0, 8));
    }
}

#[test]
fn midline_edit_and_shorter_replacement_clear_the_old_suffix() {
    let mut echo = Echo::default();
    let mut prompt = Prompt::new();
    let mut terminal = vt100::Parser::new(24, 80, 0);
    prompt.insert("long draft");
    terminal.process(&sync(&mut echo, &prompt));
    prompt.home();
    terminal.process(&sync(&mut echo, &prompt));
    prompt.delete();
    prompt.insert("s");
    terminal.process(&sync(&mut echo, &prompt));
    assert_eq!(terminal.screen().contents(), "input: song draft");
    assert_eq!(terminal.screen().cursor_position(), (0, 8));
    prompt.kill_to_end();
    terminal.process(&sync(&mut echo, &prompt));
    assert_eq!(terminal.screen().contents().trim_end(), "input: s");
    assert_eq!(terminal.screen().cursor_position(), (0, 8));
}

#[test]
fn a_family_is_followed_and_deleted_at_its_logical_columns() {
    let family = "👨‍👩‍👧‍👦";
    let mut echo = Echo::default();
    let mut prompt = Prompt::new();
    let mut terminal = vt100::Parser::new(24, 80, 0);
    terminal.process(&sync(&mut echo, &prompt));
    prompt.insert(family);
    let bytes = sync(&mut echo, &prompt);
    assert!(
        bytes
            .windows(family.len())
            .any(|part| part == family.as_bytes())
    );
    terminal.process(&bytes);
    assert_eq!(terminal.screen().cursor_position(), (0, 9));
    prompt.insert("after");
    terminal.process(&sync(&mut echo, &prompt));
    assert_eq!(terminal.screen().cursor_position(), (0, 14));
    assert_eq!(
        terminal
            .screen()
            .cell(0, 9)
            .expect("suffix cell")
            .contents(),
        "a"
    );
    prompt.home();
    terminal.process(&sync(&mut echo, &prompt));
    prompt.delete();
    terminal.process(&sync(&mut echo, &prompt));
    assert_eq!(terminal.screen().contents(), "input: after");
    assert_eq!(terminal.screen().cursor_position(), (0, 7));
}

#[test]
fn a_pasted_family_reanchors_before_its_suffix() {
    let mut echo = Echo::default();
    let mut prompt = Prompt::new();
    let mut terminal = vt100::Parser::new(24, 80, 0);
    prompt.insert("👨‍👩‍👧‍👦x");
    terminal.process(&sync(&mut echo, &prompt));
    assert_eq!(
        terminal
            .screen()
            .cell(0, 9)
            .expect("suffix cell")
            .contents(),
        "x"
    );
    assert_eq!(terminal.screen().cursor_position(), (0, 10));
    prompt.kill_to_start();
    terminal.process(&sync(&mut echo, &prompt));
    assert_eq!(terminal.screen().contents().trim_end(), "input:");
}

#[test]
fn disputed_graphemes_never_expand_past_the_live_row() {
    for columns in 1..25 {
        for text in ["👨‍👩‍👧‍👦", "ab👨‍👩‍👧‍👦x", "👍🏽x", "🇸🇪x", "❤️x"]
        {
            let mut echo = Echo::default();
            let mut prompt = Prompt::new();
            let mut terminal = vt100::Parser::new(5, columns, 0);
            prompt.insert(text);
            for _ in 0..text.chars().count() + 2 {
                let mut bytes = Vec::new();
                echo.sync(&prompt, columns, &mut bytes);
                terminal.process(&bytes);
                assert_eq!(
                    terminal.screen().cursor_position().0,
                    0,
                    "{columns}: {text:?}"
                );
                assert!(!terminal.screen().row_wrapped(0), "{columns}: {text:?}");
                prompt.left();
            }
            prompt.kill_to_end();
            let mut bytes = Vec::new();
            echo.sync(&prompt, columns, &mut bytes);
            terminal.process(&bytes);
            assert!(
                terminal.screen().contents().trim_end().is_empty()
                    || terminal.screen().contents().trim_end() == "input:"
            );
        }
    }
}

#[test]
fn multiline_current_line_has_context_and_controls_cannot_escape() {
    let mut prompt = Prompt::new();
    prompt.insert("first\n\u{1b}[2J\t👨‍👩‍👧‍👦e\u{301}\u{9b}");
    let (text, cursor) = line(&prompt, 79);
    assert_eq!(text, "input 2/2: ^[[2J\\t👨‍👩‍👧‍👦e\u{301}\\u{9b}");
    assert_eq!(cursor, width(&text));
    assert!(!text.chars().any(char::is_control));
    prompt.up();
    assert_eq!(line(&prompt, 79).0, "input 1/2: first");
}

#[test]
fn a_narrow_projection_never_wraps_even_on_a_wide_cursor_cell() {
    let mut prompt = Prompt::new();
    prompt.insert("界👨‍👩‍👧‍👦abcdefgh");
    for room in 0..20 {
        for _ in 0..12 {
            let (text, cursor) = line(&prompt, room);
            assert!(width(&text) <= room, "{room}: {text}");
            assert!(cursor <= room);
            prompt.left();
        }
        prompt.end();
    }
}

#[test]
fn resize_and_transcript_boundaries_never_move_up_or_reprint_settled_text() {
    let mut echo = Echo::default();
    let mut prompt = Prompt::new();
    prompt.insert("draft");
    let first = sync(&mut echo, &prompt);
    assert!(first.ends_with(b"input: draft"));
    let mut bytes = Vec::new();
    echo.finish(&mut bytes);
    bytes.extend_from_slice(b"agent: settled\r\n");
    echo.sync(&prompt, 80, &mut bytes);
    assert_eq!(bytes, b"\r\nagent: settled\r\n\r\x1b[Kinput: draft");
    bytes.clear();
    echo.sync(&prompt, 10, &mut bytes);
    assert!(bytes.starts_with(b"\r\n\r\x1b[K"));
    assert!(!bytes.windows(7).any(|w| w == b"settled"));
    assert!(!bytes.contains(&b'A'), "no cursor-up");
    bytes.clear();
    echo.finish(&mut bytes);
    assert_eq!(bytes, b"\r\n");
    bytes.clear();
    echo.finish(&mut bytes);
    assert!(bytes.is_empty());
}
