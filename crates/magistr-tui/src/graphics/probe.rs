//! The startup capability probe: one write, one read, a decision.
//!
//! The request is the kitty support query, kitty's text sizing handshake
//! and then DA1 (`ESC [ c`).
//! A terminal that speaks the graphics protocol answers the query with
//! `ESC _ G i=31;OK ESC \`; every terminal answers DA1, and because DA1
//! was sent last its reply marks the end of the read — no timeout is
//! needed on a terminal that answers at all. DA1 parameter 4 advertises
//! sixel. The driver runs this in the same raw-mode window as its other
//! startup queries (OSC 11 background colour), so the parser ignores
//! anything it does not recognise, in any interleaving.
//!
//! The text sizing handshake is the one kitty's protocol prescribes:
//! a carriage return and a cursor-position report, then a space in two
//! cells (`OSC 66 ; w=2`) and another report, then a space in a two by
//! two block (`OSC 66 ; s=2`) and a third. A terminal that swallowed the
//! two OSCs answers the same column three times; one that renders them
//! answers two columns further along each time. The spaces land on the
//! alternate screen the session has already entered, and the first frame
//! clears the display, so nothing of the handshake survives it.

use super::kitty::Kitty;
use super::passthrough::Passthrough;
use super::probe_reply::ProbeReply;
use magistr_cockpit::wire::next_sequence;

/// Cursor position report: the terminal answers `ESC [ row ; col R`.
const CPR: &[u8] = b"\x1b[6n";
/// Cells kitty's text sizing handshake asks each of its two runs for.
const SIZING_CELLS: u16 = 2;

/// Builds the probe request and parses whatever the terminal sends back.
#[derive(Clone, Debug)]
pub struct Probe {
    passthrough: Passthrough,
    reply: ProbeReply,
    pending: Vec<u8>,
    columns: Vec<u16>,
}

impl Probe {
    /// A probe for a terminal reached through `passthrough`. Under tmux
    /// the kitty query is wrapped so it reaches the outer terminal; DA1 is
    /// not, tmux answers it itself.
    #[must_use]
    pub fn new(passthrough: Passthrough) -> Self {
        Self {
            passthrough,
            reply: ProbeReply::default(),
            pending: Vec::new(),
            columns: Vec::new(),
        }
    }

    /// Append the request bytes: the kitty query, the text sizing
    /// handshake, then DA1.
    ///
    /// The two `OSC 66` runs are deliberately **not** wrapped for tmux.
    /// tmux has no scaled cell in its grid, so a cockpit must not draw
    /// one through it — and because tmux answers the cursor reports
    /// itself, out of a grid the runs never reached, the columns do not
    /// advance and the handshake reports exactly the "no" that is true.
    pub fn request(&self, out: &mut Vec<u8>) {
        Kitty::new(self.passthrough).query(out);
        out.push(b'\r');
        out.extend_from_slice(CPR);
        out.extend_from_slice(b"\x1b]66;w=2; \x07");
        out.extend_from_slice(CPR);
        out.extend_from_slice(b"\x1b]66;s=2; \x07");
        out.extend_from_slice(CPR);
        out.push(b'\r');
        // Ask the terminal that owns mouse reports, never the outer terminal
        // through passthrough: graphics support does not imply pixel input.
        out.extend_from_slice(b"\x1b[?1016$p");
        out.extend_from_slice(b"\x1b[c");
    }

    /// Feed bytes read from the terminal; returns whether the reply is
    /// complete. Bytes may arrive in any fragmentation; unrecognised
    /// sequences are skipped.
    pub fn feed(&mut self, bytes: &[u8]) -> bool {
        self.pending.extend_from_slice(bytes);
        let mut consumed = 0;
        while let Some((next, sequence)) = next_sequence(&self.pending[consumed..]) {
            recognise(&mut self.reply, &mut self.columns, sequence);
            consumed += next;
        }
        self.pending.drain(..consumed);
        self.reply.complete
    }

    /// The findings so far.
    #[must_use]
    pub const fn reply(&self) -> ProbeReply {
        self.reply
    }
}

/// Fold one complete sequence into the findings: the kitty query reply
/// (`i=31`, payload `OK`), a cursor-position report
/// (`ESC [ row ; col R`) or DA1 (`ESC [ ? … c`, parameter 4 = sixel).
fn recognise(reply: &mut ProbeReply, columns: &mut Vec<u16>, sequence: &[u8]) {
    if let Some(body) = sequence.strip_prefix(b"\x1b_G") {
        let body = body.strip_suffix(b"\x1b\\").unwrap_or(body);
        let (keys, payload) = split_once(body, b';');
        if keys.split(|&b| b == b',').any(|k| k == b"i=31") && payload == b"OK" {
            reply.kitty = true;
        }
    } else if let Some(status) = sequence
        .strip_prefix(b"\x1b[?1016;")
        .and_then(|rest| rest.strip_suffix(b"$y"))
    {
        // 1/2 mean set/reset and writable. Unknown or permanently reset
        // modes must retain cell reporting.
        reply.pixel_mouse = matches!(status, b"1" | b"2" | b"3");
    } else if let Some(params) = sequence
        .strip_prefix(b"\x1b[?")
        .and_then(|rest| rest.strip_suffix(b"c"))
    {
        reply.sixel |= params.split(|&b| b == b';').any(|p| p == b"4");
        reply.complete = true;
    } else if let Some(params) = sequence
        .strip_prefix(b"\x1b[")
        .and_then(|rest| rest.strip_suffix(b"R"))
    {
        note_column(reply, columns, params);
    }
}

/// One cursor-position report of the sizing handshake. Support is two
/// advances of [`SIZING_CELLS`]: the first says the terminal honours an
/// explicit width, the second that it honours a scale — and only the
/// second is what a headline needs. Reports past the third are ignored,
/// so whatever else asks the terminal where the cursor is cannot change
/// the answer.
fn note_column(reply: &mut ProbeReply, columns: &mut Vec<u16>, params: &[u8]) {
    let Some(column) = params.split(|&b| b == b';').nth(1).and_then(number) else {
        return;
    };
    if columns.len() < 3 {
        columns.push(column);
    }
    if let [first, wide, scaled] = columns[..] {
        reply.text_sizing = wide == first.saturating_add(SIZING_CELLS)
            && scaled == wide.saturating_add(SIZING_CELLS);
    }
}

/// A decimal column, or `None` when the field is empty or not a number.
fn number(digits: &[u8]) -> Option<u16> {
    if digits.is_empty() || !digits.iter().all(u8::is_ascii_digit) {
        return None;
    }
    digits.iter().try_fold(0u16, |n, d| {
        n.checked_mul(10)?.checked_add(u16::from(d - b'0'))
    })
}

fn split_once(bytes: &[u8], at: u8) -> (&[u8], &[u8]) {
    match bytes.iter().position(|&b| b == at) {
        Some(i) => (&bytes[..i], &bytes[i + 1..]),
        None => (bytes, &[]),
    }
}

#[cfg(test)]
#[path = "probe.tests.rs"]
mod tests;
