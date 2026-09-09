use super::*;

fn strip(chunks: &[&[u8]]) -> Vec<String> {
    let mut stripper = EscapeStripper::default();
    chunks.iter().map(|c| stripper.feed(c)).collect()
}

#[test]
fn colours_and_cursor_moves_go_and_the_text_stays() {
    assert_eq!(
        strip(&[b"\x1b[31mred\x1b[0m \x1b[1;32mbold\x1b[m\x1b[2K\x1b[1A\n"]),
        vec!["red bold\n"]
    );
}

#[test]
fn a_sequence_split_across_chunks_vanishes_whole() {
    assert_eq!(
        strip(&[b"a\x1b[3", b"1mb\x1b", b"[0mc"]),
        vec!["a", "b", "c"]
    );
}

#[test]
fn strings_end_at_bel_or_st() {
    assert_eq!(
        strip(&[b"\x1b]0;title\x07x\x1b]2;t\x1b\\y\x1bPq#0\x1b\\z"]),
        vec!["xyz"]
    );
}

#[test]
fn a_string_split_across_chunks_vanishes_whole() {
    assert_eq!(
        strip(&[b"\x1b]0;ti", b"tle\x1b", b"\\ok"]),
        vec!["", "", "ok"]
    );
}

#[test]
fn carriage_returns_become_line_breaks_once() {
    assert_eq!(strip(&[b"10%\r20%\r\n30%\n"]), vec!["10%\n20%\n30%\n"]);
    assert_eq!(strip(&[b"a\r", b"\nb"]), vec!["a\n", "b"]);
}

#[test]
fn two_and_three_byte_escapes_go() {
    assert_eq!(strip(&[b"\x1b(Bok\x1b=x\x1b#8y\x1bMz"]), vec!["okxyz"]);
}

#[test]
fn other_controls_go_and_tabs_stay() {
    assert_eq!(strip(&[b"\x08\x07a\tb\x7fc\x00"]), vec!["a\tbc"]);
}

#[test]
fn utf8_split_across_chunks_is_held_for_the_next() {
    let e = "é".as_bytes();
    assert_eq!(
        strip(&[&e[..1], &e[1..], "ü!".as_bytes()]),
        vec!["", "é", "ü!"]
    );
}

#[test]
fn invalid_utf8_is_replaced() {
    assert_eq!(strip(&[b"a\xffb"]), vec!["a\u{FFFD}b"]);
}
