//! Flat prompt editing through a real Unix tty, with the slave's echo
//! disabled and the master acting as the reader's keyboard and screen.

#![cfg(unix)]

use std::os::fd::OwnedFd;
use std::time::{Duration, Instant};

use magistr_application::sessions::{AgentEvent, TurnEnd};
use magistr_cockpit::cockpit::{Cmd, Model, Msg, Size, StageTab, update};
use magistr_tui::graphics::Env;
use magistr_tui::runtime::{Options, Runtime};
use magistr_tui::term::{RawMode, RealTerminal};
use rustix::event::{PollFd, PollFlags, Timespec, poll};
use rustix::fs::{Mode, OFlags};
use rustix::pty::{OpenptFlags, grantpt, openpt, ptsname, unlockpt};
use rustix::termios::{LocalModes, SpecialCodeIndex, Winsize, tcgetattr, tcsetwinsize};

const SIZE: Size = Size::new(80, 24);
const DEADLINE: Duration = Duration::from_secs(3);

struct Pty {
    master: OwnedFd,
    slave: OwnedFd,
    parser: vt100::Parser,
    wire: Vec<u8>,
}

impl Pty {
    fn new() -> Self {
        let master = openpt(OpenptFlags::RDWR | OpenptFlags::NOCTTY).expect("open pty");
        grantpt(&master).expect("grant pty");
        unlockpt(&master).expect("unlock pty");
        let name = ptsname(&master, Vec::new()).expect("pty name");
        let slave = rustix::fs::open(name, OFlags::RDWR | OFlags::NOCTTY, Mode::empty())
            .expect("open pty slave");
        tcsetwinsize(
            &slave,
            Winsize {
                ws_row: SIZE.rows,
                ws_col: SIZE.cols,
                ws_xpixel: 0,
                ws_ypixel: 0,
            },
        )
        .expect("set pty size");
        Self {
            master,
            slave,
            parser: vt100::Parser::new(SIZE.rows, SIZE.cols, 100),
            wire: Vec::new(),
        }
    }

    fn terminal(&self) -> RealTerminal {
        RealTerminal::open(
            rustix::io::dup(&self.slave).expect("duplicate input"),
            rustix::io::dup(&self.slave).expect("duplicate output"),
        )
        .expect("open real terminal")
    }

    fn type_bytes(&self, mut bytes: &[u8]) {
        while !bytes.is_empty() {
            let written = rustix::io::write(&self.master, bytes).expect("type at pty");
            assert!(written > 0, "pty stopped accepting input");
            bytes = &bytes[written..];
        }
    }

    /// Only read bytes already available; the async wait below lets the
    /// real runtime and its input thread advance between observations.
    fn drain(&mut self) {
        let zero = Timespec {
            tv_sec: 0,
            tv_nsec: 0,
        };
        let mut buf = [0_u8; 4096];
        for _ in 0..64 {
            let mut fds = [PollFd::new(&self.master, PollFlags::IN)];
            if poll(&mut fds, Some(&zero)).expect("poll pty") == 0 {
                return;
            }
            let read = rustix::io::read(&self.master, &mut buf).expect("read pty");
            assert!(read > 0, "pty output closed");
            self.parser.process(&buf[..read]);
            self.wire.extend_from_slice(&buf[..read]);
        }
    }

    async fn shows(&mut self, line: &str) {
        let deadline = Instant::now() + DEADLINE;
        loop {
            self.drain();
            let screen = self.parser.screen().contents();
            let found = if line.starts_with("input") {
                screen.lines().next_back().map(str::trim_end) == Some(line)
            } else {
                screen.lines().any(|shown| shown == line)
            };
            if found {
                return;
            }
            assert!(
                Instant::now() < deadline,
                "the live tty never showed {line:?}; screen:\n{screen}\nwire: {:?}",
                String::from_utf8_lossy(&self.wire)
            );
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    }
}

#[tokio::test]
async fn flat_tty_echoes_edits_before_submit_and_restores_the_terminal() {
    tokio::task::LocalSet::new()
        .run_until(exercise_flat_tty())
        .await;
}

async fn exercise_flat_tty() {
    let mut pty = Pty::new();
    let saved = tcgetattr(&pty.slave).expect("original termios");
    let terminal = pty.terminal();
    assert!(RawMode::is_raw(&pty.slave).expect("raw mode"));

    let mut model = Model::new(SIZE)
        .with_tab(StageTab::Turn)
        .with_title("flat pty");
    for event in [
        AgentEvent::Initialized {
            session_id: "pty-local".to_owned(),
            model: None,
        },
        AgentEvent::Text("already settled".to_owned()),
        AgentEvent::TurnEnded(TurnEnd::EndTurn),
    ] {
        update(&mut model, Msg::Agent(event));
    }
    let mut runtime = Runtime::new(
        terminal,
        Options::new(Env::new("xterm", false, None).with_screen_reader(true))
            .probe_timeout(Duration::ZERO),
    );
    let inbox = runtime.inbox();
    let mut commands = runtime.commands().expect("command receiver");
    let running = tokio::task::spawn_local(runtime.run(model));
    pty.shows("agent: already settled").await;

    // No Enter is sent until every edit has been observed. Since the
    // slave is raw, these bytes can only have come from the renderer.
    for (typed, shown) in [
        ("c", "input: c"),
        ("a", "input: ca"),
        ("f", "input: caf"),
        ("e", "input: cafe"),
        ("\u{301}", "input: cafe\u{301}"),
        ("\x7f", "input: caf"),
    ] {
        pty.type_bytes(typed.as_bytes());
        pty.shows(shown).await;
    }
    pty.type_bytes("\x1b[200~e\u{301} 界\nsecond\x1b[201~".as_bytes());
    pty.shows("input 2/2: second").await;
    pty.type_bytes(b"\x1b[H\x1b[3~");
    pty.shows("input 2/2: econd").await;
    pty.type_bytes(b"\x1b[A");
    pty.shows("input 1/2: cafe\u{301} 界").await;
    pty.type_bytes(b"\x1b[B\x1b[F");
    pty.shows("input 2/2: econd").await;
    while let Some(command) = commands.try_next() {
        assert!(!matches!(command, Cmd::Prompt(_)), "submitted before Enter");
    }

    pty.type_bytes(b"\r");
    let submitted = tokio::time::timeout(DEADLINE, async {
        loop {
            let command = commands
                .next()
                .await
                .expect("command channel stays open until submission");
            if let Cmd::Prompt(text) = command {
                break text;
            }
        }
    })
    .await
    .expect("Enter sends the edited prompt");
    assert_eq!(submitted, "cafe\u{301} 界\necond");
    inbox.send(Msg::Agent(AgentEvent::Text("accepted locally".to_owned())));
    inbox.send(Msg::Agent(AgentEvent::TurnEnded(TurnEnd::EndTurn)));
    pty.shows("agent: accepted locally").await;

    pty.type_bytes(b"draft");
    pty.shows("input: draft").await;
    pty.type_bytes(b"\x1b[A");
    pty.shows("input 2/2: econd").await;
    pty.type_bytes(b"\x1b[B");
    pty.shows("input: draft").await;
    pty.type_bytes(b"\x15\x04");
    let model = tokio::time::timeout(DEADLINE, running)
        .await
        .expect("bounded runtime shutdown")
        .expect("runtime task")
        .expect("runtime succeeds");
    pty.drain();
    assert!(model.quit_requested());
    assert!(model.prompt().is_empty());
    assert_eq!(model.prompt().history(), [submitted]);
    let restored = tcgetattr(&pty.slave).expect("restored termios");
    assert_eq!(restored.input_modes, saved.input_modes);
    assert_eq!(restored.output_modes, saved.output_modes);
    assert_eq!(restored.control_modes, saved.control_modes);
    // BSD may set PENDIN when switching the line discipline back to
    // canonical mode. It is pending kernel work, not an unrestored mode.
    assert_eq!(
        restored.local_modes.difference(LocalModes::PENDIN),
        saved.local_modes.difference(LocalModes::PENDIN)
    );
    for index in [SpecialCodeIndex::VMIN, SpecialCodeIndex::VTIME] {
        assert_eq!(restored.special_codes[index], saved.special_codes[index]);
    }
    assert_eq!(restored.input_speed(), saved.input_speed());
    assert_eq!(restored.output_speed(), saved.output_speed());

    let wire = String::from_utf8(pty.wire).expect("UTF-8 terminal output");
    for settled in [
        "session: flat pty (flat mode)\r\n",
        "agent: already settled\r\n",
        "you: cafe\u{301} 界\r\n",
        "you: econd\r\n",
        "agent: accepted locally\r\n",
    ] {
        assert_eq!(wire.matches(settled).count(), 1, "replayed {settled:?}");
    }
    for takeover in ["\x1b[?1049h", "\x1b[?25l", "\x1b[2J", "\x1b[?2026h"] {
        assert!(!wire.contains(takeover), "flat tty emitted {takeover:?}");
    }
}
