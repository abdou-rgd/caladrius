//! Renders the screen offscreen to four PNGs (light and dark, fresh and stale) at 1440x900, without
//! a window (golden rule 8): `cargo run -p caladrius-proto-egui --example snapshot`.
//!
//! The renderer is the Vulkan-only twin of the one in `egui_kittest`: the published `gpu-allocator`
//! does not build with the DirectX 12 backend of wgpu 30 yet (see the card), so the DX12 backend
//! is not compiled in and the test harness must not ask for it.

use std::path::{Path, PathBuf};

use caladrius_proto_egui::app::{App, EDGE_BLQ};
use caladrius_proto_egui::state::Screen;
use egui::TexturesDelta;
use egui_kittest::{Harness, TestRenderer};
use egui_wgpu::{RenderState, ScreenDescriptor, WgpuSetup, wgpu};
use image::RgbaImage;

const SIZE: (f32, f32) = (1440.0, 900.0);

/// Draws egui output into a texture on the first Vulkan adapter and reads it back.
struct VulkanRenderer {
    state: RenderState,
}

impl VulkanRenderer {
    fn new() -> Result<VulkanRenderer, String> {
        let mut setup = egui_wgpu::WgpuSetupCreateNew::without_display_handle();
        setup.instance_descriptor.backends = wgpu::Backends::VULKAN;
        let setup = WgpuSetup::CreateNew(setup);
        let instance = pollster::block_on(setup.new_instance());
        let state = pollster::block_on(RenderState::create(
            &egui_wgpu::WgpuConfiguration {
                wgpu_setup: setup,
                ..Default::default()
            },
            &instance,
            None,
            egui_wgpu::RendererOptions::PREDICTABLE,
        ))
        .map_err(|e| format!("no Vulkan device: {e}"))?;
        Ok(VulkanRenderer { state })
    }
}

impl TestRenderer for VulkanRenderer {
    fn handle_delta(&mut self, delta: &mut TexturesDelta) {
        let mut renderer = self.state.renderer.write();
        for (id, images) in delta.set.drain() {
            for image in images {
                renderer.update_texture(&self.state.device, &self.state.queue, id, &image);
            }
        }
        for id in delta.free.drain() {
            renderer.free_texture(&id);
        }
    }

    fn render(
        &mut self,
        ctx: &egui::Context,
        output: &egui::FullOutput,
    ) -> Result<RgbaImage, String> {
        let mut renderer = self.state.renderer.write();
        let mut encoder =
            self.state
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("snapshot encoder"),
                });
        let ppp = ctx.pixels_per_point();
        let size = ctx.content_rect().size() * ppp;
        let screen = ScreenDescriptor {
            pixels_per_point: ppp,
            size_in_pixels: [size.x.round() as u32, size.y.round() as u32],
        };
        let tessellated = ctx.tessellate(output.shapes.clone(), ppp);
        let user_buffers = renderer.update_buffers(
            &self.state.device,
            &self.state.queue,
            &mut encoder,
            &tessellated,
            &screen,
        );
        let texture = self.state.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("snapshot target"),
            size: wgpu::Extent3d {
                width: screen.size_in_pixels[0],
                height: screen.size_in_pixels[1],
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: self.state.target_format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        {
            let mut pass = encoder
                .begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("snapshot pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                            store: wgpu::StoreOp::Store,
                        },
                        depth_slice: None,
                    })],
                    ..Default::default()
                })
                .forget_lifetime();
            renderer.render(&mut pass, &tessellated, &screen);
        }
        self.state.queue.submit(
            user_buffers
                .into_iter()
                .chain(std::iter::once(encoder.finish())),
        );
        self.state
            .device
            .poll(wgpu::PollType::Wait {
                submission_index: None,
                timeout: Some(std::time::Duration::from_secs(10)),
            })
            .map_err(|e| format!("poll: {e}"))?;
        read_back(&self.state.device, &self.state.queue, &texture)
    }
}

fn read_back(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    texture: &wgpu::Texture,
) -> Result<RgbaImage, String> {
    let (w, h) = (texture.width() as usize, texture.height() as usize);
    let unpadded = w * 4;
    let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT as usize;
    let padded = unpadded + (align - unpadded % align) % align;
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("snapshot readback"),
        size: (padded * h) as u64,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("snapshot copy"),
    });
    encoder.copy_texture_to_buffer(
        texture.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(padded as u32),
                rows_per_image: None,
            },
        },
        wgpu::Extent3d {
            width: texture.width(),
            height: texture.height(),
            depth_or_array_layers: 1,
        },
    );
    let index = queue.submit(std::iter::once(encoder.finish()));
    let (tx, rx) = std::sync::mpsc::channel();
    buffer
        .slice(..)
        .map_async(wgpu::MapMode::Read, move |r| drop(tx.send(r)));
    device
        .poll(wgpu::PollType::Wait {
            submission_index: Some(index),
            timeout: Some(std::time::Duration::from_secs(10)),
        })
        .map_err(|e| format!("poll: {e}"))?;
    rx.recv()
        .map_err(|e| format!("map: {e}"))?
        .map_err(|e| format!("map: {e}"))?;
    let mapped = buffer
        .slice(..)
        .get_mapped_range()
        .map_err(|e| format!("range: {e}"))?;
    let data: Vec<u8> = mapped
        .chunks_exact(padded)
        .flat_map(|row| row.iter().take(unpadded))
        .copied()
        .collect();
    RgbaImage::from_raw(texture.width(), texture.height(), data)
        .ok_or_else(|| "bad image size".to_owned())
}

fn render(app: App, path: &Path) -> Result<(), String> {
    let mut app = app;
    let mut harness = Harness::builder()
        .with_size(egui::vec2(SIZE.0, SIZE.1))
        .renderer(VulkanRenderer::new()?)
        .build_ui(move |ui| app.ui(ui));
    harness.run();
    let image = harness.render()?;
    if let Err(e) = image.save(path) {
        // This PC refuses file writes to programs built under target/ (os error 5): hand the
        // PNG to the shell as hex on stdout, `scripts` side decodes it (see the README).
        eprintln!("cannot write {}: {e}; printing it as hex", path.display());
        let mut bytes = Vec::new();
        image
            .write_to(
                &mut std::io::Cursor::new(&mut bytes),
                image::ImageFormat::Png,
            )
            .map_err(|e| format!("png: {e}"))?;
        let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
        let name = path.file_name().map(|n| n.to_string_lossy().into_owned());
        println!("PNG {} {hex}", name.unwrap_or_default());
        return Ok(());
    }
    println!("wrote {}", path.display());
    Ok(())
}

fn scene(dark: bool, stale: bool) -> Result<App, String> {
    let mut screen = Screen::new();
    screen.load_csv("edge_blq", EDGE_BLQ, "3")?;
    if stale {
        // One edited cell: the data no longer match the last run (UX-FR-12).
        let row = screen
            .table
            .iter()
            .find(|r| r.conc > 0.0)
            .map(|r| (r.sheet_row, r.conc))
            .ok_or("no row to edit")?;
        screen.edit_cell(row.0, "conc", &format!("{:.2}", row.1 * 1.1));
    }
    Ok(App::with_screen(screen, dark))
}

fn main() {
    let dir = PathBuf::from(
        std::env::args()
            .nth(1)
            .unwrap_or_else(|| "prototypes/egui/screens".to_owned()),
    );
    if let Err(e) = std::fs::create_dir_all(&dir) {
        eprintln!("cannot create {}: {e}", dir.display());
    }
    let mut failed = false;
    for (dark, stale) in [(false, false), (true, false), (false, true), (true, true)] {
        let name = format!(
            "{}-{}.png",
            if dark { "dark" } else { "light" },
            if stale { "stale" } else { "fresh" }
        );
        let done = scene(dark, stale).and_then(|app| render(app, &dir.join(name)));
        if let Err(e) = done {
            eprintln!("snapshot failed: {e}");
            failed = true;
        }
    }
    if failed {
        std::process::exit(1);
    }
}
