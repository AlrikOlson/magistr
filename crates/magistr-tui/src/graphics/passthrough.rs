//! Getting a graphics command past tmux.
//!
//! tmux swallows APC and most DCS sequences unless the application wraps
//! them in its passthrough envelope `ESC P tmux; … ESC \`, doubling every
//! ESC inside so tmux does not interpret them — and only when the tmux
//! option `allow-passthrough` is on (tmux 3.3+). The probe learns whether
//! the envelope gets through; the encoder just applies it.

/// Whether commands go straight to the terminal or through tmux.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Passthrough {
    /// The terminal reads our bytes directly.
    Direct,
    /// tmux sits in between: wrap commands in its passthrough envelope.
    Tmux,
}

impl Passthrough {
    /// [`Passthrough::Tmux`] when tmux is in the way, else direct.
    #[must_use]
    pub const fn for_tmux(tmux: bool) -> Self {
        if tmux { Self::Tmux } else { Self::Direct }
    }

    /// Is the envelope in use?
    #[must_use]
    pub const fn is_tmux(self) -> bool {
        matches!(self, Self::Tmux)
    }

    /// Append `sequence` to `out`, wrapped for tmux when needed. Every ESC
    /// inside the envelope is doubled.
    pub fn wrap(self, sequence: &[u8], out: &mut Vec<u8>) {
        self.open(out);
        match self {
            Self::Direct => out.extend_from_slice(sequence),
            Self::Tmux => {
                for &byte in sequence {
                    if byte == 0x1b {
                        out.push(0x1b);
                    }
                    out.push(byte);
                }
            }
        }
        self.close(out);
    }

    /// Start an envelope (`ESC P tmux;` under tmux, nothing otherwise).
    /// Pair with [`Passthrough::close`] and write ESC bytes in between
    /// through [`Passthrough::esc`].
    pub fn open(self, out: &mut Vec<u8>) {
        if self.is_tmux() {
            out.extend_from_slice(b"\x1bPtmux;");
        }
    }

    /// One ESC of the wrapped sequence: doubled inside the envelope.
    pub fn esc(self, out: &mut Vec<u8>) {
        out.push(0x1b);
        if self.is_tmux() {
            out.push(0x1b);
        }
    }

    /// End an envelope opened with [`Passthrough::open`].
    pub fn close(self, out: &mut Vec<u8>) {
        if self.is_tmux() {
            out.extend_from_slice(b"\x1b\\");
        }
    }
}

#[cfg(test)]
#[path = "passthrough.tests.rs"]
mod tests;
