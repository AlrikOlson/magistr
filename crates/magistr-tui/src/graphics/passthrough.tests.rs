use super::*;

const ESC: u8 = 0x1b;

#[test]
fn direct_leaves_a_sequence_untouched() {
    let mut out = Vec::new();
    Passthrough::Direct.wrap(b"\x1b_Ga=q;AAAA\x1b\\", &mut out);
    assert_eq!(out, b"\x1b_Ga=q;AAAA\x1b\\");
}

#[test]
fn tmux_wraps_in_dcs_and_doubles_every_escape() {
    let mut out = Vec::new();
    Passthrough::Tmux.wrap(b"\x1b_Ga=q;AAAA\x1b\\", &mut out);
    assert_eq!(out, b"\x1bPtmux;\x1b\x1b_Ga=q;AAAA\x1b\x1b\\\x1b\\");
}

#[test]
fn streaming_open_esc_close_matches_wrap() {
    for mode in [Passthrough::Direct, Passthrough::Tmux] {
        let mut streamed = Vec::new();
        mode.open(&mut streamed);
        mode.esc(&mut streamed);
        streamed.extend_from_slice(b"_Gm=0;QQ==");
        mode.esc(&mut streamed);
        streamed.push(b'\\');
        mode.close(&mut streamed);

        let mut wrapped = Vec::new();
        mode.wrap(b"\x1b_Gm=0;QQ==\x1b\\", &mut wrapped);
        assert_eq!(streamed, wrapped, "{mode:?}");
    }
}

#[test]
fn tmux_wrapping_has_exactly_one_undoubled_escape_at_each_end() {
    let mut out = Vec::new();
    Passthrough::Tmux.wrap(b"\x1b[c", &mut out);
    assert_eq!(out[0], ESC);
    assert_eq!(&out[..7], b"\x1bPtmux;");
    assert_eq!(&out[out.len() - 2..], b"\x1b\\");
    // inner: ESC ESC [ c
    assert_eq!(&out[7..out.len() - 2], b"\x1b\x1b[c");
}

#[test]
fn passthrough_is_derived_from_whether_tmux_is_in_the_way() {
    assert_eq!(Passthrough::for_tmux(true), Passthrough::Tmux);
    assert_eq!(Passthrough::for_tmux(false), Passthrough::Direct);
    assert!(Passthrough::Tmux.is_tmux());
    assert!(!Passthrough::Direct.is_tmux());
}
