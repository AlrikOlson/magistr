//! The native runtime: the cockpit's loop on a winit window.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use accesskit::{ActionHandler, ActionRequest, ActivationHandler, DeactivationHandler, TreeUpdate};

use magistr_cockpit::capability::Mode;
use magistr_cockpit::cockpit::{Animation, Cmd, FrameClock, Model, Msg, Widget, update};
use magistr_cockpit_runtime::channel::{Commands, Inbox};
use qs_gpu::batcher::Renderer;
use qs_gpu::device::{Acquired, GpuContext, SurfaceRecovery, acquire, new_instance};
use qs_gpu::path::RenderPath;
use qs_platform::{PlatformWindow, WinitWindow};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowId};

use crate::cadence::Cadence;
use crate::error::Gui2Error;
use crate::frame::Frame;
use crate::input::Input;
use crate::route::Route;

/// The cockpit's runtime on a native window — the same three verbs as
/// the terminal's `Runtime` (magistr-tui's), so an
/// entry point wires its producers to either without knowing which.
///
/// Producers post on the [`Inbox`] handed out by [`Native::inbox`]
/// before the run; a forwarder thread turns every message into a winit
/// user event, which is how a channel wakes an event loop that is
/// otherwise waiting (winit's `EventLoopProxy` is the one cross-thread
/// wake it offers). The session takes [`Native::commands`], where every
/// command the core answers with arrives — [`Route`] is the split
/// between what the loop keeps and what goes out.
///
/// The event loop runs on the calling thread, which macOS requires to
/// be the main one; the producers are tokio tasks the caller spawned,
/// so a `Native::run` inside a tokio runtime's `block_on` is the same
/// arrangement the terminal runtime is in when it awaits its own loop.
#[derive(Debug)]
pub struct Native {
    title: String,
    logical: LogicalSize<f64>,
    inbox: Inbox,
    messages: UnboundedReceiver<Msg>,
    commands_tx: UnboundedSender<Cmd>,
    commands: Option<Commands>,
}

impl Native {
    /// The window's logical size when the caller names none — a comfortable desktop
    /// cockpit, and the size the HiDPI grid is stated against.
    pub const DEFAULT_LOGICAL: (f64, f64) = (1280.0, 800.0);

    /// A runtime whose window will be titled `title`.
    #[must_use]
    pub fn new(title: &str) -> Self {
        let (inbox, messages) = Inbox::channel();
        let (commands_tx, commands) = Commands::channel();
        let (width, height) = Self::DEFAULT_LOGICAL;
        Self {
            title: title.to_owned(),
            logical: LogicalSize::new(width, height),
            inbox,
            messages,
            commands_tx,
            commands: Some(commands),
        }
    }

    /// Open the window at `width` x `height` **logical** pixels rather
    /// than [`Native::DEFAULT_LOGICAL`].
    ///
    /// Logical and not physical, because that is the size a design is
    /// drawn at: a capture taken in a 1132x641 browser viewport is the
    /// same arrangement on a 1x and a 2x display, which is what lets
    /// the live window be compared against the parity gate's still.
    #[must_use]
    pub fn with_logical_size(mut self, width: f64, height: f64) -> Self {
        self.logical = LogicalSize::new(width, height);
        self
    }

    /// The inbox producers post on; clone it out before [`Native::run`].
    #[must_use]
    pub fn inbox(&self) -> Inbox {
        self.inbox.clone()
    }

    /// The core's commands — `Some` on the first call; whoever takes it
    /// acts on every prompt, cancel and permission answer.
    pub fn commands(&mut self) -> Option<Commands> {
        self.commands.take()
    }

    /// Open the window and run `model` on it until it quits or the
    /// window is closed; returns the model as it was at the end.
    ///
    /// # Errors
    ///
    /// The first error that stopped the loop, so a machine with no
    /// usable adapter fails rather than showing an empty window and
    /// exiting 0.
    pub fn run(self, model: Model) -> Result<Model, Gui2Error> {
        let Self {
            title,
            logical,
            inbox,
            mut messages,
            commands_tx,
            commands,
        } = self;
        // An untaken receiver is dropped here: the core's sends fail
        // quietly instead of queueing for nobody.
        drop(commands);
        let event_loop = EventLoop::<Msg>::with_user_event()
            .build()
            .map_err(|e| Gui2Error::Window(e.to_string()))?;
        let proxy = event_loop.create_proxy();
        // The runtime's own copy of the inbox goes away with it, so the
        // forwarder ends once every producer has hung up — or on the
        // first message after the loop is gone, whichever comes first.
        drop(inbox);
        std::thread::Builder::new()
            .name("gui2-inbox".to_owned())
            .spawn(move || {
                while let Some(msg) = messages.blocking_recv() {
                    if proxy.send_event(msg).is_err() {
                        break;
                    }
                }
            })
            .map_err(|e| Gui2Error::Window(e.to_string()))?;

        let mut app = App {
            title,
            logical,
            frame: Frame::new()?,
            model,
            route: Route::new(commands_tx),
            input: Input::default(),
            started: Instant::now(),
            cadence: Cadence::new(),
            last_paint: Instant::now(),
            listening: Listening::default(),
            published: false,
            gpu: None,
            outcome: Ok(()),
        };
        event_loop
            .run_app(&mut app)
            .map_err(|e| Gui2Error::Window(e.to_string()))?;
        app.outcome.map(|()| app.model)
    }
}

/// The window and everything that only exists once it is open.
struct Gpu {
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    context: GpuContext,
    renderer: Renderer,
    access: accesskit_winit::Adapter,
}

/// Whether assistive technology has asked for the tree.
///
/// The three AccessKit handlers may be called on any thread, so the one
/// thing they share with the loop is this flag: the loop builds an
/// [`AccessTree`](crate::AccessTree) only while something is listening,
/// which is what keeps a cockpit nobody is reading with a screen reader
/// exactly as cheap to paint as it was before.
#[derive(Clone, Default)]
struct Listening(Arc<AtomicBool>);

impl Listening {
    fn is_on(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }
}

impl ActivationHandler for Listening {
    /// No initial tree: the loop's next paint sends the whole one, and
    /// the platform shows a placeholder until it does. Returning a tree
    /// here would mean building one on a thread that does not own the
    /// frame.
    fn request_initial_tree(&mut self) -> Option<TreeUpdate> {
        self.0.store(true, Ordering::Relaxed);
        None
    }
}

impl DeactivationHandler for Listening {
    fn deactivate_accessibility(&mut self) {
        self.0.store(false, Ordering::Relaxed);
    }
}

/// The cockpit publishes roles, names and bounds and answers no actions yet: a click
/// delivered here would have to become a `Msg` on the inbox, which is its own chunk.
/// Refusing silently is what AccessKit expects of an unsupported action, and no node
/// advertises one.
struct NoActions;

impl ActionHandler for NoActions {
    fn do_action(&mut self, _: ActionRequest) {}
}

struct App {
    title: String,
    /// The window's logical size, honoured whatever the display's
    /// density is — see [`Native::with_logical_size`].
    logical: LogicalSize<f64>,
    frame: Frame,
    model: Model,
    route: Route,
    input: Input,
    started: Instant,
    /// The step clock, the sub-tick phase every frame is sampled at, and
    /// the display's own frame interval.
    cadence: Cadence,
    /// When the last frame was painted, so the next one is asked for on
    /// the display's cadence rather than as fast as the GPU will go.
    last_paint: Instant,
    /// Set while assistive technology is reading the window.
    listening: Listening,
    /// Whether a real tree has reached the adapter since it activated.
    published: bool,
    gpu: Option<Gpu>,
    outcome: Result<(), Gui2Error>,
}

impl App {
    fn fail(&mut self, event_loop: &ActiveEventLoop, error: Gui2Error) {
        tracing::error!(target: "magistr-gui2", %error);
        self.outcome = Err(error);
        event_loop.exit();
    }

    /// Fold `msg` through the route; a quit ends the loop.
    fn fold(&mut self, event_loop: &ActiveEventLoop, msg: Msg) {
        if self.route.fold(&mut self.model, msg) {
            event_loop.exit();
        }
    }

    /// The model's grid re-derived from the surface's pixels, folded in
    /// as the resize the cockpit already knows how to take.
    fn resize(&mut self, width: u32, height: u32) {
        let Some(gpu) = &self.gpu else {
            return;
        };
        gpu.context
            .configure_surface(&gpu.surface, width.max(1), height.max(1));
        let size = self.frame.metrics().grid(width, height);
        if size != self.model.size() {
            update(&mut self.model, Msg::Resize(size));
        }
    }

    /// Build a frame and present it.
    ///
    /// `FrameDue` is folded first, as the terminal driver folds it: it
    /// is what closes the activity ribbon's bucket, and a ribbon that
    /// grew may declare a sweep — which the next wait reads off the
    /// model, so nothing here has to carry it.
    ///
    /// A recoverable surface state is not an error: `Outdated` fires on
    /// every resize and `Occluded` whenever the window is minimized, so
    /// treating either as a failure would rebuild the world on a window
    /// drag or exit on a minimize.
    fn paint(&mut self) -> Result<(), Gui2Error> {
        // Read before the window is borrowed: the phase asks the model
        // what it is animating, and the borrow takes the whole `App`.
        let phase = self.phase();
        let Some(gpu) = &mut self.gpu else {
            return Ok(());
        };
        let physical = gpu.window.inner_size();
        let (width, height) = (physical.width.max(1), physical.height.max(1));
        // The one wall-clock sample a frame gets: the Signals inbox
        // reads its ages off it, and nothing else in the cockpit ever
        // asks the time.
        let mut clock = FrameClock::new(self.model.frames(), self.started.elapsed());
        if let Ok(since) = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
            clock = clock.with_epoch_secs(since.as_secs());
        }
        // Where inside the current step this frame sits, folded before
        // anything else: it decides where every curve in the motion
        // layer is read, and a `FrameDue` that stamps a sweep should
        // stamp it at the moment the frame is actually being drawn.
        // Zero whenever nothing is moving (`Cadence::phase` on a stopped
        // clock), so a settled window is the still the goldens pin.
        self.route.fold(&mut self.model, Msg::Phase(phase));
        self.route.fold(&mut self.model, Msg::FrameDue);
        // A changed graph-rect report means the next layout depends on
        // what this frame just measured — the follow-up frame the
        // terminal runtime schedules, as a redraw request.
        let reported = self.frame.build(&mut self.model, &clock, width, height)?;
        if reported {
            gpu.window.request_redraw();
        }

        if let Some([x, y, w, h]) = self
            .frame
            .rect_of(crate::comp_composer::ComposerComposition::PILL)
        {
            gpu.window.set_ime_cursor_area(
                winit::dpi::PhysicalPosition::new(x, y + h),
                winit::dpi::PhysicalSize::new(w.max(1.0), 1.0),
            );
        }
        let uploads = self.frame.text().take_uploads();
        let images = self.frame.text().take_image_uploads();
        gpu.renderer.upload_glyphs(&gpu.context, &uploads);
        gpu.renderer.upload_images(&gpu.context, &images);

        let texture = match acquire(&gpu.surface) {
            Acquired::Frame(texture) | Acquired::Suboptimal(texture) => texture,
            Acquired::Recover(SurfaceRecovery::Reconfigure) => {
                gpu.context.configure_surface(&gpu.surface, width, height);
                return Ok(());
            }
            Acquired::Recover(_) => return Ok(()),
        };
        let view = texture
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let commands = gpu
            .renderer
            .render(&gpu.context, &view, self.frame.list(), None, None);
        gpu.context.queue.submit([commands]);
        gpu.window.pre_present_notify();
        gpu.context.queue.present(texture);
        self.last_paint = Instant::now();
        // The tree is derived from the frame that was just painted, and
        // only while something is reading it — see `Listening`.
        if self.listening.is_on() {
            let tree = self.frame.access(gpu.window.scale_factor());
            // The window's prompt is the composer; the fallback-to-window rule covers a
            // slot without one.
            gpu.access
                .update_if_active(|| tree.update(Some(Widget::Composer)));
            self.published = true;
        } else {
            self.published = false;
        }
        Ok(())
    }

    fn request_redraw(&self) {
        if let Some(gpu) = &self.gpu {
            gpu.window.request_redraw();
        }
    }

    /// The sub-tick phase this frame is drawn at.
    ///
    /// Only while the cockpit declares [`Animation::Motion`]. A clock's
    /// second has nothing between its steps to show — the header's
    /// `mm:ss` changes once a second whatever the display does — so a
    /// loop running quietly keeps the terminal's own contract of one
    /// frame a second rather than paying for a hundred and twenty.
    fn phase(&self) -> f32 {
        let Some(period) = self.model.animations().period() else {
            return 0.0;
        };
        if !self.model.animates(Animation::Motion) {
            return 0.0;
        }
        self.cadence.phase(Instant::now(), period)
    }
}

impl ApplicationHandler<Msg> for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.gpu.is_some() {
            return;
        }
        // The window opens on the cell grid the metrics make of the
        // logical size, so the first frame is already the right shape.
        //
        // It opens *invisible* because AccessKit's winit adapter has to
        // subclass the view before it is shown, and panics otherwise;
        // `set_visible` below is the first frame's own show.
        let attributes = Window::default_attributes()
            .with_title(&self.title)
            .with_visible(false)
            .with_inner_size(self.logical);
        let window = match event_loop.create_window(attributes) {
            Ok(window) => Arc::new(window),
            Err(e) => return self.fail(event_loop, Gui2Error::Window(e.to_string())),
        };
        let scale = window.scale_factor();
        let access = accesskit_winit::Adapter::with_direct_handlers(
            event_loop,
            &window,
            self.listening.clone(),
            NoActions,
            self.listening.clone(),
        );
        window.set_ime_allowed(true);
        window.set_visible(true);

        let instance = new_instance(RenderPath::Primary);
        let surface = match instance.create_surface(window.clone()) {
            Ok(surface) => surface,
            Err(e) => return self.fail(event_loop, Gui2Error::Gpu(e.to_string())),
        };
        let context = match GpuContext::new(RenderPath::Primary, instance, Some(&surface)) {
            Ok(context) => context,
            Err(e) => return self.fail(event_loop, Gui2Error::Gpu(e.to_string())),
        };
        let size = window.inner_size();
        let present_mode =
            context.configure_surface(&surface, size.width.max(1), size.height.max(1));
        // Both pages' edges are the atlas's own: the coverage page and
        // the colour page the stream's pictures land on, which is wider
        // than the framework's default (`TextEngine::COLOUR`).
        let (atlas, colour) = (
            self.frame.text().atlas_size(),
            self.frame.text().colour_size(),
        );
        let renderer = Renderer::with_colour_page(&context, atlas, colour);

        let refresh_hz = WinitWindow::new(window.clone())
            .current_display()
            .and_then(|display| display.refresh_hz);
        // The loop's frame cadence is the display's, from here on.
        self.cadence.set_refresh(refresh_hz);
        tracing::info!(
            target: "magistr-gui2",
            width = size.width,
            height = size.height,
            ?present_mode,
            ?refresh_hz,
            "surface configured"
        );

        self.gpu = Some(Gpu {
            window,
            surface,
            context,
            renderer,
            access,
        });
        // Apply the window scale before deriving the grid. A 2x display receives a 2x
        // cell while text retains its logical size. Scale 1 leaves the headless grid
        // unchanged.
        if let Err(error) = self.frame.rescale(scale) {
            return self.fail(event_loop, error);
        }
        // The caller's model is what runs: it is re-gridded to the
        // window rather than replaced, and told it has a pixel layer
        // through the message the startup probe would have folded (see
        // `Fixture::with_pixels` for why the vocabulary is kitty's).
        self.resize(size.width, size.height);
        // What a live smoke has to read to say the density was honoured:
        // the scale, the cell it made and the grid that came out of it.
        let cell = self.frame.metrics().cell_pixels();
        tracing::info!(
            target: "magistr-gui2",
            scale,
            cell_px = ?cell,
            grid = ?self.model.size(),
            "cockpit gridded"
        );
        update(
            &mut self.model,
            Msg::Mode(Mode::Graphics {
                pointer_shapes: true,
            }),
        );
        self.request_redraw();
    }

    /// A producer's message, forwarded off the inbox.
    fn user_event(&mut self, event_loop: &ActiveEventLoop, msg: Msg) {
        self.fold(event_loop, msg);
        self.request_redraw();
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        if let Some(gpu) = &mut self.gpu {
            // Before the application handles it, as the adapter asks:
            // this is how it learns the view took or lost focus.
            gpu.access.process_event(&gpu.window, &event);
        }
        match &event {
            WindowEvent::CloseRequested => return event_loop.exit(),
            WindowEvent::Resized(size) => {
                self.resize(size.width, size.height);
                self.request_redraw();
                return;
            }
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                // The window moved to a display that draws at a
                // different density (or the user changed the setting):
                // the face follows it, so the text stays
                // `CellMetrics::DEFAULT_PX` *logical* and the grid stays
                // the one the window's logical size implies.
                //
                // The **grid** is corrected by the `Resized` that
                // follows, not here: winit has not updated the window's
                // inner size when this fires (rust-windowing/winit#3080)
                // and writing the `inner_size_writer` from inside this
                // handler panics (#3704), so we touch neither. Feeding
                // the size we *can* see through the ordinary resize is
                // still right — at worst it is the old one, replaced a
                // moment later — and it is what re-grids a platform that
                // changes the scale without changing the physical size.
                if let Err(error) = self.frame.rescale(*scale_factor) {
                    return self.fail(event_loop, error);
                }
                let physical = self.gpu.as_ref().map(|gpu| gpu.window.inner_size());
                if let Some(physical) = physical {
                    self.resize(physical.width, physical.height);
                }
                self.request_redraw();
                return;
            }
            WindowEvent::RedrawRequested => {
                if let Err(error) = self.paint() {
                    return self.fail(event_loop, error);
                }
                return;
            }
            // A folder dragged from the desktop: hovering arms Birth's slot, dropping
            // lands the ground, leaving disarms it — the cockpit's own messages, folded
            // like every other input.
            WindowEvent::HoveredFile(_) => {
                self.fold(event_loop, Msg::DragHover(true));
                self.request_redraw();
                return;
            }
            WindowEvent::HoveredFileCancelled => {
                self.fold(event_loop, Msg::DragHover(false));
                self.request_redraw();
                return;
            }
            WindowEvent::DroppedFile(path) => {
                let path = path.to_string_lossy().into_owned();
                self.fold(event_loop, Msg::Dropped(path));
                self.request_redraw();
                return;
            }
            _ => {}
        }

        let mut messages = Vec::new();
        let metrics = self.frame.metrics();
        let size = self.model.size();
        self.input
            .fold(&event, &metrics, size, self.frame.hits(), &mut messages);
        let preedit_changed = self.frame.set_preedit(self.input.preedit());
        let redraw = !messages.is_empty() || preedit_changed;
        for message in messages {
            for message in self.frame.input(&self.model, message) {
                self.fold(event_loop, message);
            }
        }
        if redraw {
            self.request_redraw();
        }
    }

    /// The cockpit's tick, on the period the *model* declares, and the
    /// display's own frames on top of it.
    ///
    /// A cockpit declaring no animation asks for no frames at all and
    /// the loop simply waits — the terminal runtime's idle guarantee,
    /// which is what `Cmd::Still` buys there and `ControlFlow::Wait`
    /// buys here. Reduced motion answers the empty set, so both the
    /// steps and the phase are structurally absent on this path too.
    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        // Assistive technology activated between frames: the adapter is
        // holding a placeholder until a paint hands it a real tree, and
        // an idle cockpit would otherwise never paint one.
        if self.listening.is_on() && !self.published {
            self.request_redraw();
        }
        let Some(period) = self.model.animations().period() else {
            self.cadence.stop();
            return event_loop.set_control_flow(ControlFlow::Wait);
        };
        let now = Instant::now();
        for _ in 0..self.cadence.owed(now, period) {
            self.fold(event_loop, Msg::Tick);
        }
        // Active motion is paced at the display refresh interval. The framework's
        // non-blocking `Mailbox` presentation does not provide that pacing itself.
        //
        // Without motion, wait for the next model tick. Elapsed-time updates in an
        // otherwise idle cockpit therefore need about one frame per second.
        if self.model.animates(Animation::Motion) {
            let frame = self.cadence.frame(self.last_paint);
            if now >= frame {
                self.request_redraw();
            }
            return event_loop.set_control_flow(ControlFlow::WaitUntil(frame.max(now)));
        }
        let due = self.cadence.due().unwrap_or(now);
        event_loop.set_control_flow(ControlFlow::WaitUntil(due.max(now)));
    }
}

#[cfg(test)]
#[path = "native.tests.rs"]
mod tests;
