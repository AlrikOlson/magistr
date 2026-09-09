use super::*;

fn walk(bytes: &[u8]) -> Option<(usize, &[u8])> {
    next_sequence(bytes)
}

#[test]
fn no_escape_is_none() {
    assert_eq!(walk(b"hello"), None);
    assert_eq!(walk(b""), None);
}

#[test]
fn leading_bytes_are_skipped_and_counted() {
    assert_eq!(walk(b"ab\x1b[Acd"), Some((5, &b"\x1b[A"[..])));
}

#[test]
fn csi_runs_to_its_final_byte() {
    assert_eq!(walk(b"\x1b[1;5A"), Some((6, &b"\x1b[1;5A"[..])));
    assert_eq!(
        walk(b"\x1b[<0;10;5M rest"),
        Some((10, &b"\x1b[<0;10;5M"[..]))
    );
    assert_eq!(walk(b"\x1b[?62;4c"), Some((8, &b"\x1b[?62;4c"[..])));
}

#[test]
fn incomplete_csi_is_none() {
    assert_eq!(walk(b"\x1b["), None);
    assert_eq!(walk(b"\x1b[1;5"), None);
}

#[test]
fn apc_dcs_and_osc_run_to_st_or_bel() {
    assert_eq!(
        walk(b"\x1b_Gi=31;OK\x1b\\x"),
        Some((12, &b"\x1b_Gi=31;OK\x1b\\"[..]))
    );
    assert_eq!(
        walk(b"\x1b]11;rgb:1/2/3\x07"),
        Some((15, &b"\x1b]11;rgb:1/2/3\x07"[..]))
    );
    assert_eq!(
        walk(b"\x1bPtmux;x\x1b\\"),
        Some((10, &b"\x1bPtmux;x\x1b\\"[..]))
    );
    assert_eq!(walk(b"\x1b]11;rgb:1/2/3"), None);
}

#[test]
fn ss3_is_three_bytes() {
    assert_eq!(walk(b"\x1bOP"), Some((3, &b"\x1bOP"[..])));
    assert_eq!(walk(b"\x1bO"), None);
}

#[test]
fn alt_prefix_takes_one_scalar() {
    assert_eq!(walk(b"\x1ba"), Some((2, &b"\x1ba"[..])));
    assert_eq!(walk("\x1bé".as_bytes()), Some((3, "\x1bé".as_bytes())));
    assert_eq!(walk("\x1b日".as_bytes()), Some((4, "\x1b日".as_bytes())));
    assert_eq!(walk(&b"\x1b\xc3"[..]), None);
    assert_eq!(walk(b"\x1b\x1b"), Some((2, &b"\x1b\x1b"[..])));
}

#[test]
fn a_lone_trailing_escape_is_incomplete() {
    assert_eq!(walk(b"\x1b"), None);
    assert_eq!(walk(b"abc\x1b"), None);
}
