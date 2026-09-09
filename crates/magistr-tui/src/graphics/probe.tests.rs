use super::*;

const KITTY_OK: &[u8] = b"\x1b_Gi=31;OK\x1b\\";
const DA1_SIXEL: &[u8] = b"\x1b[?62;4;6;7;22c";
const DA1_PLAIN: &[u8] = b"\x1b[?1;2c";
const OSC11: &[u8] = b"\x1b]11;rgb:1a1a/1b1b/2626\x1b\\";

#[test]
fn the_request_is_the_kitty_query_the_sizing_handshake_then_da1() {
    let mut out = Vec::new();
    Probe::new(Passthrough::Direct).request(&mut out);
    assert_eq!(
        out,
        b"\x1b_Gi=31,s=1,v=1,a=q,t=d,f=24;AAAA\x1b\\\
          \r\x1b[6n\x1b]66;w=2; \x07\x1b[6n\x1b]66;s=2; \x07\x1b[6n\r\x1b[c"
    );
}

#[test]
fn under_tmux_only_the_kitty_query_is_wrapped() {
    let mut out = Vec::new();
    Probe::new(Passthrough::Tmux).request(&mut out);
    assert!(out.starts_with(b"\x1bPtmux;\x1b\x1b_G"));
    assert!(
        out.ends_with(b"\x1b[6n\r\x1b[c"),
        "{:?}",
        String::from_utf8_lossy(&out)
    );
    // The sizing runs reach tmux itself: it has no scaled cell, and the
    // cursor reports it answers out of its own grid are the honest "no".
    let sizing = b"\x1b]66;s=2; \x07";
    assert!(
        out.windows(sizing.len()).any(|w| w == sizing),
        "the OSC 66 run is unwrapped"
    );
}

/// The handshake as kitty prescribes it, answered by a terminal that
/// renders both runs: column 1, then 3, then 5.
const SIZING_OK: &[u8] = b"\x1b[1;1R\x1b[1;3R\x1b[1;5R";
/// The same handshake on a terminal that swallowed both OSCs.
const SIZING_NONE: &[u8] = b"\x1b[1;1R\x1b[1;1R\x1b[1;1R";

#[test]
fn two_advances_of_two_cells_are_text_sizing() {
    let mut probe = Probe::new(Passthrough::Direct);
    probe.feed(SIZING_OK);
    assert!(probe.reply().text_sizing);
    assert!(probe.feed(DA1_PLAIN));
    assert!(probe.reply().text_sizing);
}

#[test]
fn a_terminal_that_swallowed_the_runs_has_no_text_sizing() {
    let mut probe = Probe::new(Passthrough::Direct);
    probe.feed(SIZING_NONE);
    probe.feed(DA1_PLAIN);
    assert!(!probe.reply().text_sizing);
}

#[test]
fn an_explicit_width_alone_is_not_a_scale() {
    // The second run advanced (`w=2` honoured) but the third did not:
    // the terminal sizes a run without scaling it, which is not enough
    // for a headline.
    let mut probe = Probe::new(Passthrough::Direct);
    probe.feed(b"\x1b[1;1R\x1b[1;3R\x1b[1;3R");
    probe.feed(DA1_PLAIN);
    assert!(!probe.reply().text_sizing);
}

#[test]
fn the_handshake_survives_being_split_and_interleaved() {
    let mut probe = Probe::new(Passthrough::Direct);
    let mut stream = Vec::new();
    stream.extend_from_slice(b"\x1b[1;1R");
    stream.extend_from_slice(OSC11);
    stream.extend_from_slice(b"\x1b[1;3R");
    stream.extend_from_slice(KITTY_OK);
    stream.extend_from_slice(b"\x1b[1;5R");
    stream.extend_from_slice(DA1_PLAIN);
    for byte in &stream {
        probe.feed(std::slice::from_ref(byte));
    }
    assert_eq!(
        probe.reply(),
        ProbeReply {
            kitty: true,
            sixel: false,
            text_sizing: true,
            complete: true
        }
    );
}

#[test]
fn a_fourth_cursor_report_cannot_change_the_answer() {
    // Whatever else asks the terminal where the cursor is later in the
    // startup window, the handshake is the first three reports.
    let mut probe = Probe::new(Passthrough::Direct);
    probe.feed(SIZING_OK);
    probe.feed(b"\x1b[9;9R");
    probe.feed(DA1_PLAIN);
    assert!(probe.reply().text_sizing);
}

#[test]
fn kitty_ok_then_da1_is_complete_with_kitty() {
    let mut probe = Probe::new(Passthrough::Direct);
    assert!(!probe.feed(KITTY_OK));
    assert_eq!(
        probe.reply(),
        ProbeReply {
            kitty: true,
            sixel: false,
            text_sizing: false,
            complete: false
        }
    );
    assert!(probe.feed(DA1_PLAIN));
    assert_eq!(
        probe.reply(),
        ProbeReply {
            kitty: true,
            sixel: false,
            text_sizing: false,
            complete: true
        }
    );
}

#[test]
fn da1_with_parameter_4_is_sixel() {
    let mut probe = Probe::new(Passthrough::Direct);
    assert!(probe.feed(DA1_SIXEL));
    assert_eq!(
        probe.reply(),
        ProbeReply {
            kitty: false,
            sixel: true,
            text_sizing: false,
            complete: true
        }
    );
}

#[test]
fn parameter_4_must_be_a_whole_parameter() {
    let mut probe = Probe::new(Passthrough::Direct);
    assert!(probe.feed(b"\x1b[?64;14;42c"));
    assert!(!probe.reply().sixel);
}

#[test]
fn a_kitty_error_reply_is_not_support() {
    let mut probe = Probe::new(Passthrough::Direct);
    probe.feed(b"\x1b_Gi=31;EINVAL:Unknown format\x1b\\");
    probe.feed(DA1_PLAIN);
    assert_eq!(
        probe.reply(),
        ProbeReply {
            kitty: false,
            sixel: false,
            text_sizing: false,
            complete: true
        }
    );
}

#[test]
fn a_reply_for_another_image_id_is_ignored() {
    let mut probe = Probe::new(Passthrough::Direct);
    probe.feed(b"\x1b_Gi=7;OK\x1b\\");
    assert!(!probe.reply().kitty);
}

#[test]
fn extra_keys_in_the_reply_are_tolerated() {
    let mut probe = Probe::new(Passthrough::Direct);
    probe.feed(b"\x1b_Gi=31,I=0;OK\x1b\\");
    assert!(probe.reply().kitty);
}

#[test]
fn an_interleaved_osc_11_reply_is_skipped() {
    let mut probe = Probe::new(Passthrough::Direct);
    let mut stream = Vec::new();
    stream.extend_from_slice(OSC11);
    stream.extend_from_slice(KITTY_OK);
    stream.extend_from_slice(b"\x1b]11;rgb:0000/0000/0000\x07");
    stream.extend_from_slice(DA1_SIXEL);
    assert!(probe.feed(&stream));
    assert_eq!(
        probe.reply(),
        ProbeReply {
            kitty: true,
            sixel: true,
            text_sizing: false,
            complete: true
        }
    );
}

#[test]
fn other_csi_replies_do_not_complete_the_probe() {
    let mut probe = Probe::new(Passthrough::Direct);
    // DECRPM for 2026, a cursor position report, then DA1.
    assert!(!probe.feed(b"\x1b[?2026;2$y\x1b[24;80R"));
    assert!(!probe.reply().complete);
    assert!(probe.feed(DA1_PLAIN));
}

#[test]
fn bytes_may_arrive_one_at_a_time() {
    let mut probe = Probe::new(Passthrough::Direct);
    let mut stream = Vec::new();
    stream.extend_from_slice(KITTY_OK);
    stream.extend_from_slice(DA1_SIXEL);
    let last = stream.len() - 1;
    for (i, byte) in stream.iter().enumerate() {
        let complete = probe.feed(std::slice::from_ref(byte));
        assert_eq!(complete, i == last, "byte {i}");
    }
    assert_eq!(
        probe.reply(),
        ProbeReply {
            kitty: true,
            sixel: true,
            text_sizing: false,
            complete: true
        }
    );
}

#[test]
fn stray_text_and_a_lone_escape_are_harmless() {
    let mut probe = Probe::new(Passthrough::Direct);
    assert!(!probe.feed(b"hello\x1b"));
    assert!(!probe.feed(b"OK"));
    assert!(probe.feed(DA1_PLAIN));
    assert_eq!(
        probe.reply(),
        ProbeReply {
            kitty: false,
            sixel: false,
            text_sizing: false,
            complete: true
        }
    );
}

#[test]
fn nothing_fed_is_nothing_found() {
    let mut probe = Probe::new(Passthrough::Tmux);
    assert!(!probe.feed(b""));
    assert_eq!(probe.reply(), ProbeReply::default());
}
