use std::io::Read;
use std::os::fd::AsFd;
use std::process::Command;

use rustix::termios::tcgetwinsize;

use super::*;

#[test]
fn the_pair_opens_at_the_size_asked() {
    let pty = Pty::open(100, 30).expect("open");
    let size = tcgetwinsize(pty.slave.as_fd()).expect("winsize");
    assert_eq!((size.ws_col, size.ws_row), (100, 30));
}

#[test]
fn a_child_on_the_slave_sees_a_tty_and_its_bytes_reach_the_master_raw() {
    let pty = Pty::open(80, 24).expect("open");
    let status = Command::new("sh")
        .args(["-c", "test -t 1 && printf '\\033[31mred\\033[0m\\n'"])
        .stdin(Stdio::null())
        .stdout(pty.stdio().expect("stdout"))
        .stderr(pty.stdio().expect("stderr"))
        .status()
        .expect("run");
    assert!(status.success(), "stdout is a tty");

    let (master, _keepalive) = pty.into_parts();
    let mut master = File::from(master);
    let mut out = Vec::new();
    let mut buf = [0u8; 256];
    // Non-blocking: poll until the bytes are in.
    for _ in 0..200 {
        match master.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => out.extend_from_slice(&buf[..n]),
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                if !out.is_empty() {
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
            Err(_) => break,
        }
    }
    assert_eq!(out, b"\x1b[31mred\x1b[0m\r\n");
}
