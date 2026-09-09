//! RFC 4648 base64 (standard alphabet, `=` padded): the kitty graphics
//! protocol carries pixel data this way, and twenty lines beat a dependency.

/// The standard alphabet, in value order.
const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Append the base64 encoding of `input` to `out`.
///
/// Every three input bytes become four output bytes; a trailing one or two
/// bytes are padded with `=` so the output length is always a multiple of
/// four.
pub fn encode(input: &[u8], out: &mut Vec<u8>) {
    out.reserve(input.len().div_ceil(3) * 4);
    let mut triples = input.chunks_exact(3);
    for triple in &mut triples {
        let n = (u32::from(triple[0]) << 16) | (u32::from(triple[1]) << 8) | u32::from(triple[2]);
        out.extend_from_slice(&[sextet(n >> 18), sextet(n >> 12), sextet(n >> 6), sextet(n)]);
    }
    match *triples.remainder() {
        [a] => {
            let n = u32::from(a) << 16;
            out.extend_from_slice(&[sextet(n >> 18), sextet(n >> 12), b'=', b'=']);
        }
        [a, b] => {
            let n = (u32::from(a) << 16) | (u32::from(b) << 8);
            out.extend_from_slice(&[sextet(n >> 18), sextet(n >> 12), sextet(n >> 6), b'=']);
        }
        _ => {}
    }
}

/// The bytes `input` encodes, or `None` when it is not standard base64.
///
/// The agent's image blocks arrive this way. Whitespace between quads is
/// skipped — a JSON string can carry line breaks — but any other character
/// outside the alphabet, a padded quad in the middle, or a length that is
/// not a multiple of four once padded, is a refusal rather than a guess:
/// a half-decoded PNG is worse than the alt text.
#[must_use]
pub fn decode(input: &[u8]) -> Option<Vec<u8>> {
    let clean: Vec<u8> = input
        .iter()
        .copied()
        .filter(|c| !c.is_ascii_whitespace())
        .collect();
    if !clean.len().is_multiple_of(4) {
        return None;
    }
    let mut out = Vec::with_capacity(clean.len() / 4 * 3);
    let quads = clean.len() / 4;
    for (index, quad) in clean.chunks_exact(4).enumerate() {
        let pad = quad.iter().filter(|&&c| c == b'=').count();
        // Padding exists only in the last quad, and only as `=` or `==`
        // at its end.
        if pad > 0 && (index + 1 != quads || pad > 2 || quad[3] != b'=') {
            return None;
        }
        if pad == 2 && quad[2] != b'=' {
            return None;
        }
        let mut n = 0u32;
        for &c in &quad[..4 - pad] {
            n = (n << 6) | u32::from(sextet_value(c)?);
        }
        n <<= 6 * u32::try_from(pad).ok()?;
        let bytes = n.to_be_bytes();
        out.extend_from_slice(&bytes[1..4 - pad]);
    }
    Some(out)
}

/// The six-bit value of an alphabet character, `None` for anything else.
fn sextet_value(c: u8) -> Option<u8> {
    let value = match c {
        b'A'..=b'Z' => c - b'A',
        b'a'..=b'z' => c - b'a' + 26,
        b'0'..=b'9' => c - b'0' + 52,
        b'+' => 62,
        b'/' => 63,
        _ => return None,
    };
    Some(value)
}

/// The alphabet character for the low six bits of `n`.
fn sextet(n: u32) -> u8 {
    ALPHABET[(n & 0x3f) as usize]
}

#[cfg(test)]
#[path = "base64.tests.rs"]
mod tests;
