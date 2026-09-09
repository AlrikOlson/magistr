//! The one thread that reads the terminal.

use std::io;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;
use std::time::Duration;

use tokio::sync::mpsc::UnboundedSender;

use super::input::TerminalInput;
use super::input_event::InputEvent;
use magistr_cockpit::cockpit::Size;
use magistr_cockpit::input::Decoder;

/// A thread that reads a [`TerminalInput`] with a short timeout, decodes
/// what arrives and sends [`InputEvent`]s down a channel.
///
/// On every quiet read it resolves a lone Escape ([`Decoder::flush`]) and
/// polls the window size, sending [`InputEvent::Resize`] when it changed —
/// so a resize costs no signal handler and reaches the runtime within one
/// [`InputThread::READ_TIMEOUT`]. A closed input, a read error or a
/// dropped receiver ends the thread; [`InputThread::stop`] (and `Drop`)
/// raise the stop flag and join it, which takes at most one read timeout.
#[derive(Debug)]
pub struct InputThread {
    stop: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl InputThread {
    /// How long one read waits before the thread checks the stop flag,
    /// flushes the decoder and polls the size.
    pub const READ_TIMEOUT: Duration = Duration::from_millis(50);

    /// Start reading `input` with `decoder` (which may already hold bytes
    /// from the startup window); `size` is what the runtime knows, so only
    /// a change from it is reported.
    ///
    /// # Errors
    ///
    /// The thread could not be spawned.
    pub fn spawn<I: TerminalInput>(
        mut input: I,
        mut decoder: Decoder,
        size: Size,
        tx: UnboundedSender<InputEvent>,
    ) -> io::Result<Self> {
        let stop = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&stop);
        let handle = std::thread::Builder::new()
            .name("magistr-tui-input".to_owned())
            .spawn(move || run(&mut input, &mut decoder, size, &tx, &flag))?;
        Ok(Self {
            stop,
            handle: Some(handle),
        })
    }

    /// Whether the thread has exited (closed input, error, or stopped).
    #[must_use]
    pub fn is_finished(&self) -> bool {
        self.handle.as_ref().is_none_or(JoinHandle::is_finished)
    }

    /// Raise the stop flag and wait for the thread to exit.
    pub fn stop(mut self) {
        self.join();
    }

    fn join(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(handle) = self.handle.take() {
            // A panic on the input thread has already been reported by the
            // panic hook; the runtime is shutting down either way.
            let _ = handle.join();
        }
    }
}

impl Drop for InputThread {
    fn drop(&mut self) {
        self.join();
    }
}

/// The thread body: read, decode, send; on a quiet read flush and check
/// the size; leave when told to, when the input ends, or when nobody
/// listens any more.
fn run<I: TerminalInput>(
    input: &mut I,
    decoder: &mut Decoder,
    mut size: Size,
    tx: &UnboundedSender<InputEvent>,
    stop: &AtomicBool,
) {
    let mut buf = [0u8; 4096];
    let mut events = Vec::new();
    while !stop.load(Ordering::SeqCst) {
        match input.read(&mut buf, InputThread::READ_TIMEOUT) {
            Ok(Some(0)) | Err(_) => {
                let _ = tx.send(InputEvent::Closed);
                return;
            }
            Ok(Some(n)) => decoder.feed(&buf[..n], &mut events),
            Ok(None) => {
                decoder.flush(&mut events);
                if let Ok(now) = input.size()
                    && now != size
                {
                    size = now;
                    if tx.send(InputEvent::Resize(now)).is_err() {
                        return;
                    }
                }
            }
        }
        for event in events.drain(..) {
            if tx.send(InputEvent::Event(event)).is_err() {
                return;
            }
        }
    }
}

#[cfg(test)]
#[path = "input_thread.tests.rs"]
mod tests;
