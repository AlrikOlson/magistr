use std::io::Write as _;

use super::*;

fn inode_of_stderr() -> u64 {
    rustix::fs::fstat(rustix::stdio::stderr())
        .expect("fstat")
        .st_ino
}

#[test]
fn what_is_written_to_stderr_while_owned_comes_back_on_restore_and_the_descriptor_is_put_back() {
    let before = inode_of_stderr();
    let owned = OwnedStderr::take().expect("take");
    assert_ne!(inode_of_stderr(), before, "fd 2 points at the sink");
    eprintln!("think-and-ship: loaded roadmap with 249 chunk(s) from disk");
    std::io::stderr()
        .write_all(b"a raw write\n")
        .expect("write");
    let bytes = owned.restore().expect("restore");
    assert_eq!(inode_of_stderr(), before, "fd 2 is back");
    let text = String::from_utf8(bytes).expect("utf-8");
    assert_eq!(
        text,
        "think-and-ship: loaded roadmap with 249 chunk(s) from disk\na raw write\n"
    );
}

#[test]
fn dropping_without_restoring_still_puts_the_descriptor_back() {
    let before = inode_of_stderr();
    let owned = OwnedStderr::take().expect("take");
    assert_ne!(inode_of_stderr(), before);
    drop(owned);
    assert_eq!(inode_of_stderr(), before);
}

#[test]
fn nothing_is_left_in_the_temp_dir() {
    let owned = OwnedStderr::take().expect("take");
    let leftovers = std::fs::read_dir(std::env::temp_dir())
        .expect("temp dir")
        .flatten()
        .filter(|e| {
            e.file_name()
                .to_string_lossy()
                .starts_with(&format!("magistr-stderr-{}-", std::process::id()))
        })
        .count();
    drop(owned);
    assert_eq!(leftovers, 0, "the sink is unlinked as soon as it is open");
}

#[test]
fn the_replay_keeps_the_tail_from_a_line_boundary() {
    let short = b"short\n".to_vec();
    assert_eq!(tail(short.clone()), short);
    let line = "0123456789abcde\n";
    let long: Vec<u8> = line.repeat(REPLAY_LIMIT / line.len() + 10).into_bytes();
    let kept = tail(long);
    assert!(kept.len() <= REPLAY_LIMIT);
    assert!(kept.starts_with(line.as_bytes()), "starts on a line");
    assert!(kept.ends_with(line.as_bytes()));
}
