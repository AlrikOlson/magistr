//! Taking the terminal over, running the loop, handing it back.

use std::time::{Duration, Instant};

use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};

use super::driver::Driver;
use super::error::RuntimeError;
use super::options::Options;
use crate::graphics::{Decision, Probe};
use crate::input::{Decoder, Event};
use crate::pixel::{Compression, IdSpace};
use crate::render::Screen;
use crate::term::{InputEvent, InputThread, PanicHook, Session, Terminal, TerminalInput};
use magistr_cockpit::cockpit::{Cmd, Model, Msg, update};
use magistr_cockpit_runtime::channel::{Commands, Inbox};

/// The cockpit's driver over a [`Terminal`].
///
/// A session whose environment asks for a screen reader
/// ([`Env::screen_reader`](crate::graphics::Env::screen_reader)) runs
/// flat instead: [`Session::flat`] leaves the terminal's screen alone
/// and every frame appends the lines that settled ([`crate::flat`]).
///
/// [`Runtime::run`] is otherwise the whole session: enter (alternate screen, mouse,
/// paste, focus, kitty keyboard), the startup queries — graphics probe,
/// OSC 11, keyboard flags, cell size — answered in one window before
/// anything else is read, then the loop until `Cmd::Quit` or the input
/// closes, then leave in reverse order. The model comes back out so a
/// caller (a test, the example) can look at what the session did.
///
/// Producers — jobs, the engine feed, the agent session — post on the
/// [`Inbox`] handed out by [`Runtime::inbox`] before the run; their
/// messages join the loop's one merged stream. The session also takes
/// [`Runtime::commands`], where every command the core answers with
/// (prompts, cancels, permission answers) arrives.
///
/// The terminal is the cockpit's alone while the run lasts: a
/// [`PanicHook`] armed with the terminal's restorer leaves the screen and
/// cooks the tty before any panic message prints, and whatever the
/// terminal collected on stderr meanwhile is replayed once the leave
/// sequence is out ([`Terminal::replay_stray`]).
#[derive(Debug)]
pub struct Runtime<T: Terminal> {
    terminal: T,
    options: Options,
    inbox: Inbox,
    messages: UnboundedReceiver<Msg>,
    commands_tx: UnboundedSender<Cmd>,
    commands: Option<Commands>,
}

impl<T: Terminal> Runtime<T> {
    /// A runtime over `terminal`.
    pub fn new(terminal: T, options: Options) -> Self {
        let (inbox, messages) = Inbox::channel();
        let (commands_tx, commands) = Commands::channel();
        Self {
            terminal,
            options,
            inbox,
            messages,
            commands_tx,
            commands: Some(commands),
        }
    }

    /// The inbox producers post on; clone it out before [`Runtime::run`].
    #[must_use]
    pub fn inbox(&self) -> Inbox {
        self.inbox.clone()
    }

    /// The core's commands — `Some` on the first call; whoever takes it
    /// acts on every prompt, cancel and permission answer.
    pub fn commands(&mut self) -> Option<Commands> {
        self.commands.take()
    }

    /// Run `model` on the terminal until it quits; returns the model as it
    /// was at the end.
    ///
    /// # Errors
    ///
    /// The terminal failed, or the view produced a tree the layout engine
    /// rejected.
    pub async fn run(self, mut model: Model) -> Result<Model, RuntimeError> {
        let Self {
            mut terminal,
            options,
            inbox,
            mut messages,
            commands_tx,
            commands,
        } = self;
        // An untaken receiver is dropped here: the driver's sends fail
        // quietly instead of queueing for nobody.
        drop(commands);
        // A screen-reader session renders flat ([`crate::flat`]): it
        // never takes the alternate screen, so the transcript stays in
        // the terminal's own scrollback where a reader can reach it.
        let flat = options.env().screen_reader();
        let session = if flat {
            Session::flat()
        } else {
            Session::cockpit()
        }
        .with_progress(options.env().shows_progress());
        let size = terminal.size()?;
        update(&mut model, Msg::Resize(size));
        let mut input = terminal.input()?;

        let hook = PanicHook::install(session, terminal.restorer()?);
        let mut out = Vec::new();
        session.enter(&mut out);
        let mut probe = Probe::new(options.env().passthrough());
        session.queries(&mut out);
        // DA1 is the probe's final request: its reply closes the startup window
        // only after the terminal has answered the size and background queries.
        probe.request(&mut out);
        terminal.write(&out)?;

        let mut decoder = Decoder::new();
        let mut events = Vec::new();
        startup_window(
            &mut input,
            &mut probe,
            &mut decoder,
            &mut events,
            options.probe_timeout_duration(),
        )?;
        let mut screen = Screen::new(size.cols, size.rows);
        if options.env().styled_underlines() {
            screen.enable_underline_styles();
        }
        let decision = Decision::for_terminal(options.env(), &probe.reply());
        update(&mut model, Msg::Mode(decision.mode()));
        update(&mut model, Msg::TextSizing(probe.reply().text_sizing));
        if let Decision::Graphics(protocol) = decision {
            // This process's own id space: a second cockpit behind the
            // same terminal image store must never transmit over ours.
            screen.enable_graphics(protocol, Compression::DEFAULT, IdSpace::for_process());
        }

        let pixel_mouse = matches!(decision, Decision::Graphics(_)) && probe.reply().pixel_mouse;
        if pixel_mouse {
            terminal.write(b"\x1b[?1016h")?;
        }
        let (tx, mut rx) = unbounded_channel();
        let thread = InputThread::spawn_with_mouse(input, decoder, size, tx, pixel_mouse)?;
        let mut driver = Driver::new(
            model,
            screen,
            commands_tx,
            options.panic_frame(),
            session.shows_progress(),
            flat,
        );
        for event in events {
            driver.event(InputEvent::Event(event));
        }
        let result = driver.run(&mut terminal, &mut rx, &mut messages).await;
        // The runtime's own sender kept the inbox open for the whole run;
        // producers learn the loop is gone from here on.
        drop(inbox);
        drop(messages);

        out.clear();
        driver.finish_flat(&mut out);
        driver.release_pointer(&mut out);
        driver.screen.release(&mut out);
        session.leave(&mut out);
        let left = terminal.write(&out);
        thread.stop();
        let replayed = terminal.replay_stray();
        drop(terminal);
        drop(hook);
        result?;
        left?;
        replayed?;
        Ok(driver.model)
    }
}

/// Read until the probe is complete (DA1 answered) or `timeout` passes,
/// feeding every byte to the probe and the decoder; the decoder's events
/// — replies, and any keys typed meanwhile — go to `events`.
fn startup_window<I: TerminalInput>(
    input: &mut I,
    probe: &mut Probe,
    decoder: &mut Decoder,
    events: &mut Vec<Event>,
    timeout: Duration,
) -> std::io::Result<()> {
    let deadline = Instant::now() + timeout;
    let mut buf = [0u8; 4096];
    while !probe.reply().complete {
        let now = Instant::now();
        if now >= deadline {
            break;
        }
        match input.read(&mut buf, deadline - now)? {
            Some(0) | None => break,
            Some(n) => {
                probe.feed(&buf[..n]);
                decoder.feed(&buf[..n], events);
            }
        }
    }
    Ok(())
}
