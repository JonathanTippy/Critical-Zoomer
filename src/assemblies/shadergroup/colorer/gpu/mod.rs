//! Colorer-owned wgpu f32 path — honest port of `color.rs` with exact Color32 parity.
//! Persistent buffers + realloc-on-resize; each paint refreshes from current inputs.
//! Fallback to OG only when no usable device exists (never for missing f64).
// r[impl cz.craft.gpu-color-parity+1]
// r[impl cz.shade.layers-in-script-order+1]

use crate::assemblies::shadergroup::colorer::color::color as color_og;
use crate::assemblies::shadergroup::escaper::{ScreenValue, ZoomerValuesScreen};
use crate::settings::{ColoringInstruction, Normalizing, Settings, Shading};
use bytemuck::{Pod, Zeroable};
use egui::Color32;
use std::sync::{Arc, Mutex, OnceLock};

const MAX_LAYERS: usize = 16;
const SHADER: &str = include_str!("color.wgsl");

/// Process-wide colorer GPU (one device). Avoids parallel `try_new` races with
/// other wgpu users under libtest. Escaper owns a separate device
/// (compartmentalized); both serialize init via [`wgpu_init_lock`].
static SHARED_GPU: OnceLock<Option<Arc<GpuColorer>>> = OnceLock::new();
static INIT_LOCK: Mutex<()> = Mutex::new(());

/// Process-wide lock for wgpu adapter/device init (libtest + multi-actor startup).
pub fn wgpu_init_lock() -> &'static Mutex<()> {
    &INIT_LOCK
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct GpuPixel {
    kind: u32,
    big_time: u32,
    small_time: u32,
    loop_period: u32,
    smallness: f32,
    gradient_angle: f32,
    _pad0: f32,
    _pad1: f32,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct GpuLayer {
    kind: u32,
    opacity_in: u32,
    opacity_out: u32,
    color_r: u32,
    color_g: u32,
    color_b: u32,
    range_u8: u32,
    shading: u32,
    normalizing: u32,
    thickness: u32,
    period: f32,
    period_recip: f32,
    phase: f32,
    range_f: f32,
    _pad0: f32,
    _pad1: f32,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct GpuFrame {
    width: u32,
    height: u32,
    layer_count: u32,
    _pad: u32,
}

struct ColorSession {
    pixel_count: u32,
    pixel_buf: wgpu::Buffer,
    frame_buf: wgpu::Buffer,
    layer_buf: wgpu::Buffer,
    out_buf: wgpu::Buffer,
    staging: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    last_colors: Vec<Color32>,
}

pub struct GpuColorer {
    pub(crate) device: wgpu::Device,
    pub(crate) queue: wgpu::Queue,
    pipeline: wgpu::ComputePipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    /// Serialize paint on this actor's device (map_async is not re-entrant).
    paint_lock: Mutex<()>,
    session: Mutex<Option<ColorSession>>,
}

impl GpuColorer {
    /// Shared device for actor + tests (init once).
    pub fn shared() -> Option<Arc<GpuColorer>> {
        SHARED_GPU
            .get_or_init(|| {
                let _g = INIT_LOCK.lock().unwrap_or_else(|e| e.into_inner());
                Self::try_new().map(Arc::new)
            })
            .clone()
    }

    pub fn try_new() -> Option<Self> {
        if std::env::var("CZ_FORCE_CPU_COLOR").ok().as_deref() == Some("1") {
            return None;
        }
        pollster::block_on(Self::try_new_async())
    }

    async fn try_new_async() -> Option<Self> {
        let backend_attempts = [
            wgpu::Backends::VULKAN,
            wgpu::Backends::GL,
            wgpu::Backends::PRIMARY,
        ];
        let mut adapter = None;
        for backends in backend_attempts {
            let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
                backends,
                ..Default::default()
            });
            if let Ok(a) = instance
                .request_adapter(&wgpu::RequestAdapterOptions {
                    power_preference: wgpu::PowerPreference::HighPerformance,
                    compatible_surface: None,
                    force_fallback_adapter: false,
                })
                .await
            {
                adapter = Some(a);
                break;
            }
        }
        let adapter = adapter?;
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("colorer_gpu"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                memory_hints: Default::default(),
                trace: Default::default(),
            })
            .await
            .ok()?;

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("colorer_color_wgsl"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("colorer_bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("colorer_pl"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("colorer_pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });

        Some(Self {
            device,
            queue,
            pipeline,
            bind_group_layout,
            paint_lock: Mutex::new(()),
            session: Mutex::new(None),
        })
    }

    fn ensure_session(&self, pixel_count: u32) -> ColorSession {
        let pixel_bytes = (pixel_count as u64) * std::mem::size_of::<GpuPixel>() as u64;
        let out_size = (pixel_count as u64) * 4;
        let layer_bytes = (MAX_LAYERS as u64) * std::mem::size_of::<GpuLayer>() as u64;

        let pixel_buf = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("colorer_pixels"),
            size: pixel_bytes,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let frame_buf = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("colorer_frame"),
            size: std::mem::size_of::<GpuFrame>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let layer_buf = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("colorer_layers"),
            size: layer_bytes,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let out_buf = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("colorer_out"),
            size: out_size,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let staging = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("colorer_staging"),
            size: out_size,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("colorer_bg"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: pixel_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: frame_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: layer_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: out_buf.as_entire_binding(),
                },
            ],
        });
        ColorSession {
            pixel_count,
            pixel_buf,
            frame_buf,
            layer_buf,
            out_buf,
            staging,
            bind_group,
            last_colors: Vec::new(),
        }
    }

    /// Paint with the GPU path. Returns `None` only on buffer/map failure (caller falls back).
    /// Always refreshes pixel + layer uploads from current inputs (persistent buffers).
    pub fn paint(
        &self,
        values: &ZoomerValuesScreen,
        settings: &mut Settings,
    ) -> Option<Vec<Color32>> {
        let _paint = self.paint_lock.lock().unwrap_or_else(|e| e.into_inner());
        let mut session_guard = self.session.lock().unwrap_or_else(|e| e.into_inner());

        let n = values.values.len() as u32;
        if n == 0 {
            return Some(Vec::new());
        }

        let need_realloc = match session_guard.as_ref() {
            Some(s) => s.pixel_count != n,
            None => true,
        };
        if need_realloc {
            *session_guard = Some(self.ensure_session(n));
        }

        let (pixels, layers, layer_count) = pack_frame(values, settings)?;
        let frame = GpuFrame {
            width: values.res.0,
            height: values.res.1,
            layer_count,
            _pad: 0,
        };

        let session = session_guard.as_mut()?;
        self.queue
            .write_buffer(&session.pixel_buf, 0, bytemuck::cast_slice(&pixels));
        self.queue
            .write_buffer(&session.frame_buf, 0, bytemuck::bytes_of(&frame));
        self.queue
            .write_buffer(&session.layer_buf, 0, bytemuck::cast_slice(&layers));

        let out_size = (n as u64) * 4;
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("colorer_enc"),
            });
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("colorer_pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &session.bind_group, &[]);
            let groups = (n + 63) / 64;
            pass.dispatch_workgroups(groups, 1, 1);
        }
        encoder.copy_buffer_to_buffer(&session.out_buf, 0, &session.staging, 0, out_size);
        self.queue.submit(Some(encoder.finish()));

        let slice = session.staging.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |r| {
            let _ = tx.send(r);
        });
        self.device.poll(wgpu::PollType::Wait);
        rx.recv().ok()?.ok()?;
        let data = slice.get_mapped_range();
        let packed: &[u32] = bytemuck::cast_slice(&data);
        let mut out = Vec::with_capacity(packed.len());
        for p in packed {
            let r = (p & 0xff) as u8;
            let g = ((p >> 8) & 0xff) as u8;
            let b = ((p >> 16) & 0xff) as u8;
            out.push(Color32::from_rgb(r, g, b));
        }
        drop(data);
        session.staging.unmap();
        session.last_colors = out.clone();
        Some(out)
    }
}

fn pack_frame(
    values: &ZoomerValuesScreen,
    settings: &mut Settings,
) -> Option<(Vec<GpuPixel>, [GpuLayer; MAX_LAYERS], u32)> {
    let mut pixels = Vec::with_capacity(values.values.len());
    for v in &values.values {
        pixels.push(match v {
            ScreenValue::Outside {
                big_time,
                small_time,
                smallness,
                gradient_angle,
            } => GpuPixel {
                kind: 0,
                big_time: *big_time,
                small_time: *small_time,
                loop_period: 0,
                smallness: *smallness as f32,
                gradient_angle: *gradient_angle,
                _pad0: 0.0,
                _pad1: 0.0,
            },
            ScreenValue::Inside {
                small_time,
                loop_period,
                smallness,
            } => GpuPixel {
                kind: 1,
                big_time: 0,
                small_time: *small_time,
                loop_period: *loop_period,
                smallness: *smallness as f32,
                gradient_angle: 0.0,
                _pad0: 0.0,
                _pad1: 0.0,
            },
        });
    }

    let mut layers = [GpuLayer::zeroed(); MAX_LAYERS];
    let script = settings.coloring_script.as_mut()?;
    if script.len() > MAX_LAYERS {
        return None;
    }
    let mut count = 0u32;
    for (i, instruction) in script.iter_mut().enumerate() {
        layers[i] = encode_layer(instruction)?;
        count += 1;
    }
    Some((pixels, layers, count))
}

fn encode_layer(instruction: &mut ColoringInstruction) -> Option<GpuLayer> {
    let norm_code = |n: Normalizing| -> u32 {
        match n {
            Normalizing::None { .. } => 0,
            Normalizing::LnLn { .. } => 1,
            Normalizing::Ln { .. } => 2,
            Normalizing::Reciprocal { .. } => 3,
            Normalizing::RecipLn { .. } => 4,
        }
    };
    let shade_code = |s: Shading| -> u32 {
        match s {
            Shading::Modular { .. } => 0,
            Shading::Sinus { .. } => 1,
        }
    };
    Some(match instruction {
        ColoringInstruction::PaintEscapeTime {
            opacity,
            color,
            range,
            shading_method,
            normalizing_method,
            ..
        } => {
            let period = shading_method.period.determine() as f32;
            let phase = shading_method.phase.determine() as f32;
            GpuLayer {
                kind: 0,
                opacity_in: 0,
                opacity_out: *opacity as u32,
                color_r: color.0 as u32,
                color_g: color.1 as u32,
                color_b: color.2 as u32,
                range_u8: *range as u32,
                shading: shade_code(shading_method.shading),
                normalizing: norm_code(*normalizing_method),
                thickness: 0,
                period,
                period_recip: 1.0 / period,
                phase,
                range_f: *range as f32 / 255.0,
                _pad0: 0.0,
                _pad1: 0.0,
            }
        }
        ColoringInstruction::PaintSmallTime {
            inside_opacity,
            outside_opacity,
            color,
            range,
            shading_method,
            normalizing_method,
            ..
        } => {
            let period = shading_method.period.determine() as f32;
            let phase = shading_method.phase.determine() as f32;
            GpuLayer {
                kind: 1,
                opacity_in: *inside_opacity as u32,
                opacity_out: *outside_opacity as u32,
                color_r: color.0 as u32,
                color_g: color.1 as u32,
                color_b: color.2 as u32,
                range_u8: *range as u32,
                shading: shade_code(shading_method.shading),
                normalizing: norm_code(*normalizing_method),
                thickness: 0,
                period,
                period_recip: 1.0 / period,
                phase,
                range_f: *range as f32 / 255.0,
                _pad0: 0.0,
                _pad1: 0.0,
            }
        }
        ColoringInstruction::PaintSmallness {
            inside_opacity,
            outside_opacity,
            color,
            range,
            shading_method,
            normalizing_method,
            ..
        } => {
            let period = shading_method.period.determine() as f32;
            let phase = shading_method.phase.determine() as f32;
            GpuLayer {
                kind: 2,
                opacity_in: *inside_opacity as u32,
                opacity_out: *outside_opacity as u32,
                color_r: color.0 as u32,
                color_g: color.1 as u32,
                color_b: color.2 as u32,
                range_u8: *range as u32,
                shading: shade_code(shading_method.shading),
                normalizing: norm_code(*normalizing_method),
                thickness: 0,
                period,
                period_recip: 1.0 / period,
                phase,
                range_f: *range as f32 / 255.0,
                _pad0: 0.0,
                _pad1: 0.0,
            }
        }
        ColoringInstruction::HighlightInFilaments {
            opacity, color, ..
        } => GpuLayer {
            kind: 3,
            opacity_in: 0,
            opacity_out: *opacity as u32,
            color_r: color.0 as u32,
            color_g: color.1 as u32,
            color_b: color.2 as u32,
            range_u8: 0,
            shading: 0,
            normalizing: 0,
            thickness: 0,
            period: 1.0,
            period_recip: 1.0,
            phase: 0.0,
            range_f: 0.0,
            _pad0: 0.0,
            _pad1: 0.0,
        },
        ColoringInstruction::HighlightOutFilaments {
            opacity, color, ..
        } => GpuLayer {
            kind: 4,
            opacity_in: 0,
            opacity_out: *opacity as u32,
            color_r: color.0 as u32,
            color_g: color.1 as u32,
            color_b: color.2 as u32,
            range_u8: 0,
            shading: 0,
            normalizing: 0,
            thickness: 0,
            period: 1.0,
            period_recip: 1.0,
            phase: 0.0,
            range_f: 0.0,
            _pad0: 0.0,
            _pad1: 0.0,
        },
        ColoringInstruction::HighlightNodes {
            inside_opacity,
            outside_opacity,
            color,
            thickness,
            ..
        } => GpuLayer {
            kind: 5,
            opacity_in: *inside_opacity as u32,
            opacity_out: *outside_opacity as u32,
            color_r: color.0 as u32,
            color_g: color.1 as u32,
            color_b: color.2 as u32,
            range_u8: 0,
            shading: 0,
            normalizing: 0,
            thickness: *thickness as u32,
            period: 1.0,
            period_recip: 1.0,
            phase: 0.0,
            range_f: 0.0,
            _pad0: 0.0,
            _pad1: 0.0,
        },
        ColoringInstruction::HighlightSmallTimeEdges {
            inside_opacity,
            outside_opacity,
            color,
            ..
        } => GpuLayer {
            kind: 6,
            opacity_in: *inside_opacity as u32,
            opacity_out: *outside_opacity as u32,
            color_r: color.0 as u32,
            color_g: color.1 as u32,
            color_b: color.2 as u32,
            range_u8: 0,
            shading: 0,
            normalizing: 0,
            thickness: 0,
            period: 1.0,
            period_recip: 1.0,
            phase: 0.0,
            range_f: 0.0,
            _pad0: 0.0,
            _pad1: 0.0,
        },
    })
}

/// True when any coloring-script Animable will change numbers this wake.
pub fn coloring_script_animated(settings: &Settings) -> bool {
    let Some(script) = settings.coloring_script.as_ref() else {
        return false;
    };
    script.iter().any(|instruction| match instruction {
        ColoringInstruction::PaintEscapeTime {
            shading_method, ..
        }
        | ColoringInstruction::PaintSmallTime {
            shading_method, ..
        }
        | ColoringInstruction::PaintSmallness {
            shading_method, ..
        } => shading_method.period.animated || shading_method.phase.animated,
        _ => false,
    })
}

/// Public entry: prefer GPU when available; else OG. Returns (pixels, hud stamp).
pub fn color_with_gear(
    values: &ZoomerValuesScreen,
    settings: &mut Settings,
    want_gpu: bool,
    gpu: &Option<Arc<GpuColorer>>,
) -> (Vec<Color32>, crate::assemblies::structs::ColorerHud) {
    use crate::assemblies::structs::ColorerHud;
    if want_gpu {
        if let Some(g) = gpu {
            if let Some(out) = g.paint(values, settings) {
                return (out, ColorerHud::Gpu);
            }
            let out = color_og(values, settings);
            return (out, ColorerHud::GpuFallbackOg);
        }
        let out = color_og(values, settings);
        return (out, ColorerHud::GpuFallbackOg);
    }
    (color_og(values, settings), ColorerHud::Og)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assemblies::structs::ColorerHud;
    use crate::settings::{DEFAULT_COLORING_SCRIPT, Settings};
    use crate::utils::ObjectivePosAndZoom;
    use std::time::Instant;

    fn tiny_screen() -> ZoomerValuesScreen {
        // 3×3: exterior ridge candidate + interior period step.
        let outside = |big_time: u32| ScreenValue::Outside {
            big_time,
            small_time: 1,
            smallness: 0.5,
            gradient_angle: 0.0,
        };
        let mut values = Vec::with_capacity(9);
        for _ in 0..9 {
            values.push(outside(10));
        }
        values[1] = outside(20);
        values[4] = ScreenValue::Inside {
            small_time: 3,
            loop_period: 2,
            smallness: 0.01,
        };
        values[3] = ScreenValue::Inside {
            small_time: 1,
            loop_period: 1,
            smallness: 0.02,
        };
        values[5] = ScreenValue::Inside {
            small_time: 1,
            loop_period: 1,
            smallness: 0.02,
        };
        ZoomerValuesScreen {
            values,
            res: (3, 3),
            objective_location: ObjectivePosAndZoom {
                pos: (crate::utils::IntExp::ZERO, crate::utils::IntExp::ZERO),
                zoom_pot: 0,
            },
            hud: Default::default(),
        }
    }

    // r[verify cz.shade.layers-in-script-order+1]
    // r[verify cz.craft.gpu-color-parity+1]
    #[test]
    fn gpu_matches_og_color32_default_script() {
        let _wgpu = crate::assemblies::workgroup::screen_worker::naive_gpu::lock_gpu_tests();
        let Some(gpu) = GpuColorer::shared() else {
            // No device — gear must fall back without panic.
            let mut settings = Settings::DEFAULT;
            settings.coloring_script = Some(DEFAULT_COLORING_SCRIPT.to_vec());
            let screen = tiny_screen();
            let (out, hud) = color_with_gear(&screen, &mut settings, true, &None);
            assert_eq!(hud, ColorerHud::GpuFallbackOg);
            assert_eq!(out.len(), 9);
            return;
        };
        let mut settings_og = Settings::DEFAULT;
        settings_og.coloring_script = Some(DEFAULT_COLORING_SCRIPT.to_vec());
        let mut settings_gpu = settings_og.clone();
        let screen = tiny_screen();
        let og = color_og(&screen, &mut settings_og);
        let (gpu_out, hud) =
            color_with_gear(&screen, &mut settings_gpu, true, &Some(gpu));
        assert_eq!(hud, ColorerHud::Gpu);
        assert_eq!(
            gpu_out, og,
            "GPU colorer must match OG Color32 for the same inputs"
        );
    }

    #[test]
    fn default_gear_is_gpu() {
        let _wgpu = crate::assemblies::workgroup::screen_worker::naive_gpu::lock_gpu_tests();
        let mut settings = Settings::DEFAULT;
        settings.coloring_script = Some(DEFAULT_COLORING_SCRIPT.to_vec());
        let screen = tiny_screen();
        let gpu = GpuColorer::shared();
        let want_gpu = matches!(
            settings.resolved_color_gear(),
            crate::assemblies::structs::ColorerMode::Gpu
        );
        let (out, hud) = color_with_gear(&screen, &mut settings, want_gpu, &gpu);
        if gpu.is_some() {
            assert_eq!(hud, ColorerHud::Gpu);
        } else {
            assert_eq!(hud, ColorerHud::GpuFallbackOg);
        }
        assert_eq!(out.len(), 9);
    }

    #[test]
    fn gpu_matches_og_per_layer_scripts() {
        let _wgpu = crate::assemblies::workgroup::screen_worker::naive_gpu::lock_gpu_tests();
        let Some(gpu) = GpuColorer::shared() else {
            return;
        };
        let screen = tiny_screen();
        for instr in DEFAULT_COLORING_SCRIPT.iter().cloned() {
            let mut settings_og = Settings::DEFAULT;
            settings_og.coloring_script = Some(vec![instr.clone()]);
            let mut settings_gpu = settings_og.clone();
            let og = color_og(&screen, &mut settings_og);
            let gpu_out = gpu
                .paint(&screen, &mut settings_gpu)
                .expect("gpu paint");
            assert_eq!(
                gpu_out, og,
                "layer {:?} must match OG",
                std::mem::discriminant(&instr)
            );
        }
    }

    // r[verify cz.shade.layers-in-script-order+1]
    #[test]
    fn gpu_matches_og_home_escape_frame() {
        let _wgpu = crate::assemblies::workgroup::screen_worker::naive_gpu::lock_gpu_tests();
        use crate::assemblies::shadergroup::escaper::escape_frame;
        use crate::assemblies::workgroup::screen_worker::workshift::{
            from_stencil, workshift_with_kernel, CompletedPoint, DirectKernel,
        };
        use crate::assemblies::workgroup::work_collector::ResultsPackage;
        use crate::constants::HOME_POSITION;
        use crate::utils::IntExp;

        let Some(gpu) = GpuColorer::shared() else {
            return;
        };
        let home = ObjectivePosAndZoom {
            pos: (
                IntExp::from(HOME_POSITION.0),
                IntExp::from(HOME_POSITION.1),
            ),
            zoom_pot: HOME_POSITION.2,
        };
        let res = (64u32, 36u32);
        let mut ctx: crate::assemblies::workgroup::screen_worker::workshift::WorkContext<f64> =
            from_stencil((home.clone(), res), None).expect("home");
        while !ctx.points.iter().all(|p| p.delivered) {
            workshift_with_kernel(0, 0, 0, 0, &mut ctx, &DirectKernel);
            while ctx.completed_points.pop().is_some() {}
        }
        let mut results = Vec::with_capacity(ctx.points.len());
        for p in &ctx.points {
            results.push(if p.repeats {
                CompletedPoint::Repeats {
                    period: p.period,
                    smallness: p.smallness_squared,
                    small_time: p.small_time,
                }
            } else if p.escapes {
                CompletedPoint::Escapes {
                    escape_time: p.iterations,
                    escape_location: (p.z.0, p.z.1),
                    escape_derivative: p.dc,
                    start_location: (p.c.0, p.c.1),
                    smallness: p.smallness_squared,
                    small_time: p.small_time,
                }
            } else {
                CompletedPoint::Dummy {}
            });
        }
        let package = ResultsPackage {
            results,
            screen_res: res,
            location: home,
            hud: Default::default(),
        };
        let mut settings = Settings::DEFAULT;
        settings.coloring_script = Some(DEFAULT_COLORING_SCRIPT.to_vec());
        let radius = settings.bailout_radius.determine() as f32;
        let screen = escape_frame(&package, radius, &settings);
        let mut settings_og = settings.clone();
        let mut settings_gpu = settings.clone();
        let og = color_og(&screen, &mut settings_og);
        let gpu_out = gpu
            .paint(&screen, &mut settings_gpu)
            .expect("gpu");
        assert_eq!(gpu_out, og, "home escape_frame GPU must match OG Color32");
    }

    #[test]
    fn gpu_repeated_paint_is_stable() {
        let _wgpu = crate::assemblies::workgroup::screen_worker::naive_gpu::lock_gpu_tests();
        let Some(gpu) = GpuColorer::shared() else {
            return;
        };
        let screen = tiny_screen();
        let mut settings = Settings::DEFAULT;
        settings.coloring_script = Some(DEFAULT_COLORING_SCRIPT.to_vec());
        let first = gpu.paint(&screen, &mut settings).expect("first");
        let second = gpu.paint(&screen, &mut settings).expect("second");
        assert_eq!(first, second);
    }

    /// Shade steady-state pin: under value-changing wake cadence (bailout-anim
    /// style), persistent GPU paint must stay fast enough that a small channel
    /// would not force unbounded coalesce drops (mechanical sympathy).
    #[test]
    fn steady_state_gpu_color_anim_keeps_up() {
        let _wgpu = crate::assemblies::workgroup::screen_worker::naive_gpu::lock_gpu_tests();
        let Some(gpu) = GpuColorer::shared() else {
            return;
        };
        let mut screen = tiny_screen();
        // Upscale to a real-ish frame so residency matters.
        let res = (320u32, 180u32);
        let n = (res.0 * res.1) as usize;
        screen.res = res;
        screen.values = (0..n)
            .map(|_| ScreenValue::Outside {
                big_time: 10,
                small_time: 1,
                smallness: 0.5,
                gradient_angle: 0.0,
            })
            .collect();
        let mut settings = Settings::DEFAULT;
        settings.coloring_script = Some(DEFAULT_COLORING_SCRIPT.to_vec());

        // Warm session.
        let _ = gpu
            .paint(&screen, &mut settings)
            .expect("warm");

        const WAKES: u32 = 40;
        let t0 = Instant::now();
        for i in 0..WAKES {
            // Bailout anim changes values every wake.
            if let ScreenValue::Outside { big_time, .. } = &mut screen.values[0] {
                *big_time = 10 + i;
            }
            let out = gpu
                .paint(&screen, &mut settings)
                .expect("anim paint");
            assert_eq!(out.len(), n);
        }
        let elapsed = t0.elapsed();
        // 8ms wake budget: 40 wakes → 320ms wall if each paint ≤ wake. Allow 2×
        // slack for CI noise; still fails hard if we recreate/upload every time
        // at this res.
        assert!(
            elapsed.as_millis() < 640,
            "GPU anim paints too slow for shade drain ({:?} for {WAKES} wakes)",
            elapsed
        );
    }
}
