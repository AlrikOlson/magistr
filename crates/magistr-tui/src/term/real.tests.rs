//! Against a real pty pair: the slave is the cockpit's terminal, the
//! master is the test's hand on it.

use std::os::fd::OwnedFd;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::time::Duration;

use rustix::event::{PollFd, PollFlags, Timespec, poll};
use rustix::fs::{Mode, OFlags};
use rustix::pty::{OpenptFlags, grantpt, openpt, ptsname, unlockpt};
use rustix::termios::{Winsize, tcsetwinsize};

use super::*;
use crate::term::TerminalInput;

const MS: fn(u64) -> Duration = Duration::from_millis;

struct Pty {
    master: OwnedFd,
    slave: OwnedFd,
}

fn pty() -> Pty {
    let master = openpt(OpenptFlags::RDWR | OpenptFlags::NOCTTY).expect("openpt");
    grantpt(&master).expect("grantpt");
    unlockpt(&master).expect("unlockpt");
    let name = ptsname(&master, Vec::new()).expect("ptsname");
    let slave = rustix::fs::open(name, OFlags::RDWR | OFlags::NOCTTY, Mode::empty()).expect("open");
    tcsetwinsize(
        &slave,
        Winsize {
            ws_row: 24,
            ws_col: 80,
            ws_xpixel: 0,
            ws_ypixel: 0,
        },
    )
    .expect("winsize");
    Pty { master, slave }
}

fn open(pty: &Pty) -> RealTerminal {
    let input = rustix::io::dup(&pty.slave).expect("dup");
    let output = rustix::io::dup(&pty.slave).expect("dup");
    RealTerminal::open(input, output).expect("open")
}

#[test]
fn raw_mode_lasts_as_long_as_the_terminal_and_is_restored_after_a_panic() {
    let pty = pty();
    assert!(!RawMode::is_raw(&pty.slave).expect("termios"));
    let terminal = open(&pty);
    assert!(terminal.is_raw().expect("termios"));
    assert!(RawMode::is_raw(&pty.slave).expect("termios"));
    assert!(RawMode::is_raw(terminal.raw_mode_fd()).expect("termios"));
    drop(terminal);
    assert!(!RawMode::is_raw(&pty.slave).expect("termios"));

    let result = catch_unwind(AssertUnwindSafe(|| {
        let _terminal = open(&pty);
        assert!(RawMode::is_raw(&pty.slave).expect("termios"));
        panic!("the event loop blew up");
    }));
    assert!(result.is_err());
    assert!(
        !RawMode::is_raw(&pty.slave).expect("termios"),
        "termios restored while unwinding"
    );
}

#[test]
fn a_write_reaches_the_master_and_the_size_is_the_winsize() {
    let pty = pty();
    let mut terminal = open(&pty);
    terminal.write(b"hello").expect("write");
    let mut buf = [0u8; 16];
    let n = rustix::io::read(&pty.master, &mut buf).expect("read");
    assert_eq!(&buf[..n], b"hello");
    assert_eq!(terminal.size().expect("size"), Size::new(80, 24));
    tcsetwinsize(
        &pty.slave,
        Winsize {
            ws_row: 30,
            ws_col: 100,
            ws_xpixel: 0,
            ws_ypixel: 0,
        },
    )
    .expect("winsize");
    assert_eq!(terminal.size().expect("size"), Size::new(100, 30));
    let input = terminal.input().expect("input");
    assert_eq!(input.size().expect("size"), Size::new(100, 30));
    assert!(terminal.input().is_err(), "the input is taken once");
}

#[test]
fn a_timed_read_returns_none_on_silence_and_the_bytes_typed_at_the_master() {
    let pty = pty();
    let mut terminal = open(&pty);
    let mut input = terminal.input().expect("input");
    let mut buf = [0u8; 16];
    let began = std::time::Instant::now();
    assert_eq!(input.read(&mut buf, MS(20)).expect("read"), None);
    assert!(began.elapsed() >= MS(20));
    rustix::io::write(&pty.master, b"q\x1b[<0;3;4M").expect("write");
    let mut got = Vec::new();
    while got.len() < 10 {
        match input.read(&mut buf, MS(500)).expect("read") {
            Some(0) | None => break,
            Some(n) => got.extend_from_slice(&buf[..n]),
        }
    }
    assert_eq!(got, b"q\x1b[<0;3;4M");
}

#[test]
fn opening_a_non_terminal_is_refused() {
    let (read, write) = rustix::pipe::pipe().expect("pipe");
    let err = RealTerminal::open(read, write).expect_err("not a tty");
    assert_eq!(err.kind(), std::io::ErrorKind::Unsupported);
}

#[test]
fn the_master_going_away_reads_as_closed() {
    let pty = pty();
    let mut terminal = open(&pty);
    let mut input = terminal.input().expect("input");
    drop(pty.master);
    let mut buf = [0u8; 16];
    let mut answer = None;
    for _ in 0..20 {
        match input.read(&mut buf, MS(50)) {
            Ok(None) => continue,
            other => {
                answer = Some(other);
                break;
            }
        }
    }
    assert!(
        matches!(answer, Some(Ok(Some(0))) | Some(Err(_))),
        "expected end of file or an error, got {answer:?}"
    );
}

/// Read whatever the master has within `timeout`; empty on silence.
fn read_master(pty: &Pty, timeout: Duration) -> Vec<u8> {
    let mut got = Vec::new();
    let mut buf = [0u8; 4096];
    loop {
        let mut fds = [PollFd::new(&pty.master, PollFlags::IN)];
        let timespec = Timespec {
            tv_sec: 0,
            tv_nsec: i64::from(timeout.subsec_nanos())
                + i64::try_from(timeout.as_secs()).expect("small") * 1_000_000_000,
        };
        match poll(&mut fds, Some(&timespec)) {
            Ok(0) => return got,
            Ok(_) => match rustix::io::read(&pty.master, &mut buf) {
                Ok(0) | Err(_) => return got,
                Ok(n) => got.extend_from_slice(&buf[..n]),
            },
            Err(_) => return got,
        }
    }
}

/// The test's own stderr, pointed at the pty slave so a replay can be
/// read from the master; put back on drop.
struct StderrOnSlave {
    original: OwnedFd,
}

impl StderrOnSlave {
    fn point_at(pty: &Pty) -> Self {
        let original = rustix::io::dup(rustix::stdio::stderr()).expect("dup");
        rustix::stdio::dup2_stderr(&pty.slave).expect("dup2");
        Self { original }
    }
}

impl Drop for StderrOnSlave {
    fn drop(&mut self) {
        let _ = rustix::stdio::dup2_stderr(&self.original);
    }
}

#[test]
fn stderr_is_held_while_the_terminal_lives_and_replayed_on_the_tty_afterwards() {
    let pty = pty();
    let _stderr = StderrOnSlave::point_at(&pty);
    let mut terminal = open(&pty).own_stderr().expect("own stderr");
    eprintln!("think-and-ship: loaded roadmap with 249 chunk(s) from disk");
    assert!(
        read_master(&pty, MS(50)).is_empty(),
        "nothing printed reaches the tty while it is held"
    );
    terminal.write(b"frame").expect("write");
    assert_eq!(read_master(&pty, MS(200)), b"frame");
    terminal.replay_stray().expect("replay");
    let replayed = read_master(&pty, MS(200));
    assert_eq!(
        String::from_utf8_lossy(&replayed).trim_end(),
        "think-and-ship: loaded roadmap with 249 chunk(s) from disk"
    );
    eprintln!("after the replay");
    assert_eq!(
        String::from_utf8_lossy(&read_master(&pty, MS(200))).trim_end(),
        "after the replay",
        "stderr is the tty again"
    );
    assert!(terminal.is_raw().expect("termios"));
}

#[test]
fn the_restorer_writes_the_bytes_cooks_the_tty_and_hands_stderr_back_from_another_thread() {
    let pty = pty();
    let _stderr = StderrOnSlave::point_at(&pty);
    let terminal = open(&pty).own_stderr().expect("own stderr");
    let restorer = terminal.restorer().expect("restorer");
    eprintln!("a banner while the screen is held");
    assert!(read_master(&pty, MS(50)).is_empty());
    assert!(RawMode::is_raw(&pty.slave).expect("termios"));
    std::thread::spawn(move || restorer(b"LEAVE"))
        .join()
        .expect("restorer thread");
    let got = read_master(&pty, MS(200));
    let text = String::from_utf8_lossy(&got);
    assert!(text.starts_with("LEAVE"), "{text:?}");
    assert!(
        text.contains("a banner while the screen is held"),
        "{text:?}"
    );
    assert!(
        !RawMode::is_raw(&pty.slave).expect("termios"),
        "cooked while the terminal still lives"
    );
    // The terminal's own replay finds nothing left to do.
    let mut terminal = terminal;
    terminal.replay_stray().expect("replay");
    assert!(read_master(&pty, MS(50)).is_empty());
}
