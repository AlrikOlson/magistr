use super::*;

#[test]
fn chunks_land_in_order_by_id() {
    let mut t = Terminals::default();
    t.push("a", b"\x1b[31mred", 1);
    t.push("b", b"other", 1);
    t.push("a", b"\x1b[0m\r\n", 2);
    assert_eq!(t.bytes("a"), Some(&b"\x1b[31mred\x1b[0m\r\n"[..]));
    assert_eq!(t.bytes("b"), Some(&b"other"[..]));
    assert_eq!(t.bytes("c"), None);
    assert_eq!(
        t.term("a").map(|term| term.text_rows()[0].clone()),
        Some("red".to_owned())
    );
    assert!(t.term("c").is_none());
}

#[test]
fn the_oldest_bytes_go_past_the_cap() {
    let mut t = Terminals::default();
    t.push("a", &vec![b'x'; Term::KEEP], 1);
    t.push("a", b"tail", 2);
    let bytes = t.bytes("a").expect("kept");
    assert_eq!(bytes.len(), Term::KEEP);
    assert!(bytes.ends_with(b"tail"));
}

#[test]
fn finishing_freezes_a_known_terminal_and_creates_no_unknown_one() {
    let mut t = Terminals::default();
    t.push("a", b"x", 1);
    t.finish("a");
    t.finish("never");
    assert!(t.term("a").is_some_and(Term::is_finished));
    assert!(t.term("never").is_none());
}

#[test]
fn follow_all_returns_every_scrolled_terminal_to_its_tail() {
    let mut t = Terminals::default();
    let many: Vec<u8> = (0..40)
        .map(|i| format!("{i}\r\n"))
        .collect::<String>()
        .into_bytes();
    t.push("a", &many, 1);
    t.push("b", &many, 1);
    assert!(!t.follow_all(), "nothing scrolled");
    t.term_mut("a").expect("a").scroll_up(4);
    assert!(t.follow_all());
    assert!(t.term("a").is_some_and(Term::is_following));
    assert!(t.term_mut("zz").is_none());
}

#[test]
fn killing_marks_a_known_terminal_and_reports_an_unknown_one() {
    let mut t = Terminals::default();
    t.push("a", b"x", 1);
    assert!(t.kill("a"));
    assert!(t.term("a").is_some_and(Term::is_killed));
    assert!(!t.kill("b"), "nothing to kill");
    assert!(t.term("b").is_none(), "and nothing was created");
}
