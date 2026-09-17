use std::time::{Duration, Instant};

use magistr_cockpit::cockpit::Msg;
use magistr_cockpit::hit::HitRegistry;
use magistr_cockpit::input::MouseEvent;
use qs_gpu::batcher::Renderer;
use qs_gpu::device::{Acquired, GpuContext, SurfaceRecovery, acquire, new_instance};
use qs_gpu::frame::DrawList;
use qs_gpu::path::RenderPath;
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowId};

use std::sync::Arc;

use crate::element::scaled;
use crate::element_emit::emit;
use crate::element_layout::ElementLayout;
use crate::error::Gui2Error;
use crate::input::Input;
use crate::lexicon::Lexicon;
use crate::medium::Medium;
use crate::metrics::CellMetrics;
use crate::sink::Sink;
use crate::text::TextEngine;

/// Open the interactive component catalogue with `magistr-gui2 sink`.
///
/// The catalogue drives the element tree directly, without a cockpit model, using a
/// small winit/wgpu event loop.
///
/// State, scrolling, and hit testing use logical pixels. [`descale`] converts incoming
/// pointer coordinates; [`scaled`] converts the rendered tree to physical pixels for
/// the surface and scissor bounds.
///
/// # Errors
///
/// [`Gui2Error::Window`] or [`Gui2Error::Gpu`] if the event loop, window, or graphics
/// adapter cannot be created.
pub fn open_sink(title: &str) -> Result<(), Gui2Error> {
    let event_loop = EventLoop::new().map_err(|e| Gui2Error::Window(e.to_string()))?;
    let mut app = SinkApp {
        title: title.to_owned(),
        sink: Sink::new(Medium::Night),
        text: TextEngine::embedded(),
        input: Input::default(),
        generation: 0,
        scene: None,
        gpu: None,
        outcome: Ok(()),
    };
    event_loop
        .run_app(&mut app)
        .map_err(|e| Gui2Error::Window(e.to_string()))?;
    app.outcome
}

/// The sink's window once it is open.
struct Gpu {
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    context: GpuContext,
    renderer: Renderer,
}

struct SinkApp {
    title: String,
    sink: Sink,
    text: TextEngine,
    input: Input,
    generation: u64,
    /// The scaled physical scene, kept across frames: a hover changes
    /// what `emit` paints (it reads [`Sink::state`]) but never the
    /// tree, so only a page change ([`Sink::revision`]), a resize or a
    /// density change lays out again.
    scene: Option<Scene>,
    gpu: Option<Gpu>,
    outcome: Result<(), Gui2Error>,
}

/// The laid-out frame and the key it is valid under.
struct Scene {
    key: (u64, u32, u32, u32),
    tree: crate::element::Element,
    layout: ElementLayout,
}

impl SinkApp {
    /// The window's logical size at open — roomy enough for the widest
    /// exhibit column plus the page padding.
    const LOGICAL: (f64, f64) = (1180.0, 860.0);
    /// The transient tick — [`Sink::FILL_STEPS`] of these make the
    /// meter's sweep about 600 ms, the volume's own cadence for a
    /// designed transient.
    const TICK: Duration = Duration::from_millis(50);

    fn fail(&mut self, event_loop: &ActiveEventLoop, error: Gui2Error) {
        tracing::error!(target: "magistr-gui2", %error);
        self.outcome = Err(error);
        event_loop.exit();
    }

    /// The window's logical viewport, the space the tree is laid out in.
    fn viewport(&self) -> (f32, f32) {
        let Some(gpu) = &self.gpu else {
            return (Self::LOGICAL.0 as f32, Self::LOGICAL.1 as f32);
        };
        let physical = gpu.window.inner_size();
        let scale = gpu.window.scale_factor();
        #[allow(clippy::cast_possible_truncation)]
        (
            (f64::from(physical.width.max(1)) / scale) as f32,
            (f64::from(physical.height.max(1)) / scale) as f32,
        )
    }

    /// Fold `msg` into the sink; a change asks for a frame.
    fn fold(&mut self, msg: &Msg) {
        let viewport = self.viewport();
        if self.sink.fold(msg, viewport, &mut self.text)
            && let Some(gpu) = &self.gpu
        {
            gpu.window.request_redraw();
        }
    }

    /// Lay the sink out and present it.
    ///
    /// The sink *thinks* in logical pixels (state, scroll, hit tests),
    /// but the frame is drawn at the display's own density: the built
    /// tree's pixel tokens are multiplied by the scale factor and laid
    /// out over the physical surface, so glyphs and marks rasterize at
    /// native resolution and the renderer's scissor — which speaks
    /// framebuffer pixels — covers the whole window.
    fn paint(&mut self) -> Result<(), Gui2Error> {
        let (lw, lh) = self.viewport();
        let Some(gpu) = &mut self.gpu else {
            return Ok(());
        };
        let physical = gpu.window.inner_size();
        let (pw, ph) = (physical.width.max(1), physical.height.max(1));
        #[allow(clippy::cast_possible_truncation)]
        let scale = gpu.window.scale_factor() as f32;
        let key = (self.sink.revision(), pw, ph, scale.to_bits());
        let scene = match self.scene.take() {
            Some(scene) if scene.key == key => scene,
            _ => {
                let tree = scaled(self.sink.view((lw, lh), &mut self.text), scale);
                #[allow(clippy::cast_precision_loss)]
                let layout = ElementLayout::compute(&tree, pw as f32, ph as f32, &mut self.text)?;
                Scene { key, tree, layout }
            }
        };
        self.generation += 1;
        let ground = Lexicon::of(self.sink.medium()).base;
        let mut list = DrawList::default();
        list.reset([pw, ph], ground, self.generation);
        self.text.begin_frame();
        emit(
            &scene.tree,
            &scene.layout,
            self.sink.state(),
            &mut self.text,
            &mut list,
            0..scene.tree.count(),
        );
        self.scene = Some(scene);
        list.end_batch(None, true);
        list.finish();
        let glyphs = self.text.take_uploads();
        let images = self.text.take_image_uploads();
        let converging = !glyphs.is_empty() || !images.is_empty();
        gpu.renderer.upload_glyphs(&gpu.context, &glyphs);
        gpu.renderer.upload_images(&gpu.context, &images);

        let texture = match acquire(&gpu.surface) {
            Acquired::Frame(texture) | Acquired::Suboptimal(texture) => texture,
            Acquired::Recover(SurfaceRecovery::Reconfigure) => {
                let physical = gpu.window.inner_size();
                gpu.context.configure_surface(
                    &gpu.surface,
                    physical.width.max(1),
                    physical.height.max(1),
                );
                return Ok(());
            }
            Acquired::Recover(_) => return Ok(()),
        };
        let view = texture
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let commands = gpu.renderer.render(&gpu.context, &view, &list, None, None);
        gpu.context.queue.submit([commands]);
        gpu.window.pre_present_notify();
        gpu.context.queue.present(texture);
        // The atlas admits two pictures a frame, so a page of marks
        // converges over a few frames — same rule as `atom_still`.
        if converging {
            gpu.window.request_redraw();
        }
        Ok(())
    }
}

impl ApplicationHandler for SinkApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.gpu.is_some() {
            return;
        }
        let (width, height) = Self::LOGICAL;
        let attributes = Window::default_attributes()
            .with_title(&self.title)
            .with_inner_size(LogicalSize::new(width, height));
        let window = match event_loop.create_window(attributes) {
            Ok(window) => Arc::new(window),
            Err(e) => return self.fail(event_loop, Gui2Error::Window(e.to_string())),
        };
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
        context.configure_surface(&surface, size.width.max(1), size.height.max(1));
        let renderer =
            Renderer::with_colour_page(&context, self.text.atlas_size(), self.text.colour_size());
        window.request_redraw();
        self.gpu = Some(Gpu {
            window,
            surface,
            context,
            renderer,
        });
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        match &event {
            WindowEvent::CloseRequested => return event_loop.exit(),
            WindowEvent::Resized(size) => {
                if let Some(gpu) = &self.gpu {
                    gpu.context.configure_surface(
                        &gpu.surface,
                        size.width.max(1),
                        size.height.max(1),
                    );
                    gpu.window.request_redraw();
                }
                return;
            }
            WindowEvent::RedrawRequested => {
                if let Err(error) = self.paint() {
                    self.fail(event_loop, error);
                }
                return;
            }
            _ => {}
        }
        // The window's input layer speaks the cockpit's Msg; the sink
        // needs only the pixel, descaled to the logical space the tree
        // was laid out in. The cell registry the cockpit resolves
        // targets through has nothing to say here, so it stays empty.
        let scale = self
            .gpu
            .as_ref()
            .map_or(1.0, |gpu| gpu.window.scale_factor());
        let Some(metrics) = self.text.metrics(CellMetrics::DEFAULT_PX) else {
            return;
        };
        let (lw, lh) = self.viewport();
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let size = metrics.grid((lw * scale as f32) as u32, (lh * scale as f32) as u32);
        let hits = HitRegistry::new();
        let mut messages = Vec::new();
        self.input
            .fold(&event, &metrics, size, &hits, &mut messages);
        for message in messages {
            let message = descale(message, scale);
            self.fold(&message);
        }
    }

    /// The transient cadence: while a sweep or a deal is mid-flight the
    /// loop ticks and paints; a settled sink waits, costing nothing —
    /// the cockpit's own idle guarantee, kept on the dev route.
    fn about_to_wait(&mut self, _: &ActiveEventLoop) {
        if !self.sink.animating() {
            return;
        }
        self.fold(&Msg::Tick);
        if let Some(gpu) = &self.gpu {
            gpu.window.request_redraw();
        }
    }

    fn new_events(&mut self, event_loop: &ActiveEventLoop, _: winit::event::StartCause) {
        if self.sink.animating() {
            event_loop.set_control_flow(ControlFlow::WaitUntil(Instant::now() + Self::TICK));
        } else {
            event_loop.set_control_flow(ControlFlow::Wait);
        }
    }
}

/// A pointer message's physical pixel divided down to the logical
/// space the sink's layout lives in. Everything else passes through.
fn descale(msg: Msg, scale: f64) -> Msg {
    let Msg::Mouse { event, target } = &msg else {
        return msg;
    };
    let Some((x, y)) = event.px else {
        return msg;
    };
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let scaled = MouseEvent::new(event.kind, event.col, event.row)
        .at_px((f64::from(x) / scale) as u32, (f64::from(y) / scale) as u32);
    Msg::Mouse {
        event: scaled,
        target: *target,
    }
}

#[cfg(test)]
#[path = "sink_window.tests.rs"]
mod tests;
