//! Opt-in hardware check: real shader execution and texture readback in both themes.
//! Run explicitly on a host with a usable GPU; absence is an error, never a pass.
use magistr_cockpit::cockpit::FrameClock;
use magistr_cockpit::theme::{Theme, ThemeMode};
use magistr_gui2::{Frame, fixture::Fixture};
use qs_gpu::batcher::Renderer;
use qs_gpu::device::{GpuContext, new_instance};
use qs_gpu::path::RenderPath;
use tiny_skia::Pixmap;

#[test]
#[ignore = "requires a GPU; run the documented desktop GPU gate explicitly"]
fn gpu_renders_fixture_images_in_both_themes() {
    let ctx = GpuContext::new(RenderPath::Primary, new_instance(RenderPath::Primary), None)
        .expect("desktop GPU required");
    eprintln!("GPU: {:?}", ctx.capabilities);
    for (tag, mode) in [("dark", ThemeMode::Dark), ("light", ThemeMode::Light)] {
        for scene in ["adoption", "roadmap", "requests", "composer"] {
            let mut frame = Frame::new().expect("embedded fonts");
            let cell = frame.metrics();
            let size = Fixture::capture_grid((cell.width(), cell.height()));
            let mut model = match scene {
                "roadmap" => Fixture::roadmap(size),
                "requests" => {
                    Fixture::roadmap_request(size, magistr_cockpit::cockpit::RequestPhase::Complete)
                }
                _ => Fixture::with_pixels(Fixture::imaged(size, 120, 80)),
            }
            .with_theme(Theme::forced(mode));
            if scene == "composer" {
                use magistr_cockpit::cockpit::{Msg, Widget, update};
                use magistr_cockpit::input::{Button, MouseEvent, MouseKind};
                model = model
                    .with_workspace_files(vec!["src/main.rs".into(), "résumé draft.md".into()]);
                update(&mut model, Msg::Paste("Review @src/main.rs\n".repeat(16)));
                update(
                    &mut model,
                    Msg::Mouse {
                        event: MouseEvent::new(MouseKind::Down(Button::Left), 0, 0),
                        target: Some(Widget::PromptFiles.id()),
                    },
                );
            }
            let mut cpu_model = model.clone();
            let mut cpu_frame = Frame::new().expect("CPU fonts");
            let clock = FrameClock::new(0, std::time::Duration::ZERO);
            let (width, height) = Fixture::CAPTURE;
            let cpu = cpu_frame
                .still(&mut cpu_model, &clock, width, height)
                .expect("CPU frame");
            let mut renderer = Renderer::with_colour_page(
                &ctx,
                frame.text().atlas_size(),
                frame.text().colour_size(),
            );
            let mut settled = false;
            for _ in 0..8 {
                let reported = frame
                    .build(&mut model, &clock, width, height)
                    .expect("GPU layout");
                let glyphs = frame.text().take_uploads();
                let images = frame.text().take_image_uploads();
                renderer.upload_glyphs(&ctx, &glyphs);
                renderer.upload_images(&ctx, &images);
                if !reported && glyphs.is_empty() && images.is_empty() {
                    settled = true;
                    break;
                }
            }
            assert!(
                settled,
                "layout and atlas must converge within eight passes"
            );
            assert_eq!(model.now(), cpu_model.now());
            let gpu = readback(&ctx, &mut renderer, &frame, width, height);
            assert!(
                gpu.data().chunks_exact(4).all(|p| p[3] == 255),
                "opaque canvas"
            );
            // Compare real output across the whole viewport while allowing text edge
            // coverage to differ between the CPU and GPU rasterizers.
            let matching = gpu
                .data()
                .chunks_exact(4)
                .zip(cpu.data().chunks_exact(4))
                .filter(|(a, b)| a.iter().zip(*b).all(|(x, y)| x.abs_diff(*y) <= 4))
                .count();
            let pixels = (width * height) as usize;
            assert!(
                matching * 100 >= pixels * 95,
                "{scene}-{tag}: CPU/GPU agreement {matching}/{pixels} below 95%"
            );
            let red = gpu
                .data()
                .chunks_exact(4)
                .filter(|p| {
                    p[0].abs_diff(200) <= 2 && p[1].abs_diff(40) <= 2 && p[2].abs_diff(40) <= 2
                })
                .count();
            eprintln!("{scene}-{tag}: {matching}/{pixels} pixels agree; {red} image pixels");
            let output = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target");
            gpu.save_png(output.join(format!("{scene}-gpu-{tag}.png")))
                .expect("GPU capture");
            cpu.save_png(output.join(format!("{scene}-cpu-{tag}.png")))
                .expect("CPU capture");
            assert!(
                scene != "adoption" || red >= (120 - 2) * (80 - 2), // exclude the subpixel coverage at the border
                "{scene}-{tag}: image atlas content missing ({red} pixels)"
            );
        }
    }
}

fn readback(
    ctx: &GpuContext,
    renderer: &mut Renderer,
    frame: &Frame,
    width: u32,
    height: u32,
) -> Pixmap {
    let extent = wgpu::Extent3d {
        width,
        height,
        depth_or_array_layers: 1,
    };
    let format = ctx.capabilities.surface_format;
    assert!(matches!(
        format,
        wgpu::TextureFormat::Bgra8UnormSrgb | wgpu::TextureFormat::Rgba8UnormSrgb
    ));
    let texture = ctx.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("desktop fixture capture"),
        size: extent,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    let rendered = renderer.render(ctx, &view, frame.list(), None, None);
    let padded = (width * 4).div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT)
        * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
    let buffer = ctx.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("desktop fixture readback"),
        size: u64::from(padded) * u64::from(height),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = ctx
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(padded),
                rows_per_image: Some(height),
            },
        },
        extent,
    );
    ctx.queue.submit([rendered, encoder.finish()]);
    let slice = buffer.slice(..);
    slice.map_async(wgpu::MapMode::Read, |result| result.expect("map capture"));
    ctx.device
        .poll(wgpu::PollType::wait_indefinitely())
        .expect("GPU completion");
    let mapped = slice.get_mapped_range().expect("capture bytes");
    let mut pixmap = Pixmap::new(width, height).expect("capture dimensions");
    for (source, dest) in mapped
        .chunks_exact(padded as usize)
        .zip(pixmap.data_mut().chunks_exact_mut((width * 4) as usize))
    {
        dest.copy_from_slice(&source[..(width * 4) as usize]);
    }
    drop(mapped);
    buffer.unmap();
    if format == wgpu::TextureFormat::Bgra8UnormSrgb {
        for pixel in pixmap.data_mut().chunks_exact_mut(4) {
            pixel.swap(0, 2);
        }
    }
    pixmap
}
