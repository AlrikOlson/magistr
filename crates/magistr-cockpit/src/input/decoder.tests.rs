use proptest::prelude::*;

use super::*;

fn decode(bytes: &[u8]) -> Vec<Event> {
    let mut decoder = Decoder::new();
    let mut out = Vec::new();
    decoder.feed(bytes, &mut out);
    decoder.flush(&mut out);
    out
}

fn key(code: KeyCode) -> Event {
    Event::Key(KeyEvent::new(code))
}

fn key_with(code: KeyCode, mods: Modifiers) -> Event {
    Event::Key(KeyEvent::new(code).with(mods))
}

// --- legacy keys ---------------------------------------------------------

#[test]
fn plain_characters() {
    assert_eq!(decode(b"a"), vec![key(KeyCode::Char('a'))]);
    assert_eq!(
        decode(b"A"),
        vec![key_with(KeyCode::Char('A'), Modifiers::SHIFT)]
    );
    assert_eq!(
        decode("é日".as_bytes()),
        vec![key(KeyCode::Char('é')), key(KeyCode::Char('日'))]
    );
    assert_eq!(decode(b" "), vec![key(KeyCode::Char(' '))]);
}

#[test]
fn control_characters() {
    assert_eq!(
        decode(b"\x01"),
        vec![key_with(KeyCode::Char('a'), Modifiers::CTRL)]
    );
    assert_eq!(
        decode(b"\x1a"),
        vec![key_with(KeyCode::Char('z'), Modifiers::CTRL)]
    );
    assert_eq!(
        decode(b"\x00"),
        vec![key_with(KeyCode::Char(' '), Modifiers::CTRL)]
    );
    assert_eq!(
        decode(b"\x1f"),
        vec![key_with(KeyCode::Char('_'), Modifiers::CTRL)]
    );
    assert_eq!(decode(b"\x7f"), vec![key(KeyCode::Backspace)]);
    assert_eq!(decode(b"\x08"), vec![key(KeyCode::Backspace)]);
    assert_eq!(decode(b"\r"), vec![key(KeyCode::Enter)]);
    assert_eq!(decode(b"\n"), vec![key(KeyCode::Enter)]);
    assert_eq!(decode(b"\t"), vec![key(KeyCode::Tab)]);
}

#[test]
fn alt_prefix() {
    assert_eq!(
        decode(b"\x1ba"),
        vec![key_with(KeyCode::Char('a'), Modifiers::ALT)]
    );
    assert_eq!(
        decode(b"\x1bA"),
        vec![key_with(
            KeyCode::Char('A'),
            Modifiers::SHIFT | Modifiers::ALT
        )]
    );
    assert_eq!(
        decode("\x1bé".as_bytes()),
        vec![key_with(KeyCode::Char('é'), Modifiers::ALT)]
    );
    assert_eq!(
        decode(b"\x1b\x01"),
        vec![key_with(
            KeyCode::Char('a'),
            Modifiers::CTRL | Modifiers::ALT
        )]
    );
    assert_eq!(decode(b"\x1b\x1b"), vec![key(KeyCode::Esc)]);
}

#[test]
fn csi_and_ss3_cursor_keys() {
    assert_eq!(decode(b"\x1b[A"), vec![key(KeyCode::Up)]);
    assert_eq!(decode(b"\x1b[B"), vec![key(KeyCode::Down)]);
    assert_eq!(decode(b"\x1b[C"), vec![key(KeyCode::Right)]);
    assert_eq!(decode(b"\x1b[D"), vec![key(KeyCode::Left)]);
    assert_eq!(decode(b"\x1b[H"), vec![key(KeyCode::Home)]);
    assert_eq!(decode(b"\x1b[F"), vec![key(KeyCode::End)]);
    assert_eq!(
        decode(b"\x1b[1;5A"),
        vec![key_with(KeyCode::Up, Modifiers::CTRL)]
    );
    assert_eq!(
        decode(b"\x1b[1;4D"),
        vec![key_with(KeyCode::Left, Modifiers::SHIFT | Modifiers::ALT)]
    );
    assert_eq!(decode(b"\x1bOA"), vec![key(KeyCode::Up)]);
    assert_eq!(decode(b"\x1bOP"), vec![key(KeyCode::F(1))]);
    assert_eq!(decode(b"\x1bOQ"), vec![key(KeyCode::F(2))]);
    assert_eq!(decode(b"\x1bOS"), vec![key(KeyCode::F(4))]);
    assert_eq!(
        decode(b"\x1b[1;2P"),
        vec![key_with(KeyCode::F(1), Modifiers::SHIFT)]
    );
}

#[test]
fn a_cursor_position_report_is_not_a_key() {
    assert_eq!(decode(b"\x1b[12;40R"), vec![]);
    assert_eq!(decode(b"\x1b[?62;4c"), vec![]);
}

#[test]
fn tilde_keys() {
    assert_eq!(decode(b"\x1b[2~"), vec![key(KeyCode::Insert)]);
    assert_eq!(decode(b"\x1b[3~"), vec![key(KeyCode::Delete)]);
    assert_eq!(decode(b"\x1b[5~"), vec![key(KeyCode::PageUp)]);
    assert_eq!(decode(b"\x1b[6~"), vec![key(KeyCode::PageDown)]);
    assert_eq!(decode(b"\x1b[7~"), vec![key(KeyCode::Home)]);
    assert_eq!(decode(b"\x1b[8~"), vec![key(KeyCode::End)]);
    assert_eq!(decode(b"\x1b[15~"), vec![key(KeyCode::F(5))]);
    assert_eq!(decode(b"\x1b[17~"), vec![key(KeyCode::F(6))]);
    assert_eq!(decode(b"\x1b[24~"), vec![key(KeyCode::F(12))]);
    assert_eq!(
        decode(b"\x1b[3;5~"),
        vec![key_with(KeyCode::Delete, Modifiers::CTRL)]
    );
    assert_eq!(decode(b"\x1b[99~"), vec![]);
}

// --- kitty keyboard protocol ----------------------------------------------

#[test]
fn csi_u_keys() {
    assert_eq!(decode(b"\x1b[97u"), vec![key(KeyCode::Char('a'))]);
    assert_eq!(
        decode(b"\x1b[97;2u"),
        vec![key_with(KeyCode::Char('A'), Modifiers::SHIFT)]
    );
    assert_eq!(
        decode(b"\x1b[49:33;2u"),
        vec![key_with(KeyCode::Char('!'), Modifiers::SHIFT)]
    );
    assert_eq!(
        decode(b"\x1b[13;5u"),
        vec![key_with(KeyCode::Enter, Modifiers::CTRL)]
    );
    assert_eq!(decode(b"\x1b[27u"), vec![key(KeyCode::Esc)]);
    assert_eq!(
        decode(b"\x1b[9;2u"),
        vec![key_with(KeyCode::Tab, Modifiers::SHIFT)]
    );
    assert_eq!(
        decode(b"\x1b[127;3u"),
        vec![key_with(KeyCode::Backspace, Modifiers::ALT)]
    );
    assert_eq!(decode(b"\x1b[57376u"), vec![key(KeyCode::F(13))]);
    assert_eq!(decode(b"\x1b[57398u"), vec![key(KeyCode::F(35))]);
    assert_eq!(decode(b"\x1b[57414u"), vec![key(KeyCode::Enter)]);
    assert_eq!(decode(b"\x1b[57400u"), vec![key(KeyCode::Char('1'))]);
    assert_eq!(decode(b"\x1b[57419u"), vec![key(KeyCode::Up)]);
    assert_eq!(
        decode(b"\x1b[97;9u"),
        vec![key_with(KeyCode::Char('a'), Modifiers::SUPER)]
    );
}

#[test]
fn csi_u_event_types() {
    assert_eq!(
        decode(b"\x1b[97;1:3u"),
        vec![Event::Key(
            KeyEvent::new(KeyCode::Char('a')).kind(KeyKind::Release)
        )]
    );
    assert_eq!(
        decode(b"\x1b[97;5:2u"),
        vec![Event::Key(
            KeyEvent::new(KeyCode::Char('a'))
                .with(Modifiers::CTRL)
                .kind(KeyKind::Repeat)
        )]
    );
    assert_eq!(
        decode(b"\x1b[1;1:3A"),
        vec![Event::Key(
            KeyEvent::new(KeyCode::Up).kind(KeyKind::Release)
        )]
    );
    assert!(!KeyEvent::new(KeyCode::Up).kind(KeyKind::Release).is_press());
}

#[test]
fn csi_u_lock_modifier_and_media_keys_are_dropped() {
    assert_eq!(decode(b"\x1b[57358u"), vec![]);
    assert_eq!(decode(b"\x1b[57441u"), vec![]);
    assert_eq!(decode(b"\x1b[57428u"), vec![]);
}

#[test]
fn window_and_cell_pixel_replies() {
    assert_eq!(
        decode(b"\x1b[6;20;10t"),
        vec![Event::Reply(Reply::CellPixels {
            width: 10,
            height: 20
        })]
    );
    assert_eq!(
        decode(b"\x1b[4;480;800t"),
        vec![Event::Reply(Reply::WindowPixels {
            width: 800,
            height: 480
        })]
    );
    assert_eq!(decode(b"\x1b[8;24;80t"), vec![]);
    assert_eq!(decode(b"\x1b[6;20t"), vec![]);
    assert_eq!(decode(b"\x1b[4;70000;800t"), vec![]);
}

#[test]
fn keyboard_flags_reply() {
    assert_eq!(
        decode(b"\x1b[?3u"),
        vec![Event::Reply(Reply::KeyboardFlags(3))]
    );
    assert_eq!(
        decode(b"\x1b[?0u"),
        vec![Event::Reply(Reply::KeyboardFlags(0))]
    );
}

// --- mouse ---------------------------------------------------------------

fn mouse(kind: MouseKind, col: u16, row: u16) -> Event {
    Event::Mouse(MouseEvent::new(kind, col, row))
}

#[test]
fn sgr_buttons() {
    assert_eq!(
        decode(b"\x1b[<0;10;5M"),
        vec![mouse(MouseKind::Down(Button::Left), 9, 4)]
    );
    assert_eq!(
        decode(b"\x1b[<0;10;5m"),
        vec![mouse(MouseKind::Up(Button::Left), 9, 4)]
    );
    assert_eq!(
        decode(b"\x1b[<1;1;1M"),
        vec![mouse(MouseKind::Down(Button::Middle), 0, 0)]
    );
    assert_eq!(
        decode(b"\x1b[<2;3;4m"),
        vec![mouse(MouseKind::Up(Button::Right), 2, 3)]
    );
    assert_eq!(decode(b"\x1b[<3;3;4M"), vec![]);
}

#[test]
fn sgr_motion_and_wheel() {
    assert_eq!(
        decode(b"\x1b[<32;10;5M"),
        vec![mouse(MouseKind::Drag(Button::Left), 9, 4)]
    );
    assert_eq!(
        decode(b"\x1b[<34;10;5M"),
        vec![mouse(MouseKind::Drag(Button::Right), 9, 4)]
    );
    assert_eq!(
        decode(b"\x1b[<35;10;5M"),
        vec![mouse(MouseKind::Move, 9, 4)]
    );
    assert_eq!(
        decode(b"\x1b[<64;10;5M"),
        vec![mouse(MouseKind::ScrollUp, 9, 4)]
    );
    assert_eq!(
        decode(b"\x1b[<65;10;5M"),
        vec![mouse(MouseKind::ScrollDown, 9, 4)]
    );
    assert_eq!(
        decode(b"\x1b[<66;10;5M"),
        vec![mouse(MouseKind::ScrollLeft, 9, 4)]
    );
    assert_eq!(
        decode(b"\x1b[<67;10;5M"),
        vec![mouse(MouseKind::ScrollRight, 9, 4)]
    );
    assert_eq!(decode(b"\x1b[<65;10;5m"), vec![]);
    assert_eq!(decode(b"\x1b[<128;10;5M"), vec![]);
}

#[test]
fn sgr_modifiers_and_bounds() {
    assert_eq!(
        decode(b"\x1b[<16;10;5M"),
        vec![Event::Mouse(
            MouseEvent::new(MouseKind::Down(Button::Left), 9, 4).with(Modifiers::CTRL)
        )]
    );
    assert_eq!(
        decode(b"\x1b[<20;10;5M"),
        vec![Event::Mouse(
            MouseEvent::new(MouseKind::Down(Button::Left), 9, 4)
                .with(Modifiers::CTRL | Modifiers::SHIFT)
        )]
    );
    assert_eq!(
        decode(b"\x1b[<44;10;5M"),
        vec![Event::Mouse(
            MouseEvent::new(MouseKind::Drag(Button::Left), 9, 4)
                .with(Modifiers::SHIFT | Modifiers::ALT)
        )]
    );
    assert_eq!(
        decode(b"\x1b[<0;0;0M"),
        vec![mouse(MouseKind::Down(Button::Left), 0, 0)]
    );
    assert_eq!(
        decode(b"\x1b[<0;70000;5M"),
        vec![mouse(MouseKind::Down(Button::Left), u16::MAX, 4)]
    );
    assert_eq!(decode(b"\x1b[<0;10M"), vec![]);
}

// --- paste, focus, replies --------------------------------------------------

#[test]
fn bracketed_paste_is_one_event() {
    assert_eq!(
        decode(b"\x1b[200~hello\r\nworld\x1b[A\x1b[201~z"),
        vec![
            Event::Paste("hello\r\nworld\x1b[A".to_owned()),
            key(KeyCode::Char('z'))
        ]
    );
}

#[test]
fn bracketed_paste_byte_at_a_time() {
    let bytes = b"\x1b[200~ab\x1b[201~";
    let mut decoder = Decoder::new();
    let mut out = Vec::new();
    for b in bytes {
        decoder.feed(std::slice::from_ref(b), &mut out);
    }
    assert_eq!(out, vec![Event::Paste("ab".to_owned())]);
    assert!(decoder.pending().is_empty());
}

#[test]
fn an_unfinished_paste_waits_even_through_flush() {
    let mut decoder = Decoder::new();
    let mut out = Vec::new();
    decoder.feed(b"\x1b[200~abc", &mut out);
    decoder.flush(&mut out);
    assert_eq!(out, vec![]);
    decoder.feed(b"\x1b[201~", &mut out);
    assert_eq!(out, vec![Event::Paste("abc".to_owned())]);
}

#[test]
fn focus_events() {
    assert_eq!(decode(b"\x1b[I"), vec![Event::Focus(true)]);
    assert_eq!(decode(b"\x1b[O"), vec![Event::Focus(false)]);
}

#[test]
fn background_reply() {
    assert_eq!(
        decode(b"\x1b]11;rgb:1e1e/1e1e/2e2e\x1b\\"),
        vec![Event::Reply(Reply::Background(Color::Rgb(30, 30, 46)))]
    );
    assert_eq!(
        decode(b"\x1b]11;rgb:ff/80/00\x07"),
        vec![Event::Reply(Reply::Background(Color::Rgb(255, 128, 0)))]
    );
    assert_eq!(
        decode(b"\x1b]11;rgb:f/8/0\x07"),
        vec![Event::Reply(Reply::Background(Color::Rgb(255, 136, 0)))]
    );
    assert_eq!(
        decode(b"\x1b]11;#1e1e2e\x1b\\"),
        vec![Event::Reply(Reply::Background(Color::Rgb(30, 30, 46)))]
    );
    assert_eq!(decode(b"\x1b]11;rgb:zz/00/00\x07"), vec![]);
    assert_eq!(decode(b"\x1b]10;rgb:ff/ff/ff\x07"), vec![]);
    assert_eq!(decode(b"\x1b_Gi=31;OK\x1b\\"), vec![]);
}

// --- fragmentation and flush -------------------------------------------------

#[test]
fn a_lone_escape_is_escape_only_after_flush() {
    let mut decoder = Decoder::new();
    let mut out = Vec::new();
    decoder.feed(b"a\x1b", &mut out);
    assert_eq!(out, vec![key(KeyCode::Char('a'))]);
    assert_eq!(decoder.pending(), b"\x1b");
    decoder.flush(&mut out);
    assert_eq!(out, vec![key(KeyCode::Char('a')), key(KeyCode::Esc)]);
    assert!(decoder.pending().is_empty());
}

#[test]
fn a_partial_sequence_completes_on_the_next_feed() {
    let mut decoder = Decoder::new();
    let mut out = Vec::new();
    decoder.feed(b"\x1b[1;", &mut out);
    assert_eq!(out, vec![]);
    decoder.feed(b"5A", &mut out);
    assert_eq!(out, vec![key_with(KeyCode::Up, Modifiers::CTRL)]);
}

#[test]
fn flushing_a_partial_sequence_yields_escape_then_text() {
    let mut decoder = Decoder::new();
    let mut out = Vec::new();
    decoder.feed(b"\x1b[", &mut out);
    decoder.flush(&mut out);
    assert_eq!(out, vec![key(KeyCode::Esc), key(KeyCode::Char('['))]);
}

#[test]
fn split_utf8_waits_and_invalid_utf8_is_skipped() {
    let mut decoder = Decoder::new();
    let mut out = Vec::new();
    decoder.feed(&[0xc3], &mut out);
    assert_eq!(out, vec![]);
    decoder.feed(&[0xa9], &mut out);
    assert_eq!(out, vec![key(KeyCode::Char('é'))]);
    assert_eq!(decode(&[0xff, b'a']), vec![key(KeyCode::Char('a'))]);
    assert_eq!(decode(&[0xc3, 0x1b, b'[', b'A']), vec![key(KeyCode::Up)]);
    assert_eq!(decode(b"\x1b\xff"), vec![]);
}

fn known_sequence() -> impl Strategy<Value = Vec<u8>> {
    prop_oneof![
        Just(b"a".to_vec()),
        Just(b"\x1b".to_vec()),
        Just(b"\x1b[A".to_vec()),
        Just(b"\x1b[1;5A".to_vec()),
        Just(b"\x1bOP".to_vec()),
        Just(b"\x1b[97;2u".to_vec()),
        Just(b"\x1b[<0;10;5M".to_vec()),
        Just(b"\x1b[200~x\x1b[201~".to_vec()),
        Just(b"\x1b]11;rgb:1e1e/1e1e/2e2e\x1b\\".to_vec()),
        Just("é".as_bytes().to_vec()),
        Just("\x1b日".as_bytes().to_vec()),
        proptest::collection::vec(any::<u8>(), 1..6),
    ]
}

proptest! {
    #[test]
    fn fragmentation_never_changes_the_events(
        pieces in proptest::collection::vec(known_sequence(), 0..12),
        cuts in proptest::collection::vec(any::<usize>(), 0..8),
    ) {
        let bytes: Vec<u8> = pieces.concat();
        let whole = decode(&bytes);

        let mut decoder = Decoder::new();
        let mut out = Vec::new();
        let mut cuts: Vec<usize> = cuts.iter().map(|c| if bytes.is_empty() { 0 } else { c % (bytes.len() + 1) }).collect();
        cuts.sort_unstable();
        let mut start = 0;
        for cut in cuts.into_iter().chain(std::iter::once(bytes.len())) {
            decoder.feed(&bytes[start..cut], &mut out);
            start = cut;
        }
        decoder.flush(&mut out);
        prop_assert_eq!(out, whole);
    }

    #[test]
    fn random_bytes_never_panic(bytes in proptest::collection::vec(any::<u8>(), 0..64)) {
        let _ = decode(&bytes);
    }
}
