//! Finding one complete escape sequence at the head of a byte buffer.

const ESC: u8 = 0x1b;

/// The first complete escape sequence at the head of `bytes` (leading
/// non-ESC bytes are skipped): `(bytes consumed, the sequence)`, or `None`
/// while the head is incomplete or holds no ESC at all.
///
/// The grammar is the one every reply and key report shares:
/// APC (`ESC _`), DCS (`ESC P`) and OSC (`ESC ]`) run to ST (`ESC \`) or
/// BEL; CSI (`ESC [`) runs to its final byte (`0x40..=0x7e`); SS3
/// (`ESC O`) is ESC plus two bytes; anything else is ESC plus one UTF-8
/// scalar (an alt-prefixed key), so `ESC é` is not cut in half.
#[must_use]
pub fn next_sequence(bytes: &[u8]) -> Option<(usize, &[u8])> {
    let start = bytes.iter().position(|&b| b == ESC)?;
    let rest = &bytes[start..];
    let end = match *rest.get(1)? {
        b'_' | b'P' | b']' => rest
            .windows(2)
            .position(|w| w == b"\x1b\\")
            .map(|i| i + 2)
            .into_iter()
            .chain(rest.iter().position(|&b| b == 0x07).map(|i| i + 1))
            .min()?,
        b'[' => rest
            .get(2..)?
            .iter()
            .position(|&b| (0x40..=0x7e).contains(&b))
            .map(|i| i + 3)?,
        b'O' => {
            rest.get(2)?;
            3
        }
        lead => {
            let len = 1 + utf8_len(lead);
            rest.get(1..len)?;
            len
        }
    };
    Some((start + end, &rest[..end]))
}

/// Bytes in the UTF-8 scalar `lead` starts (a continuation or invalid
/// lead counts as one byte so the walker always makes progress).
const fn utf8_len(lead: u8) -> usize {
    match lead {
        0xc0..=0xdf => 2,
        0xe0..=0xef => 3,
        0xf0..=0xf7 => 4,
        _ => 1,
    }
}

#[cfg(test)]
#[path = "sequence.tests.rs"]
mod tests;
