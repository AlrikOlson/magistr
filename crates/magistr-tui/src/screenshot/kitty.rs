//! What a kitty terminal would be holding after a frame: the graphics
//! APC commands read back out of the bytes the cockpit just wrote.
//!
//! This is the screenshot gate's answer to the limit every pixel
//! surface shipped with — "the rasterizer has no kitty, so the pixel
//! path has no golden". The encoder is ours, so the decoder can be too:
//! [`decode`] plays the frame's own byte stream the way a terminal
//! would and hands back the images actually on screen, which is what
//! lets a PNG golden pin the pixel path rather than the cell fallback.
//!
//! Only what the cockpit emits is understood — `a=t` transmits (raw or
//! `o=z`), `a=p` placements and `a=d` deletes, unwrapped, since the
//! screenshot's `Screen` runs without tmux passthrough. Anything else
//! in the stream is skipped rather than guessed at.
//!
//! **A placement's cell origin comes from the cursor**, because
//! `Compositor::finish` writes `Encoder::move_to` immediately before
//! every `a=p`. The encoder elides a move to where the cursor already
//! is, so the origin is only recoverable when a `CUP` really went out —
//! and it always does here: a screenshot frame is the first frame of a
//! fresh [`Screen`](crate::render::Screen), which clears the display
//! (dropping the encoder's cursor) and then redraws to the last cell of
//! the last row (which leaves it dropped, the write having reached the
//! right edge). A placement with no cursor to its name is therefore a
//! decode this module refuses rather than places at a guess.

use std::collections::HashMap;

use super::placement::Placement;
use magistr_cockpit::base64;
use magistr_cockpit::pixel::PixelFormat;

/// An image the terminal is holding, mid-transmit or complete.
#[derive(Clone, Debug)]
struct Stored {
    format: PixelFormat,
    width: u32,
    height: u32,
    /// zlib (`o=z`) rather than raw pixels.
    zlib: bool,
    /// The base64 payload, appended chunk by chunk.
    encoded: Vec<u8>,
}

/// Every image on screen after `bytes`, in placement order.
///
/// Placements are returned in the order the frame made them, which is
/// the order they must be composited in — the cockpit places every
/// surface at the same z, so ties are broken by the wire's own order.
///
/// A placement whose image never arrived, whose payload will not
/// base64-decode or inflate, or which was made with no cursor to locate
/// it, is dropped: a still that silently misplaces an image would be
/// worse than one that is missing it, and
/// `every_surface_the_frame_shows_is_decoded` is what turns a drop into
/// a failing test rather than a quiet hole.
pub(crate) fn decode(bytes: &[u8]) -> Vec<Placement> {
    let mut store: HashMap<u32, Stored> = HashMap::new();
    let mut placements: Vec<Pending> = Vec::new();
    // Which image the transmit in flight belongs to: only a transmit's
    // first chunk carries `i=`, so the continuations are addressed by
    // position in the stream.
    let mut transmitting: Option<u32> = None;
    let mut cursor: Option<(u16, u16)> = None;
    let mut at = 0;

    while at < bytes.len() {
        if bytes[at..].starts_with(b"\x1b_G") {
            let Some(len) = apc_len(&bytes[at..]) else {
                break;
            };
            let body = &bytes[at + 3..at + len];
            command(body, &mut store, &mut transmitting, &mut placements, cursor);
            at += len + 2;
            continue;
        }
        if bytes[at..].starts_with(b"\x1b[")
            && let Some((len, moved)) = csi_cup(&bytes[at..])
        {
            if let Some(position) = moved {
                cursor = Some(position);
            }
            if bytes[at..at + len] == *b"\x1b[2J" {
                placements.clear();
            }
            at += len;
            continue;
        }
        at += 1;
    }

    placements
        .into_iter()
        .filter_map(|pending| pending.resolve(&store))
        .collect()
}

/// A placement waiting for the image store to be complete — the frame
/// transmits before it places, but reading the store as we go would
/// miss a `d=I` later in the same frame.
struct Pending {
    image: u32,
    col: u16,
    row: u16,
    cols: u16,
    rows: u16,
    z: i32,
}

impl Pending {
    /// The placement's pixels, or `None` when the image is not there or
    /// will not decode.
    fn resolve(self, store: &HashMap<u32, Stored>) -> Option<Placement> {
        let stored = store.get(&self.image)?;
        let rgba = stored.rgba()?;
        Some(Placement {
            col: self.col,
            row: self.row,
            cols: self.cols,
            rows: self.rows,
            width: stored.width,
            height: stored.height,
            rgba,
            z: self.z,
        })
    }
}

impl Stored {
    /// The image as straight RGBA, widening `f=24` to opaque and
    /// inflating an `o=z` payload first.
    fn rgba(&self) -> Option<Vec<u8>> {
        let packed = base64::decode(&self.encoded)?;
        let raw = if self.zlib { inflate(&packed)? } else { packed };
        let pixels = (self.width as usize).checked_mul(self.height as usize)?;
        if raw.len() != pixels * self.format.bytes_per_pixel() {
            return None;
        }
        Some(match self.format {
            PixelFormat::Rgba => raw,
            PixelFormat::Rgb => raw
                .chunks_exact(3)
                .flat_map(|p| [p[0], p[1], p[2], 255])
                .collect(),
        })
    }
}

/// A zlib (RFC 1950) stream inflated, or `None` when it is not one.
fn inflate(stream: &[u8]) -> Option<Vec<u8>> {
    use std::io::Read as _;
    let mut out = Vec::new();
    flate2::read::ZlibDecoder::new(stream)
        .read_to_end(&mut out)
        .ok()?;
    Some(out)
}

/// Act on one APC body (everything between `ESC _ G` and `ESC \`).
fn command(
    body: &[u8],
    store: &mut HashMap<u32, Stored>,
    transmitting: &mut Option<u32>,
    placements: &mut Vec<Pending>,
    cursor: Option<(u16, u16)>,
) {
    let split = body.iter().position(|b| *b == b';');
    let (keys, payload) = match split {
        Some(at) => (&body[..at], &body[at + 1..]),
        None => (body, &body[body.len()..]),
    };
    let keys = parse_keys(keys);
    match keys.get("a").copied() {
        // A continuation chunk carries no `a` at all.
        None => append(store, transmitting, &keys, payload),
        Some("t") => {
            let id = keys.get("i").and_then(|v| number(v));
            let (Some(id), Some(width), Some(height)) = (
                id,
                keys.get("s").and_then(|v| number(v)),
                keys.get("v").and_then(|v| number(v)),
            ) else {
                return;
            };
            let format = match keys.get("f").and_then(|v| number(v)) {
                Some(24) => PixelFormat::Rgb,
                _ => PixelFormat::Rgba,
            };
            store.insert(
                id,
                Stored {
                    format,
                    width,
                    height,
                    zlib: keys.get("o") == Some(&"z"),
                    encoded: payload.to_vec(),
                },
            );
            *transmitting = (keys.get("m") == Some(&"1")).then_some(id);
        }
        Some("p") => {
            let (Some(image), Some(cols), Some(rows), Some((col, row))) = (
                keys.get("i").and_then(|v| number(v)),
                keys.get("c").and_then(|v| number(v)),
                keys.get("r").and_then(|v| number(v)),
                cursor,
            ) else {
                return;
            };
            placements.push(Pending {
                image,
                col,
                row,
                cols: cols as u16,
                rows: rows as u16,
                z: keys.get("z").and_then(|v| signed(v)).unwrap_or(0),
            });
        }
        Some("d") => delete(store, placements, &keys),
        Some(_) => {}
    }
}

/// A transmit's continuation chunk: more base64 for the image in
/// flight, and the end of it when `m=0`.
fn append(
    store: &mut HashMap<u32, Stored>,
    transmitting: &mut Option<u32>,
    keys: &HashMap<&str, &str>,
    payload: &[u8],
) {
    let Some(id) = *transmitting else {
        return;
    };
    if let Some(stored) = store.get_mut(&id) {
        stored.encoded.extend_from_slice(payload);
    }
    if keys.get("m") != Some(&"1") {
        *transmitting = None;
    }
}

/// `a=d`: drop one image's placements (`d=i`), or the image and its
/// placements (`d=I`).
fn delete(
    store: &mut HashMap<u32, Stored>,
    placements: &mut Vec<Pending>,
    keys: &HashMap<&str, &str>,
) {
    let Some(id) = keys.get("i").and_then(|v| number(v)) else {
        return;
    };
    placements.retain(|pending| pending.image != id);
    if keys.get("d") == Some(&"I") {
        store.remove(&id);
    }
}

/// `k=v,k=v,…` as a map. A key with no `=` is ignored rather than
/// guessed at.
fn parse_keys(keys: &[u8]) -> HashMap<&str, &str> {
    keys.split(|b| *b == b',')
        .filter_map(|pair| {
            let at = pair.iter().position(|b| *b == b'=')?;
            let key = std::str::from_utf8(&pair[..at]).ok()?;
            let value = std::str::from_utf8(&pair[at + 1..]).ok()?;
            Some((key, value))
        })
        .collect()
}

/// A decimal key value.
fn number(value: &str) -> Option<u32> {
    value.parse().ok()
}

/// A possibly negative decimal key value (`z`).
fn signed(value: &str) -> Option<i32> {
    value.parse().ok()
}

/// The length of the APC starting at `bytes[0]`, up to but not
/// including its `ESC \` terminator.
fn apc_len(bytes: &[u8]) -> Option<usize> {
    bytes
        .windows(2)
        .position(|w| w == b"\x1b\\")
        .filter(|end| *end >= 3)
}

/// The length of the CSI starting at `bytes[0]`, and the cell it moved
/// the cursor to when it is a `CUP` (`CSI row ; col H`, one-based).
fn csi_cup(bytes: &[u8]) -> Option<(usize, Option<(u16, u16)>)> {
    let end = bytes[2..]
        .iter()
        .position(|b| (0x40..=0x7e).contains(b))
        .map(|at| at + 2)?;
    let params = std::str::from_utf8(&bytes[2..end]).ok();
    if bytes[end] != b'H' {
        return Some((end + 1, None));
    }
    let moved = params.and_then(|params| {
        let (row, col) = params.split_once(';')?;
        let row: u16 = row.parse().ok()?;
        let col: u16 = col.parse().ok()?;
        Some((col.saturating_sub(1), row.saturating_sub(1)))
    });
    Some((end + 1, moved))
}

#[cfg(test)]
#[path = "kitty.tests.rs"]
mod tests;
