//! The event loop proper: fold events, paint on the tick.

use std::time::Duration;

use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tokio::time::{Instant, Interval, MissedTickBehavior};

use super::activity::Activity;
use super::error::RuntimeError;
use super::osc52;
use super::scheduler::FrameScheduler;
use super::surfaces::Surfaces;
use crate::flat::echo::Echo;
use crate::flat::{FlatLine, Role, Transcript, plain};
use crate::render::Screen;
use crate::term::{InputEvent, Terminal};
use magistr_cockpit::cockpit::{
    Animation, Cmd, FlightRecorder, FrameClock, Model, Msg, PointerShape, Progress, Widget, update,
    view,
};
use magistr_cockpit::hit::HitRegistry;
use magistr_cockpit::input::Event;
use magistr_cockpit::layout::{LayoutTree, Resolved};
use magistr_cockpit::render::paint;

/// Everything the loop owns between the startup window and the shutdown.
pub(crate) struct Driver {
    pub(crate) model: Model,
    pub(crate) screen: Screen,
    tree: LayoutTree,
    hits: HitRegistry,
    geometry_dirty: bool,
    scheduler: FrameScheduler,
    started: Instant,
    out: Vec<u8>,
    quit: bool,
    commands: UnboundedSender<Cmd>,
    panic_on_frame: Option<u64>,
    /// Every pixel surface this frame shows, and what each was painted
    /// from — the same unit `screenshot::frame` drives under a graphics
    /// profile, so a golden pins the pixels the runtime really places.
    surfaces: Surfaces,
    /// Announcements queued for the screen-reader channel, written as
    /// OSC 9 notifications beside the next frame.
    announcements: Vec<String>,
    /// Text for the reader's clipboard, written as OSC 52 beside the
    /// next frame — the runtime's own, like an
    /// announcement.
    clipboard: Vec<String>,
    /// The mouse-pointer shape the terminal was last asked for, so an
    /// OSC 22 goes out on a change of shape and never per frame.
    pointer: PointerShape,
    /// Whether this terminal shows an OSC 9;4 progress bar at all.
    reports_progress: bool,
    /// The progress the terminal was last told, so an OSC 9;4 goes out on
    /// a change of state and never per frame.
    progress: Progress,
    /// How far the flat cockpit has been written out — `Some` only in a
    /// screen-reader session ([`crate::flat`]), where a frame appends
    /// the lines that settled instead of presenting a cell grid.
    flat: Option<Transcript>,
    /// Live prompt bytes are queued per edit, never collapsed to a frame.
    echo: Echo,
}

impl Driver {
    /// A driver over `model` and `screen`; every command but `Quit` is
    /// forwarded on `commands` (and dropped when nobody listens).
    /// `panic_on_frame` is the harness's fault injection
    /// ([`Options::panic_on_frame`](super::Options::panic_on_frame)).
    pub(crate) fn new(
        model: Model,
        screen: Screen,
        commands: UnboundedSender<Cmd>,
        panic_on_frame: Option<u64>,
        reports_progress: bool,
        flat: bool,
    ) -> Self {
        let mut driver = Self {
            model,
            screen,
            tree: LayoutTree::new(),
            hits: HitRegistry::new(),
            geometry_dirty: true,
            scheduler: FrameScheduler::new(),
            started: Instant::now(),
            out: Vec::new(),
            quit: false,
            commands,
            panic_on_frame,
            surfaces: Surfaces::default(),
            announcements: Vec::new(),
            clipboard: Vec::new(),
            pointer: PointerShape::Default,
            reports_progress,
            progress: Progress::Cleared,
            flat: flat.then(Transcript::new),
            echo: Echo::default(),
        };
        if let Some(period) = driver.model.animations().period() {
            driver.scheduler.animate(period, Instant::now());
        }
        if flat {
            // Queue the session header before any input saved by the probe.
            driver.compose_flat();
        }
        driver
    }

    /// Fold one event from the input thread into the model.
    pub(crate) fn event(&mut self, event: InputEvent) {
        match event {
            InputEvent::Event(Event::Key(key)) => self.apply(Msg::Key(key), Activity::Input),
            InputEvent::Event(Event::Mouse(mut event)) => {
                self.refresh_hits();
                let target = if self.model.has_pixels() {
                    let cell = self.model.cell_size();
                    let (x, y) = match event.px {
                        Some((x, y)) => {
                            event.col =
                                (x / u32::from(cell.width.max(1))).min(u32::from(u16::MAX)) as u16;
                            event.row =
                                (y / u32::from(cell.height.max(1))).min(u32::from(u16::MAX)) as u16;
                            (f64::from(x), f64::from(y))
                        }
                        None => (
                            (f64::from(event.col) + 0.5) * f64::from(cell.width),
                            (f64::from(event.row) + 0.5) * f64::from(cell.height),
                        ),
                    };
                    self.hits.hit_at(x, y)
                } else {
                    self.hits.hit(event.col, event.row)
                };
                self.apply(Msg::Mouse { event, target }, Activity::Input);
            }
            InputEvent::Event(Event::Paste(text)) => self.apply(Msg::Paste(text), Activity::Input),
            InputEvent::Event(Event::Focus(focused)) => {
                self.apply(Msg::Focus(focused), Activity::Message);
            }
            InputEvent::Event(Event::Reply(reply)) => {
                self.apply(Msg::Reply(reply), Activity::Message);
            }
            InputEvent::Resize(size) => {
                self.screen.resize(size.cols, size.rows);
                self.apply(Msg::Resize(size), Activity::Message);
                if self.model.has_pixels() {
                    // Font changes can alter both cells and physical pixels.
                    // Refresh the ruler used by layout and pointer conversion.
                    self.screen.enclose(b"\x1b[16t\x1b[14t");
                }
            }
            InputEvent::Closed => self.quit = true,
        }
    }

    fn apply(&mut self, msg: Msg, activity: Activity) {
        self.fold(msg);
        self.scheduler.note(activity, Instant::now());
    }

    /// Fold `msg` and act on what the core answered: quit, announcements
    /// and the animation declaration are the runtime's own; everything
    /// else goes to whoever took the commands.
    fn fold(&mut self, msg: Msg) {
        self.geometry_dirty = true;
        let sampling = matches!(msg, Msg::Phase(_));
        for cmd in update(&mut self.model, msg) {
            match cmd {
                Cmd::Quit => self.quit = true,
                Cmd::Announce(text) => self.announcements.push(text),
                Cmd::Clipboard(text) => self.clipboard.push(text),
                // Fractional sampling can finish a curve before the whole tick.
                // Keep that tick booked so the next gesture never restarts the
                // model clock at an earlier phase. The tick reconciles timing.
                Cmd::Animate(period) => {
                    if !sampling {
                        self.scheduler.animate(period, Instant::now());
                    }
                }
                Cmd::Still => {
                    if !sampling {
                        self.scheduler.still();
                    }
                }
                other => {
                    if let Err(error) = self.commands.send(other)
                        && let Cmd::Prompt(text) | Cmd::RoadmapPrompt(text) = error.0
                    {
                        self.fold(Msg::Agent(magistr_cockpit::agent::AgentEvent::PromptRejected {
                            text,
                            error: "Your message could not be sent because the agent connection closed. Your draft has been kept. Restart Magistr to reconnect.".into(),
                        }));
                    }
                }
            }
        }
        if self.flat.is_some() {
            self.echo
                .sync(self.model.prompt(), self.model.size().cols, &mut self.out);
        }
    }

    /// One animation step. It asks for a frame without touching the
    /// activity tier: a cockpit whose only news is its own spinner is
    /// still an idle one, and painting it at 60 fps would be a lie.
    fn tick(&mut self) {
        // Phase belongs to the interval that just ended. Clear it before
        // taking the next stamp so time never jumps ahead by that fraction.
        if self.model.has_pixels() {
            self.fold(Msg::Phase(0.0));
        }
        self.fold(Msg::Tick);
        // A sampling frame may already have retired the last animation.
        // Reconcile explicitly even if this whole tick changes no model flag.
        match self.model.animations().period() {
            Some(period) => self.scheduler.animate(period, Instant::now()),
            None => self.scheduler.still(),
        }
        self.scheduler.request();
    }

    /// Pixel motion is sampled at the interactive frame rate while its
    /// durations still advance on the model's existing tick. Plain terminals,
    /// reduced motion and a running clock retain their quieter cadence.
    fn interpolates(&self) -> bool {
        self.flat.is_none()
            && self.model.has_pixels()
            && self.model.theme().motion().is_full()
            && self.model.animates(Animation::Motion)
            && (self.model.is_scrolling()
                || self.model.motion().is_settling(self.model.now())
                || self.model.roadmap().is_gliding(self.model.now())
                || self.model.term_dock().is_settling(self.model.now()))
    }

    fn frame_period(&self, now: Instant) -> Duration {
        let period = self.scheduler.period(now);
        if self.interpolates() {
            period.min(super::tier::Tier::Interactive.period())
        } else if self.flat.is_none() && self.model.has_activity_motion() {
            // Only the small activity mark needs these intermediate samples.
            // Keep ambient feedback at 30 fps; gestures retain their 60 fps budget.
            period.min(Duration::from_millis(32))
        } else {
            period
        }
    }

    /// Fold one message a producer posted on the inbox.
    pub(crate) fn message(&mut self, msg: Msg) {
        self.apply(msg, Activity::Message);
    }

    /// Fold everything already queued on either channel, so a burst or a
    /// resize storm costs one paint.
    fn drain(
        &mut self,
        rx: &mut UnboundedReceiver<InputEvent>,
        inbox: &mut UnboundedReceiver<Msg>,
    ) {
        while !self.quit {
            if let Ok(event) = rx.try_recv() {
                self.event(event);
            } else if let Ok(msg) = inbox.try_recv() {
                self.message(msg);
            } else {
                break;
            }
        }
    }

    /// One frame: `FrameDue`, view, layout, paint, hit rects, present,
    /// write.
    ///
    /// The frame message goes through [`Driver::fold`] like every other,
    /// so what the core answers is acted on here too. It matters because
    /// `FrameDue` is what closes the activity ribbon's bucket
    /// ([`Model::roll_activity`]), and a ribbon that grew has a sweep to
    /// run — a `Cmd::Animate` dropped on the
    /// floor would leave the header's monitor frozen at the foot of it.
    fn paint<T: Terminal>(&mut self, terminal: &mut T) -> Result<(), RuntimeError> {
        if self.flat.is_some() {
            return self.write_flat(terminal);
        }
        let clock = FrameClock::new(self.model.frames(), self.started.elapsed());
        self.fold(Msg::FrameDue);
        assert!(
            self.panic_on_frame != Some(self.model.frames()),
            "cockpit fault injection: the view panicked on frame {}",
            self.model.frames()
        );
        let root = view(&self.model, &clock);
        let resolved = self.surfaces.layout(&root, &self.model, &mut self.tree)?;
        paint(&root, &resolved, self.screen.buffer());
        self.register_hits(&resolved);
        let report_changed =
            self.surfaces
                .show(&root, &mut self.model, &resolved, &mut self.screen);
        self.report_undock(&resolved);
        self.show_progress();
        self.out.clear();
        self.screen.present(&mut self.out);
        self.show_pointer();
        for text in self.announcements.drain(..) {
            announce(&mut self.out, &text);
        }
        for text in self.clipboard.drain(..) {
            osc52::clipboard(&mut self.out, &text);
        }
        terminal.write(&self.out)?;
        self.scheduler.painted();
        if report_changed {
            // This frame's layout was computed before the report landed
            // in the model; the next one draws the tab at its real size.
            self.scheduler.note(Activity::Message, Instant::now());
        }
        Ok(())
    }

    /// Input may arrive after resize or new content but before the next paint.
    /// Resolve the new bounds first instead of dispatching against yesterday's frame.
    fn refresh_hits(&mut self) {
        if !self.geometry_dirty || self.flat.is_some() {
            return;
        }
        let clock = FrameClock::new(self.model.frames(), self.started.elapsed());
        let root = view(&self.model, &clock);
        self.hits.clear();
        if let Ok(resolved) = self.surfaces.layout(&root, &self.model, &mut self.tree) {
            self.register_hits(&resolved);
        }
    }

    fn register_hits(&mut self, resolved: &Resolved) {
        self.hits.clear();
        if self.model.has_pixels() {
            self.hits.register_layout_px(resolved, 0);
            if let Some(graph) = resolved
                .placed()
                .iter()
                .find(|place| place.id == Some(Widget::TraceGraph.id()))
            {
                let rect = graph.px;
                let points =
                    FlightRecorder::graph(&self.model, rect.width).points(rect.width, rect.height);
                let indices = FlightRecorder::step_indices(&self.model, rect.width);
                // One graph geometry drives pixels, labels and activation. Its
                // layer remains below modal overlays; points outrank the graph.
                let z = graph
                    .layer
                    .saturating_mul(resolved.max_depth().saturating_add(1))
                    .saturating_add(graph.depth);
                for ((x, y), index) in points.into_iter().zip(indices) {
                    let hit = magistr_cockpit::layout::PxRect::new(
                        rect.x + x - 12.0,
                        rect.y + y - 12.0,
                        24.0,
                        24.0,
                    )
                    .intersection(rect);
                    self.hits.register_px(hit, z, Widget::trace_row(index));
                }
            }
        } else {
            self.hits.register_layout(resolved, 0);
        }
        self.geometry_dirty = false;
    }

    /// A flat frame appends settled transcript once per batch. Prompt
    /// edits were queued per fold, so even transient edits reach the wire.
    fn write_flat<T: Terminal>(&mut self, terminal: &mut T) -> Result<(), RuntimeError> {
        self.fold(Msg::FrameDue);
        assert!(
            self.panic_on_frame != Some(self.model.frames()),
            "cockpit fault injection: the view panicked on frame {}",
            self.model.frames()
        );
        self.compose_flat();
        self.flush_flat(terminal)?;
        self.scheduler.painted();
        Ok(())
    }

    /// Append new settled lines, then reconcile only the live input row.
    /// Keep transcript scanning on the frame, not on every keystroke.
    fn compose_flat(&mut self) {
        let fresh = self
            .flat
            .as_mut()
            .map(|transcript| transcript.advance(&self.model))
            .unwrap_or_default();
        if !fresh.is_empty() || !self.announcements.is_empty() {
            self.echo.finish(&mut self.out);
        }
        for line in fresh {
            write_line(&mut self.out, &line);
        }
        for text in self.clipboard.drain(..) {
            osc52::clipboard(&mut self.out, &text);
        }
        for text in self.announcements.drain(..) {
            announce(&mut self.out, &text);
            write_line(
                &mut self.out,
                &FlatLine::settled(Some(Role::Attention), plain(&text)).render(),
            );
        }
        self.echo
            .sync(self.model.prompt(), self.model.size().cols, &mut self.out);
    }

    fn flush_flat<T: Terminal>(&mut self, terminal: &mut T) -> Result<(), RuntimeError> {
        if self.flat.is_some() && !self.out.is_empty() {
            terminal.write(&self.out)?;
            self.out.clear();
        }
        Ok(())
    }

    /// Flush edits even on input closure, then give the shell a fresh line.
    pub(crate) fn finish_flat(&mut self, out: &mut Vec<u8>) {
        if self.flat.is_some() {
            self.compose_flat();
            out.append(&mut self.out);
            self.echo.finish(out);
        }
    }

    /// Where the undocked third column landed, so a wheel over it moves
    /// that column's rows instead of the stage's
    /// ([`Model::set_undock_rect`]). `None` on a
    /// screen too narrow to undock anything. The registry cannot answer
    /// this: [`HitRegistry::hit`](magistr_cockpit::hit::HitRegistry::hit) reports
    /// the *innermost* widget, so a notch over a trace row names that
    /// row and never the column holding it.
    ///
    /// Unlike the two graph reports this one changes no layout, so it
    /// asks for no follow-up frame.
    fn report_undock(&mut self, resolved: &Resolved) {
        let rect = resolved
            .placed()
            .iter()
            .find_map(|placed| (placed.id == Some(Widget::Undock.id())).then_some(placed.rect));
        self.model.set_undock_rect(rect);
    }

    /// The mouse-pointer shape for what the pointer is over, beside the
    /// frame's bytes: a hand over a button, a chip, a tab or a
    /// link, the terminal's own shape everywhere else. Written only when
    /// the shape *changes*, so resting on one button costs nothing, and
    /// only when the probe found kitty — every other terminal would
    /// print the string.
    fn show_pointer(&mut self) {
        if !self.model.has_pointer_shapes() {
            return;
        }
        let shape = PointerShape::over(self.model.hovered());
        if shape != self.pointer {
            self.pointer = shape;
            self.out.extend_from_slice(shape.sequence());
        }
    }

    /// Where the loop stands, as the terminal's own progress bar:
    /// iteration `n/N` as a fraction, a running gate as
    /// indeterminate, a failed gate as the error state. Enclosed in the
    /// frame's bracket, and written only when the state *changes*, so a
    /// loop grinding through one iteration costs nothing and a cockpit
    /// running no loop never says a word. Terminals off the allowlist are
    /// never told — they would print the sequence.
    fn show_progress(&mut self) {
        if !self.reports_progress {
            return;
        }
        let progress = Progress::for_loop(self.model.loop_status());
        if progress != self.progress {
            self.progress = progress;
            let mut bytes = Vec::new();
            progress.write(&mut bytes);
            self.screen.enclose(&bytes);
        }
    }

    /// Hand the pointer's shape back to the terminal (before leaving the
    /// session). A no-op when the cockpit never changed it.
    pub(crate) fn release_pointer(&mut self, out: &mut Vec<u8>) {
        if self.pointer != PointerShape::Default {
            self.pointer = PointerShape::Default;
            out.extend_from_slice(PointerShape::Default.sequence());
        }
    }

    /// Run until quit: events from `rx` and messages from `inbox` as they
    /// come (both drained), and one interval carrying both halves of the
    /// clock — an animation step when one is owed, then a frame when
    /// something changed. The interval runs at
    /// [`FrameScheduler::period`] (the tier's budget, or the animation's
    /// if that is shorter) and is re-made whenever that changes; a fresh
    /// interval ticks at once, so the first frame after a quiet spell is
    /// not held back. It is polled at all only while the scheduler
    /// [`wants_frame`](FrameScheduler::wants_frame) — a still, clean
    /// cockpit parks on its two channels and paints nothing. A closed
    /// input quits; a closed inbox only stops being polled. A frame is
    /// painted only after both channels are drained, so a message that
    /// reached the inbox before the tick is in the frame the tick paints.
    pub(crate) async fn run<T: Terminal>(
        &mut self,
        terminal: &mut T,
        rx: &mut UnboundedReceiver<InputEvent>,
        inbox: &mut UnboundedReceiver<Msg>,
    ) -> Result<(), RuntimeError> {
        if self.flat.is_some() {
            self.compose_flat();
            self.flush_flat(terminal)?;
        }
        let mut period = self.frame_period(Instant::now());
        let mut ticks = interval_for(period);
        let mut producers = true;
        while !self.quit {
            tokio::select! {
                event = rx.recv() => match event {
                    Some(event) => {
                        self.event(event);
                        self.drain(rx, inbox);
                    }
                    None => self.quit = true,
                },
                msg = inbox.recv(), if producers => match msg {
                    Some(msg) => {
                        self.message(msg);
                        self.drain(rx, inbox);
                    }
                    None => producers = false,
                },
                _ = ticks.tick(), if self.scheduler.wants_frame() => {
                    // Everything already on either channel is folded
                    // before this frame is painted. `select!` polls its
                    // branches in a random order, so a due tick can win
                    // over a message that reached the inbox first — and
                    // a frame painted around that message would break the
                    // one-frame promise (a permission request is on the
                    // very next frame) on exactly the runs where the
                    // machine is loaded and the tick is overdue.
                    self.drain(rx, inbox);
                    if self.quit {
                        break;
                    }
                    if self.scheduler.tick_due(Instant::now()) {
                        self.tick();
                    }
                    if self.interpolates()
                        || (self.flat.is_none() && self.model.has_activity_motion())
                    {
                        self.fold(Msg::Phase(self.scheduler.phase(Instant::now())));
                        self.scheduler.request();
                    }
                    if self.scheduler.is_dirty() {
                        self.paint(terminal)?;
                    }
                }
            }
            self.flush_flat(terminal)?;
            let next = self.frame_period(Instant::now());
            if next != period {
                period = next;
                ticks = interval_for(period);
            }
        }
        Ok(())
    }
}

/// One announcement on the screen-reader channel: an OSC 9 notification
/// beside the frame's bytes — the terminal's reader speaks it and the
/// cell grid is untouched. Control bytes become spaces so the text can
/// never break the sequence.
fn announce(out: &mut Vec<u8>, text: &str) {
    out.extend_from_slice(b"\x1b]9;");
    out.extend(text.bytes().map(|byte| {
        if byte < 0x20 || byte == 0x7f {
            b' '
        } else {
            byte
        }
    }));
    out.push(0x07);
}

/// One line of the flat cockpit. The carriage return is not optional:
/// the tty is in raw mode, so `\n` alone would step down a row and leave
/// the next line indented under the last.
fn write_line(out: &mut Vec<u8>, line: &str) {
    out.extend_from_slice(line.as_bytes());
    out.extend_from_slice(b"\r\n");
}

fn interval_for(period: Duration) -> Interval {
    let mut interval = tokio::time::interval(period);
    interval.set_missed_tick_behavior(MissedTickBehavior::Skip);
    interval
}

#[cfg(test)]
#[path = "driver.tests.rs"]
mod tests;
