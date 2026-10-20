//! Plain text from process output, shared by command diagnostics, quality
//! gates and agent terminals. Raw capture remains with each caller.

/// Where the stripper stands inside a control sequence between two
/// chunks: a sequence split across reads must still vanish whole.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum State {
    /// Printing.
    #[default]
    Ground,
    /// After `ESC`: the next byte names the sequence.
    Escape,
    /// After `ESC` and an intermediate (`ESC (`, `ESC #`…): one final
    /// byte ends it.
    EscapeIntermediate,
    /// Inside `CSI`: parameters and intermediates until a final byte.
    Csi,
    /// Inside an OSC, DCS, APC, PM or SOS string: until `BEL` or `ST`.
    Str,
    /// Inside a string and just past `ESC`: a `\` is the `ST` that
    /// ends it.
    StrEscape,
}

/// Strips escape sequences and control characters from a byte stream
/// fed in arbitrary chunks, answering the printable text of each.
///
/// What survives: printable bytes, `\n` and `\t`. A carriage return
/// becomes a line break — `\r\n` is one break, a lone `\r` (a progress
/// bar overwriting itself) is one too, so the overwrites read as lines
/// rather than run together. Every `ESC`-introduced sequence goes:
/// CSI (colours, cursor moves, erases), OSC/DCS/APC/PM/SOS strings to
/// their `BEL` or `ST`, and the two- and three-byte sequences (charset
/// designations, keypad modes). Every other C0 control and `DEL` is
/// dropped. A UTF-8 sequence cut by a chunk boundary is held for the
/// next chunk; an invalid one is replaced.
#[derive(Debug, Default)]
pub struct EscapeStripper {
    state: State,
    /// Whether the last printable-stream byte was `\r`, so a following
    /// `\n` is the same line break.
    after_cr: bool,
    /// The trailing bytes of an unfinished UTF-8 sequence.
    pending: Vec<u8>,
}

impl EscapeStripper {
    /// The text in `bytes`, continuing whatever the previous chunk left
    /// open.
    pub fn feed(&mut self, bytes: &[u8]) -> String {
        let mut out = std::mem::take(&mut self.pending);
        for &b in bytes {
            self.step(b, &mut out);
        }
        self.decode(out)
    }

    fn step(&mut self, b: u8, out: &mut Vec<u8>) {
        const ESC: u8 = 0x1B;
        const BEL: u8 = 0x07;
        match self.state {
            State::Ground => match b {
                ESC => self.state = State::Escape,
                b'\r' => {
                    out.push(b'\n');
                    self.after_cr = true;
                }
                b'\n' => {
                    if !self.after_cr {
                        out.push(b'\n');
                    }
                    self.after_cr = false;
                }
                b'\t' | 0x20..=0x7E | 0x80..=0xFF => {
                    out.push(b);
                    self.after_cr = false;
                }
                _ => {}
            },
            State::Escape => {
                self.state = match b {
                    b'[' => State::Csi,
                    b']' | b'P' | b'X' | b'^' | b'_' => State::Str,
                    0x20..=0x2F => State::EscapeIntermediate,
                    ESC => State::Escape,
                    _ => State::Ground,
                };
            }
            State::EscapeIntermediate => {
                self.state = match b {
                    0x20..=0x2F => State::EscapeIntermediate,
                    ESC => State::Escape,
                    0x30..=0x7E => State::Ground,
                    _ => State::EscapeIntermediate,
                };
            }
            State::Csi => {
                self.state = match b {
                    0x40..=0x7E => State::Ground,
                    ESC => State::Escape,
                    _ => State::Csi,
                };
            }
            State::Str => {
                self.state = match b {
                    BEL => State::Ground,
                    ESC => State::StrEscape,
                    _ => State::Str,
                };
            }
            State::StrEscape => {
                if b == b'\\' {
                    self.state = State::Ground;
                } else {
                    self.state = State::Escape;
                    self.step(b, out);
                }
            }
        }
    }

    /// The printable bytes as a string, holding back an unfinished
    /// trailing UTF-8 sequence for the next chunk.
    fn decode(&mut self, out: Vec<u8>) -> String {
        match String::from_utf8(out) {
            Ok(text) => text,
            Err(err) => {
                let valid = err.utf8_error().valid_up_to();
                let incomplete = err.utf8_error().error_len().is_none();
                let mut bytes = err.into_bytes();
                if incomplete {
                    self.pending = bytes.split_off(valid);
                    // Everything before `valid` is UTF-8 by construction.
                    String::from_utf8(bytes).unwrap_or_default()
                } else {
                    String::from_utf8_lossy(&bytes).into_owned()
                }
            }
        }
    }
}

#[cfg(test)]
#[path = "strip_escapes.tests.rs"]
mod tests;
