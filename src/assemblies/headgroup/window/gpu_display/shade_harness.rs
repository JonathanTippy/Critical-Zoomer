//! Headless rig which runs the real sampling + shading wgsl and hands back the pixels.
//!
//! Tests build a small patch of raw answers, wrap it in a single tile, render it offscreen
//! and compare against `shade_oracle`. No window, no egui, no hoard.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::OnceLock;
use std::task::Context;
use std::task::Poll;
use std::task::Wake;
use std::task::Waker;
use std::time::{Duration, Instant};

use eframe::egui_wgpu::wgpu;

use super::shade_oracle::RawGrid;
use super::GpuDisplayResources;
use super::GpuInstruction;
use super::GpuTileEntry;
use super::PendingTileUpload;
use super::ShadeFrame;
use super::ShadeUniforms;
use super::GPU_TILE_CELL_SLOTS;
use super::GPU_TILE_GRID_EMPTY;
use crate::constants::TILE_EDGE_LENGTH;

/// The one tile every harness frame uses.
const TILE_ID: u64 = 1;

pub const TARGET_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

pub struct TestGpu {
    device: wgpu::Device
    , queue: wgpu::Queue
    // Built once: compiling the wgsl is the expensive part, and every harness frame asks for
    // a slot reset anyway, so there is no state to carry between renders.
    , resources: Mutex<GpuDisplayResources>
}

impl TestGpu {
    fn new() -> Option<Self> {
        // Borrow the app's one device so the harness exercises exactly the
        // context the real display uses, rather than a lookalike of its own.
        let shared = crate::gpu_context::GpuContext::shared()?;
        let device = shared.device.clone();
        let queue = shared.queue.clone();
        let resources = Mutex::new(GpuDisplayResources::new(&device, TARGET_FORMAT));
        Some(Self { device, queue, resources })
    }

    /// Run sample+shade prepare/paint/submit without CPU readback (for frametime bars).
    /// Returns wall time of one paint+submit+gpu-wait after warm prepare (shader work).
    /// Texture alloc and prepare are outside the timed window.
    pub fn paint_frametime(&self, frame: &ShadeFrame) -> Duration {
        let width = frame.uniforms.viewport_size[0] as u32;
        let height = frame.uniforms.viewport_size[1] as u32;
        let mut resources = self.resources.lock().expect("shade parity resources poisoned");
        let target = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("shade_timing_target")
            , size: wgpu::Extent3d {
                width
                , height
                , depth_or_array_layers: 1
            }
            , mip_level_count: 1
            , sample_count: 1
            , dimension: wgpu::TextureDimension::D2
            , format: TARGET_FORMAT
            , usage: wgpu::TextureUsages::RENDER_ATTACHMENT
            , view_formats: &[]
        });
        let view = target.create_view(&wgpu::TextureViewDescriptor::default());
        // Prepare once (uploads / bind groups) — product frames reuse prepared state
        // across paints; including it every sample made the 2ms bar a host-tax lie.
        resources.prepare(&self.device, &self.queue, frame);
        let paint_only = |resources: &mut GpuDisplayResources| {
            let clear = resources.nores_clear();
            let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("shade_timing_encoder")
            });
            {
                let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("shade_timing_pass")
                    , color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &view
                        , resolve_target: None
                        , ops: wgpu::Operations {
                            // Far-from-tiles seats are constant nores; only the
                            // tile-grid scissor runs the sample+shade pipeline.
                            load: wgpu::LoadOp::Clear(clear)
                            , store: wgpu::StoreOp::Store
                        }
                    })]
                    , depth_stencil_attachment: None
                    , timestamp_writes: None
                    , occlusion_query_set: None
                }).forget_lifetime();
                resources.paint(&mut pass);
            }
            self.queue.submit([encoder.finish()]);
            let _ = self.device.poll(wgpu::PollType::Wait);
        };
        paint_only(&mut resources);
        let t0 = Instant::now();
        paint_only(&mut resources);
        let elapsed = t0.elapsed();
        std::hint::black_box(target.size());
        elapsed
    }

    /// Run the pipeline over one frame and read the target back, row major.
    pub fn render(&self, frame: &ShadeFrame) -> Vec<[u8; 3]> {
        let width = frame.uniforms.viewport_size[0] as u32;
        let height = frame.uniforms.viewport_size[1] as u32;
        let mut resources = self.resources.lock().expect("shade parity resources poisoned");
        resources.prepare(&self.device, &self.queue, frame);

        let target = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("shade_parity_target")
            , size: wgpu::Extent3d {
                width
                , height
                , depth_or_array_layers: 1
            }
            , mip_level_count: 1
            , sample_count: 1
            , dimension: wgpu::TextureDimension::D2
            , format: TARGET_FORMAT
            , usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC
            , view_formats: &[]
        });
        let view = target.create_view(&wgpu::TextureViewDescriptor::default());

        let unpadded = width as usize * 4;
        let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT as usize;
        let padded = unpadded.div_ceil(align) * align;
        let readback = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("shade_parity_readback")
            , size: (padded * height as usize) as u64
            , usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ
            , mapped_at_creation: false
        });

        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("shade_parity_encoder")
        });
        {
            let clear = resources.nores_clear();
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("shade_parity_pass")
                , color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view
                    , resolve_target: None
                    , ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(clear)
                        , store: wgpu::StoreOp::Store
                    }
                })]
                , depth_stencil_attachment: None
                , timestamp_writes: None
                , occlusion_query_set: None
            }).forget_lifetime();
            resources.paint(&mut pass);
        }
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &target
                , mip_level: 0
                , origin: wgpu::Origin3d::ZERO
                , aspect: wgpu::TextureAspect::All
            }
            , wgpu::TexelCopyBufferInfo {
                buffer: &readback
                , layout: wgpu::TexelCopyBufferLayout {
                    offset: 0
                    , bytes_per_row: Some(padded as u32)
                    , rows_per_image: Some(height)
                }
            }
            , wgpu::Extent3d {
                width
                , height
                , depth_or_array_layers: 1
            }
        );
        self.queue.submit([encoder.finish()]);

        let slice = readback.slice(..);
        slice.map_async(wgpu::MapMode::Read, |_| {});
        let _ = self.device.poll(wgpu::PollType::Wait);
        let mapped = slice.get_mapped_range();
        let mut pixels = Vec::with_capacity((width * height) as usize);
        for y in 0..height as usize {
            let row = &mapped[y * padded..y * padded + unpadded];
            for x in 0..width as usize {
                pixels.push([row[x * 4], row[x * 4 + 1], row[x * 4 + 2]]);
            }
        }
        drop(mapped);
        readback.unmap();
        pixels
    }
}

/// One device for the whole test binary. Standing up an adapter costs more than every
/// render put together, so it is not worth doing per test.
static SHARED: OnceLock<Option<TestGpu>> = OnceLock::new();

/// Grab the device, or say plainly why the test is not running.
///
/// Set CZ_REQUIRE_GPU=1 to turn a missing adapter into a failure, which is what a machine
/// with a gpu should do.
pub fn gpu_or_skip(test: &str) -> Option<&'static TestGpu> {
    match SHARED.get_or_init(TestGpu::new) {
        Some(gpu) => Some(gpu)
        , None => {
            if std::env::var("CZ_REQUIRE_GPU").as_deref() == Ok("1") {
                panic!("{test}: no wgpu adapter and CZ_REQUIRE_GPU=1");
            }
            eprintln!("{test}: skipped, no wgpu adapter available");
            None
        }
    }
}

/// A viewport of the given size, sitting on the unit grid at the origin, with the bailout
/// settings the escape phase tests want.
pub fn base_uniforms(size: (u32, u32)) -> ShadeUniforms {
    ShadeUniforms {
        viewport_size: [size.0 as f32, size.1 as f32]
        , seat_offset: [0, 0]
        , zoom_match: 1
        , instruction_count: 0
        , bailout_radius: 2.0
        , bailout_max_extra: 0
        , origin_re: 0.0
        , origin_im: 0.0
        , space: 0.0
        , tile_count: 1
        , grid_w: 1
        , grid_h: 1
        , nores_r: 0.0
        , nores_g: 0.0
        , nores_b: 0.0
        , edge_margin: 1
    , _pad_end: 0
    , _pad_end2: 0
    }
}

/// Wrap a patch of raw answers as the single tile covering the top left of the screen.
pub fn frame_from_grid(
    grid: &RawGrid
    , mut uniforms: ShadeUniforms
    , instructions: Vec<GpuInstruction>
) -> ShadeFrame {
    assert!(
        grid.size.0 <= TILE_EDGE_LENGTH as i32 && grid.size.1 <= TILE_EDGE_LENGTH as i32
        , "the harness carries one tile, so the answer patch must fit in {TILE_EDGE_LENGTH} square"
    );
    let mut meta = vec![[0.0f32; 4]; TILE_EDGE_LENGTH * TILE_EDGE_LENGTH];
    let mut z = vec![[0.0f32; 4]; TILE_EDGE_LENGTH * TILE_EDGE_LENGTH];
    for y in 0..TILE_EDGE_LENGTH as i32 {
        for x in 0..TILE_EDGE_LENGTH as i32 {
            let raw = grid.get((x, y));
            let index = y as usize * TILE_EDGE_LENGTH + x as usize;
            meta[index] = [raw.escape_or_period, raw.small_time, raw.smallness, raw.kind];
            z[index] = [raw.zx, raw.zy, 0.0, 0.0];
        }
    }

    uniforms.instruction_count = instructions.len() as u32;
    let nores = super::nores_rgb_for_instructions(&instructions);
    uniforms.nores_r = nores[0];
    uniforms.nores_g = nores[1];
    uniforms.nores_b = nores[2];
    uniforms.edge_margin = super::edge_margin_for_instructions(&instructions);
    uniforms.tile_count = 1;
    uniforms.grid_w = 1;
    uniforms.grid_h = 1;

    let mut tile_grid = vec![GPU_TILE_GRID_EMPTY; GPU_TILE_CELL_SLOTS as usize];
    tile_grid[0] = 0;

    ShadeFrame {
        uniforms
        , instructions
        , tile_entries: vec![GpuTileEntry {
            origin_x: 0
            , origin_y: 0
            , pan_x: 0
            , pan_y: 0
            , zoom_delta: 0
            , slot: 0
            , rank: 1
            , _pad: 0
        }]
        , entry_ids: vec![TILE_ID]
        , live_ids: vec![TILE_ID]
        , tile_grid
        , pending_uploads: vec![PendingTileUpload {
            id: TILE_ID
            , meta
            , z
            , production_slot: None
        }]
        , reset_gpu_slots: true
    }
}

struct SpinWaker;

impl Wake for SpinWaker {
    fn wake(self: Arc<Self>) {}
}

fn block_on<T>(future: impl Future<Output = T>) -> T {
    let waker = Waker::from(Arc::new(SpinWaker));
    let mut context = Context::from_waker(&waker);
    let mut future = Pin::from(Box::new(future));
    loop {
        match future.as_mut().poll(&mut context) {
            Poll::Ready(value) => return value
            , Poll::Pending => std::thread::yield_now()
        }
    }
}
