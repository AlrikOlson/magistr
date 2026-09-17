//! The incremental decoder: bytes in, events out, any fragmentation.

use super::csi::Params;
use super::event::Event;
use super::reply::Reply;
use crate::wire::next_sequence;
use magistr_cockpit::input::Button;
use magistr_cockpit::input::KeyCode;
use magistr_cockpit::input::KeyEvent;
use magistr_cockpit::input::KeyKind;
use magistr_cockpit::input::Modifiers;
use magistr_cockpit::input::MouseEvent;
use magistr_cockpit::input::MouseKind;
use magistr_cockpit::render::Color;

const ESC: u8 = 0x1b;
const PASTE_END: &[u8] = b"\x1b[201~";

/// Turns the terminal's byte stream into [`Event`]s.
///
/// Bytes may arrive in any fragmentation: an incomplete sequence, an
/// incomplete UTF-8 scalar or an unfinished paste stays pending until the
/// next [`Decoder::feed`]. A lone `ESC` is ambiguous — the start of a
/// sequence or the Escape key — so it, too, waits; the runtime calls
/// [`Decoder::flush`] after a read timeout to resolve it as Escape.
#[derive(Clone, Debug, Default)]
pub struct Decoder {
    pending: Vec<u8>,
    pasting: bool,
}

impl Decoder {
    /// A decoder with nothing pending.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Decode `bytes` (plus whatever was pending) into `out`.
    pub fn feed(&mut self, bytes: &[u8], out: &mut Vec<Event>) {
        self.pending.extend_from_slice(bytes);
        let mut consumed = 0;
        loop {
            let rest = &self.pending[consumed..];
            if rest.is_empty() {
                break;
            }
            if self.pasting {
                let Some(end) = rest.windows(PASTE_END.len()).position(|w| w == PASTE_END) else {
                    break;
                };
                out.push(Event::Paste(
                    String::from_utf8_lossy(&rest[..end]).into_owned(),
                ));
                self.pasting = false;
                consumed += end + PASTE_END.len();
            } else if rest[0] == ESC {
                let Some((len, sequence)) = next_sequence(rest) else {
                    break;
                };
                self.pasting = decode_sequence(sequence, out);
                consumed += len;
            } else {
                let run = rest.iter().position(|&b| b == ESC).unwrap_or(rest.len());
                let taken = decode_text(&rest[..run], run == rest.len(), out);
                consumed += taken;
                if taken < run {
                    break;
                }
            }
        }
        self.pending.drain(..consumed);
    }

    /// Resolve a pending lone `ESC` as the Escape key (call after a read
    /// timeout); anything that followed it is decoded afresh as text.
    pub fn flush(&mut self, out: &mut Vec<Event>) {
        if self.pasting
            || self.pending.first() != Some(&ESC)
            || next_sequence(&self.pending).is_some()
        {
            return;
        }
        out.push(Event::Key(KeyEvent::new(KeyCode::Esc)));
        let rest = self.pending.split_off(1);
        self.pending.clear();
        self.feed(&rest, out);
    }

    /// Bytes waiting for their continuation.
    #[must_use]
    pub fn pending(&self) -> &[u8] {
        &self.pending
    }
}

/// Decode a run of non-ESC bytes as typed characters; returns how many
/// bytes were consumed — everything, except a trailing incomplete scalar
/// when `at_end` (it waits for the next feed). Invalid bytes are skipped.
fn decode_text(bytes: &[u8], at_end: bool, out: &mut Vec<Event>) -> usize {
    let mut consumed = 0;
    while consumed < bytes.len() {
        let rest = &bytes[consumed..];
        let (valid, error) = match std::str::from_utf8(rest) {
            Ok(text) => (text, None),
            Err(e) => (
                std::str::from_utf8(&rest[..e.valid_up_to()])
                    .expect("the prefix before a UTF-8 error is valid"),
                Some(e.error_len()),
            ),
        };
        out.extend(valid.chars().map(|c| Event::Key(char_key(c))));
        consumed += valid.len();
        match error {
            None => {}
            Some(Some(invalid)) => consumed += invalid,
            Some(None) if at_end => break,
            Some(None) => consumed += 1,
        }
    }
    consumed
}

/// The key a typed character stands for: control characters become
/// `Char + CTRL` (or Enter, Tab, Backspace), uppercase ASCII carries
/// `SHIFT`.
fn char_key(c: char) -> KeyEvent {
    match c {
        '\r' | '\n' => KeyEvent::new(KeyCode::Enter),
        '\t' => KeyEvent::new(KeyCode::Tab),
        '\u{7f}' | '\u{8}' => KeyEvent::new(KeyCode::Backspace),
        '\0' => KeyEvent::new(KeyCode::Char(' ')).with(Modifiers::CTRL),
        c if (c as u32) < 0x20 => {
            let letter = match c as u32 {
                0x1c => '\\',
                0x1d => ']',
                0x1e => '^',
                0x1f => '_',
                n => char::from_u32(n + 0x60).unwrap_or(c),
            };
            KeyEvent::new(KeyCode::Char(letter)).with(Modifiers::CTRL)
        }
        c if c.is_ascii_uppercase() => KeyEvent::new(KeyCode::Char(c)).with(Modifiers::SHIFT),
        c => KeyEvent::new(KeyCode::Char(c)),
    }
}

/// Decode one complete escape sequence; returns whether it opened a
/// bracketed paste.
fn decode_sequence(sequence: &[u8], out: &mut Vec<Event>) -> bool {
    let Some((&last, body)) = sequence.split_last() else {
        return false;
    };
    match sequence.get(1) {
        Some(b'[') => return decode_csi(&body[2..], last, out),
        Some(b'O') => {
            if let Some(code) = legacy_code(last) {
                out.push(Event::Key(KeyEvent::new(code)));
            }
        }
        Some(b']') => decode_osc(&sequence[2..], out),
        Some(b'_' | b'P') => {}
        Some(&ESC) => out.push(Event::Key(KeyEvent::new(KeyCode::Esc))),
        Some(_) => {
            if let Some(c) = std::str::from_utf8(&sequence[1..])
                .ok()
                .and_then(|s| s.chars().next())
            {
                let key = char_key(c);
                out.push(Event::Key(key.with(key.mods | Modifiers::ALT)));
            }
        }
        None => {}
    }
    false
}

/// A CSI sequence: `params` are the bytes between `ESC [` and `last`.
fn decode_csi(params: &[u8], last: u8, out: &mut Vec<Event>) -> bool {
    if let Some(rest) = params.strip_prefix(b"<") {
        decode_mouse(rest, last, out);
        return false;
    }
    if let Some(rest) = params.strip_prefix(b"?") {
        if last == b'u'
            && let Some(flags) = Params::parse(rest).get(0)
        {
            let flags = u8::try_from(flags).unwrap_or(u8::MAX);
            out.push(Event::Reply(Reply::KeyboardFlags(flags)));
        }
        return false;
    }
    let p = Params::parse(params);
    let mods = kitty_modifiers(p.get(1).unwrap_or(1));
    let kind = key_kind(p.sub(1, 1));
    match last {
        b'~' => match p.get(0) {
            Some(200) => return true,
            Some(n) => {
                if let Some(code) = tilde_code(n) {
                    out.push(Event::Key(KeyEvent::new(code).with(mods).kind(kind)));
                }
            }
            None => {}
        },
        b'u' => {
            if let Some(code) = kitty_code(&p, mods) {
                out.push(Event::Key(KeyEvent::new(code).with(mods).kind(kind)));
            }
        }
        b'I' => out.push(Event::Focus(true)),
        b'O' => out.push(Event::Focus(false)),
        b't' => {
            if let Some(reply) = window_reply(&p) {
                out.push(Event::Reply(reply));
            }
        }
        _ => {
            if let Some(code) = legacy_code(last) {
                out.push(Event::Key(KeyEvent::new(code).with(mods).kind(kind)));
            }
        }
    }
    false
}

/// Decode terminal modifier fields before constructing neutral modifiers.
fn kitty_modifiers(field: u32) -> Modifiers {
    let bits = field.saturating_sub(1);
    [
        (1, Modifiers::SHIFT),
        (2, Modifiers::ALT),
        (4, Modifiers::CTRL),
        (8, Modifiers::SUPER),
    ]
    .into_iter()
    .filter(|(bit, _)| bits & bit != 0)
    .fold(Modifiers::NONE, |all, (_, modifier)| all | modifier)
}

fn sgr_modifiers(button: u32) -> Modifiers {
    [
        (4, Modifiers::SHIFT),
        (8, Modifiers::ALT),
        (16, Modifiers::CTRL),
    ]
    .into_iter()
    .filter(|(bit, _)| button & bit != 0)
    .fold(Modifiers::NONE, |all, (_, modifier)| all | modifier)
}

fn key_kind(field: Option<u32>) -> KeyKind {
    match field {
        Some(2) => KeyKind::Repeat,
        Some(3) => KeyKind::Release,
        _ => KeyKind::Press,
    }
}

/// The key behind a CSI / SS3 final letter (`CSI 1;5 A`, `ESC O P`).
/// `R` is left out: `CSI n;m R` is also a cursor-position report.
fn legacy_code(last: u8) -> Option<KeyCode> {
    Some(match last {
        b'A' => KeyCode::Up,
        b'B' => KeyCode::Down,
        b'C' => KeyCode::Right,
        b'D' => KeyCode::Left,
        b'H' => KeyCode::Home,
        b'F' => KeyCode::End,
        b'P' => KeyCode::F(1),
        b'Q' => KeyCode::F(2),
        b'S' => KeyCode::F(4),
        _ => return None,
    })
}

/// The key behind `CSI n ~`.
fn tilde_code(n: u32) -> Option<KeyCode> {
    Some(match n {
        1 | 7 => KeyCode::Home,
        2 => KeyCode::Insert,
        3 => KeyCode::Delete,
        4 | 8 => KeyCode::End,
        5 => KeyCode::PageUp,
        6 => KeyCode::PageDown,
        11..=15 => KeyCode::F(u8::try_from(n - 10).ok()?),
        17..=21 => KeyCode::F(u8::try_from(n - 11).ok()?),
        23..=24 => KeyCode::F(u8::try_from(n - 12).ok()?),
        _ => return None,
    })
}

/// The key behind `CSI code[:shifted] ; mods u` (kitty). Keypad keys fold
/// onto their main-keyboard twins; lock, modifier and media keys are
/// dropped.
fn kitty_code(p: &Params, mods: Modifiers) -> Option<KeyCode> {
    let code = p.get(0)?;
    Some(match code {
        27 => KeyCode::Esc,
        13 | 57414 => KeyCode::Enter,
        9 => KeyCode::Tab,
        127 => KeyCode::Backspace,
        57376..=57398 => KeyCode::F(u8::try_from(code - 57376 + 13).ok()?),
        57399..=57408 => KeyCode::Char(char::from_u32(code - 57399 + u32::from(b'0'))?),
        57409 => KeyCode::Char('.'),
        57410 => KeyCode::Char('/'),
        57411 => KeyCode::Char('*'),
        57412 => KeyCode::Char('-'),
        57413 => KeyCode::Char('+'),
        57415 => KeyCode::Char('='),
        57416 => KeyCode::Char(','),
        57417 => KeyCode::Left,
        57418 => KeyCode::Right,
        57419 => KeyCode::Up,
        57420 => KeyCode::Down,
        57421 => KeyCode::PageUp,
        57422 => KeyCode::PageDown,
        57423 => KeyCode::Home,
        57424 => KeyCode::End,
        57425 => KeyCode::Insert,
        57426 => KeyCode::Delete,
        57358..=57363 | 57427..=u32::MAX => return None,
        _ => {
            let c = char::from_u32(code)?;
            if mods.contains(Modifiers::SHIFT) {
                match p.sub(0, 1).and_then(char::from_u32) {
                    Some(shifted) => KeyCode::Char(shifted),
                    None => KeyCode::Char(c.to_ascii_uppercase()),
                }
            } else {
                KeyCode::Char(c)
            }
        }
    })
}

/// `CSI 6 ; h ; w t` (cell pixels) and `CSI 4 ; h ; w t` (window pixels).
fn window_reply(p: &Params) -> Option<Reply> {
    let height = u16::try_from(p.get(1)?).ok()?;
    let width = u16::try_from(p.get(2)?).ok()?;
    match p.get(0)? {
        6 => Some(Reply::CellPixels { width, height }),
        4 => Some(Reply::WindowPixels { width, height }),
        _ => None,
    }
}

/// `CSI < b ; x ; y M|m`.
fn decode_mouse(params: &[u8], last: u8, out: &mut Vec<Event>) {
    let p = Params::parse(params);
    let (Some(b), Some(x), Some(y)) = (p.get(0), p.get(1), p.get(2)) else {
        return;
    };
    let button = match b & 3 {
        0 => Some(Button::Left),
        1 => Some(Button::Middle),
        2 => Some(Button::Right),
        _ => None,
    };
    let kind = if b & 64 != 0 {
        if last != b'M' {
            return;
        }
        match b & 3 {
            0 => MouseKind::ScrollUp,
            1 => MouseKind::ScrollDown,
            2 => MouseKind::ScrollLeft,
            _ => MouseKind::ScrollRight,
        }
    } else if b & 128 != 0 {
        return;
    } else if b & 32 != 0 {
        button.map_or(MouseKind::Move, MouseKind::Drag)
    } else {
        let Some(button) = button else {
            return;
        };
        if last == b'm' {
            MouseKind::Up(button)
        } else {
            MouseKind::Down(button)
        }
    };
    let cell = |n: u32| u16::try_from(n.saturating_sub(1)).unwrap_or(u16::MAX);
    out.push(Event::Mouse(
        MouseEvent::new(kind, cell(x), cell(y)).with(sgr_modifiers(b)),
    ));
}

/// `OSC 11 ; colour ST|BEL` — the background colour reply.
fn decode_osc(body: &[u8], out: &mut Vec<Event>) {
    let body = body
        .strip_suffix(b"\x1b\\")
        .or_else(|| body.strip_suffix(b"\x07"))
        .unwrap_or(body);
    if let Some(colour) = body.strip_prefix(b"11;").and_then(parse_colour) {
        out.push(Event::Reply(Reply::Background(colour)));
    }
}

/// `rgb:RRRR/GGGG/BBBB` (1–4 hex digits per channel, scaled to 8 bits) or
/// `#RRGGBB`.
fn parse_colour(bytes: &[u8]) -> Option<Color> {
    let text = std::str::from_utf8(bytes).ok()?;
    if let Some(rgb) = text.strip_prefix("rgb:") {
        let mut channels = rgb.split('/').map(scale_hex);
        let (r, g, b) = (channels.next()??, channels.next()??, channels.next()??);
        return Some(Color::Rgb(r, g, b));
    }
    let hex = text.strip_prefix('#')?;
    if hex.len() != 6 {
        return None;
    }
    let mut channels = (0..3).map(|i| scale_hex(&hex[i * 2..i * 2 + 2]));
    Some(Color::Rgb(
        channels.next()??,
        channels.next()??,
        channels.next()??,
    ))
}

/// A 1–4 digit hex channel scaled to `0..=255`.
fn scale_hex(digits: &str) -> Option<u8> {
    if digits.is_empty() || digits.len() > 4 {
        return None;
    }
    let value = u32::from_str_radix(digits, 16).ok()?;
    let max = (1u32 << (4 * digits.len())) - 1;
    u8::try_from((value * 255 + max / 2) / max).ok()
}

#[cfg(test)]
#[path = "decoder.tests.rs"]
mod tests;
