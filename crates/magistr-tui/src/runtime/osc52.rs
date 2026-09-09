//! The terminal's clipboard: OSC 52.

use magistr_cockpit::base64;

/// Put `text` on the reader's clipboard through the terminal — `OSC 52;
/// c; <base64> BEL`, the one clipboard channel a terminal has
/// (xterm's; kitty, ghostty, WezTerm, iTerm2, foot and tmux honour it,
/// some behind a setting). Written beside a frame like an
/// announcement, for [`Cmd::Clipboard`](magistr_cockpit::cockpit::Cmd::Clipboard)
/// — Copy. The payload is base64, so a transcript's
/// escapes and newlines never reach the wire as themselves.
pub(crate) fn clipboard(out: &mut Vec<u8>, text: &str) {
    out.extend_from_slice(b"\x1b]52;c;");
    base64::encode(text.as_bytes(), out);
    out.push(0x07);
}

#[cfg(test)]
#[path = "osc52.tests.rs"]
mod tests;
