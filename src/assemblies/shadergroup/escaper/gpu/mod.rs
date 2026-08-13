//! Escaper-owned wgpu f32 bailout tail — R=2 → radius only; interiors pass-through.
//! Own device/queue (compartmentalized from the colorer). Resident answers;
//! radius uniform on anim ticks.

use crate::assemblies::shadergroup::colorer::gpu::wgpu_init_lock;
use crate::assemblies::shadergroup::escaper::{ScreenValue, ZoomerValuesScreen};
use crate::assemblies::workgroup::screen_worker::workshift::CompletedPoint;
use crate::assemblies::workgroup::work_collector::ResultsPackage;
use crate::settings::Settings;
use crate::utils::ObjectivePosAndZoom;
use bytemuck::{Pod, Zeroable};
use std::sync::{Arc, Mutex, OnceLock};

const SHADER: &str = include_str!("escape.wgsl");

static SHARED: OnceLock<Option<Arc<GpuEscaper>>> = OnceLock::new();

/// Host-side GPU answer row (upload pack). Public so benches can call `escape_frame`.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct GpuAnswer {
    pub kind: u32,
    pub escape_time: u32,
    pub small_time: u32,
    pub loop_period: u32,
    pub zr: f32,
    pub zi: f32,
    pub dcr: f32,
    pub dci: f32,
    pub cr: f32,
    pub ci: f32,
    pub smallness: f32,
    pub _pad: f32,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct GpuParams {
    radius_sq: f32,
    max_extra: u32,
    count: u32,
    _pad: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct GpuOut {
    kind: u32,
    big_time: u32,
    small_time: u32,
    loop_period: u32,
    smallness: f32,
    gradient_angle: f32,
    _pad0: f32,
    _pad1: f32,
}

struct EscapeSession {
    count: u32,
    answer_buf: wgpu::Buffer,
    params_buf: wgpu::Buffer,
    out_buf: wgpu::Buffer,
    staging: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    /// Reused only when convert did not supply a prepack.
    pack_scratch: Vec<GpuAnswer>,
}

pub struct GpuEscaper {
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipeline: wgpu::ComputePipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    /// Serialize submit+map on this actor's device only.
    ops_lock: Mutex<()>,
    session: Mutex<Option<EscapeSession>>,
}

impl GpuEscaper {
    pub fn shared() -> Option<Arc<GpuEscaper>> {
        SHARED
            .get_or_init(|| {
                let _g = wgpu_init_lock()
                    .lock()
                    .unwrap_or_else(|e| e.into_inner());
                Self::try_new().map(Arc::new)
            })
            .clone()
    }

    pub fn try_new() -> Option<Self> {
        if std::env::var("CZ_FORCE_CPU_ESCAPE").ok().as_deref() == Some("1") {
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
                label: Some("escaper_gpu"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                memory_hints: Default::default(),
                trace: Default::default(),
            })
            .await
            .ok()?;
        Self::try_new_on(device, queue)
    }

    pub fn try_new_on(device: wgpu::Device, queue: wgpu::Queue) -> Option<Self> {
        if std::env::var("CZ_FORCE_CPU_ESCAPE").ok().as_deref() == Some("1") {
            return None;
        }
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("escaper_escape_wgsl"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("escaper_bgl"),
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
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("escaper_pl"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("escaper_pipeline"),
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
            ops_lock: Mutex::new(()),
            session: Mutex::new(None),
        })
    }

    fn ensure_session(&self, count: u32) -> EscapeSession {
        let answer_bytes = (count as u64) * std::mem::size_of::<GpuAnswer>() as u64;
        let out_bytes = (count as u64) * std::mem::size_of::<GpuOut>() as u64;
        let answer_buf = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("escaper_answers"),
            size: answer_bytes,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let params_buf = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("escaper_params"),
            size: std::mem::size_of::<GpuParams>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let out_buf = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("escaper_out"),
            size: out_bytes,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let staging = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("escaper_staging"),
            size: out_bytes,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("escaper_bg"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: answer_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: params_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: out_buf.as_entire_binding(),
                },
            ],
        });
        EscapeSession {
            count,
            answer_buf,
            params_buf,
            out_buf,
            staging,
            bind_group,
            pack_scratch: Vec::with_capacity(count as usize),
        }
    }

    /// Escape a package. `upload_answers` false → resident buffer + radius-only uniform.
    /// `prepacked_answers` (from convert) skips a second host walk on the upload path.
    pub fn escape_frame<T>(
        &self,
        package: &ResultsPackage<T>,
        radius: f32,
        settings: &Settings,
        upload_answers: bool,
        prepacked_answers: Option<&[GpuAnswer]>,
    ) -> Option<ZoomerValuesScreen>
    where
        T: Into<f64> + Copy,
    {
        let _ops = self.ops_lock.lock().unwrap_or_else(|e| e.into_inner());
        let n = package.results.len() as u32;
        if n == 0 {
            return Some(ZoomerValuesScreen {
                values: Vec::new(),
                res: package.screen_res,
                objective_location: package.location.clone(),
                hud: package.hud,
            });
        }

        let mut session_guard = self.session.lock().unwrap_or_else(|e| e.into_inner());
        let need_realloc = match session_guard.as_ref() {
            Some(s) => s.count != n,
            None => true,
        };
        if need_realloc {
            *session_guard = Some(self.ensure_session(n));
        }
        let upload_answers = upload_answers || need_realloc;

        if upload_answers {
            let session = session_guard.as_mut()?;
            let bytes: &[u8] = match prepacked_answers {
                Some(pre) if pre.len() == n as usize => bytemuck::cast_slice(pre),
                _ => {
                    pack_answers_into(&mut session.pack_scratch, &package.results);
                    bytemuck::cast_slice(&session.pack_scratch)
                }
            };
            self.queue
                .write_buffer(&session.answer_buf, 0, bytes);
        }

        let params = GpuParams {
            radius_sq: radius * radius,
            max_extra: settings.bailout_max_additional_iterations,
            count: n,
            _pad: 0,
        };
        {
            let session = session_guard.as_ref()?;
            self.queue
                .write_buffer(&session.params_buf, 0, bytemuck::bytes_of(&params));
        }

        let out_size = (n as u64) * std::mem::size_of::<GpuOut>() as u64;
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("escaper_enc"),
            });
        {
            let session = session_guard.as_ref()?;
            {
                let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some("escaper_pass"),
                    timestamp_writes: None,
                });
                pass.set_pipeline(&self.pipeline);
                pass.set_bind_group(0, &session.bind_group, &[]);
                pass.dispatch_workgroups((n + 63) / 64, 1, 1);
            }
            encoder.copy_buffer_to_buffer(&session.out_buf, 0, &session.staging, 0, out_size);
        }
        self.queue.submit(Some(encoder.finish()));

        let session = session_guard.as_mut()?;
        let slice = session.staging.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |r| {
            let _ = tx.send(r);
        });
        self.device.poll(wgpu::PollType::Wait);
        rx.recv().ok()?.ok()?;
        let data = slice.get_mapped_range();
        let packed: &[GpuOut] = bytemuck::cast_slice(&data);
        let mut values = Vec::with_capacity(packed.len());
        for o in packed {
            values.push(match o.kind {
                0 => ScreenValue::Outside {
                    big_time: o.big_time,
                    small_time: o.small_time,
                    smallness: o.smallness as f64,
                    gradient_angle: o.gradient_angle,
                },
                _ => ScreenValue::Inside {
                    small_time: o.small_time,
                    loop_period: o.loop_period,
                    smallness: o.smallness as f64,
                },
            });
        }
        drop(data);
        session.staging.unmap();

        Some(ZoomerValuesScreen {
            values,
            res: package.screen_res,
            objective_location: package.location.clone(),
            hud: package.hud,
        })
    }
}

fn pack_answers_into<T: Into<f64> + Copy>(out: &mut Vec<GpuAnswer>, results: &[CompletedPoint<T>]) {
    out.clear();
    out.reserve(results.len());
    for p in results {
        out.push(match p {
            CompletedPoint::Escapes {
                escape_time,
                escape_location: z,
                escape_derivative: dc,
                start_location: c,
                smallness: s,
                small_time: st,
            } => GpuAnswer {
                kind: 0,
                escape_time: *escape_time,
                small_time: *st,
                loop_period: 0,
                zr: Into::<f64>::into(z.0) as f32,
                zi: Into::<f64>::into(z.1) as f32,
                dcr: Into::<f64>::into(dc.0) as f32,
                dci: Into::<f64>::into(dc.1) as f32,
                cr: Into::<f64>::into(c.0) as f32,
                ci: Into::<f64>::into(c.1) as f32,
                smallness: Into::<f64>::into(*s) as f32,
                _pad: 0.0,
            },
            CompletedPoint::Repeats {
                period,
                smallness: s,
                small_time: st,
            } => GpuAnswer {
                kind: 1,
                escape_time: 0,
                small_time: *st,
                loop_period: *period,
                zr: 0.0,
                zi: 0.0,
                dcr: 0.0,
                dci: 0.0,
                cr: 0.0,
                ci: 0.0,
                smallness: Into::<f64>::into(*s) as f32,
                _pad: 0.0,
            },
            CompletedPoint::Dummy {} => GpuAnswer {
                kind: 2,
                escape_time: 0,
                small_time: 0,
                loop_period: 0,
                zr: 0.0,
                zi: 0.0,
                dcr: 0.0,
                dci: 0.0,
                cr: 0.0,
                ci: 0.0,
                smallness: 100.0,
                _pad: 0.0,
            },
        });
    }
}

/// Prefer GPU when requested; else OG `escape_frame`.
pub fn escape_with_gear<T>(
    package: &ResultsPackage<T>,
    radius: f32,
    settings: &Settings,
    want_gpu: bool,
    gpu: &Option<Arc<GpuEscaper>>,
    upload_answers: bool,
    prepacked_answers: Option<&[GpuAnswer]>,
) -> (
    ZoomerValuesScreen,
    crate::assemblies::structs::EscaperHud,
)
where
    T: std::ops::Sub<Output = T>
        + std::ops::Add<Output = T>
        + std::ops::Mul<Output = T>
        + Into<f64>
        + PartialOrd
        + crate::assemblies::workgroup::screen_worker::workshift::Finite
        + crate::assemblies::workgroup::screen_worker::workshift::Gt
        + crate::assemblies::workgroup::screen_worker::workshift::Abs
        + From<f32>
        + Copy,
{
    use crate::assemblies::shadergroup::escaper::escape_frame;
    use crate::assemblies::structs::EscaperHud;
    if want_gpu {
        if let Some(g) = gpu {
            if let Some(out) =
                g.escape_frame(package, radius, settings, upload_answers, prepacked_answers)
            {
                return (out, EscaperHud::Gpu);
            }
            return (
                escape_frame(package, radius, settings),
                EscaperHud::GpuFallbackOg,
            );
        }
        return (
            escape_frame(package, radius, settings),
            EscaperHud::GpuFallbackOg,
        );
    }
    (
        escape_frame(package, radius, settings),
        EscaperHud::Og,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assemblies::shadergroup::escaper::escape_frame;
    use crate::assemblies::structs::EscaperHud;
    use crate::assemblies::workgroup::screen_worker::workshift::{
        from_stencil, workshift_with_kernel, DirectKernel,
    };
    use crate::constants::HOME_POSITION;
    use crate::utils::IntExp;

    fn home_package(res: (u32, u32)) -> ResultsPackage<f64> {
        let home = ObjectivePosAndZoom {
            pos: (
                IntExp::from(HOME_POSITION.0),
                IntExp::from(HOME_POSITION.1),
            ),
            zoom_pot: HOME_POSITION.2,
        };
        let mut ctx = from_stencil((home.clone(), res), None).expect("home");
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
        ResultsPackage {
            results,
            screen_res: res,
            location: home,
            hud: Default::default(),
        }
    }

    fn screen_values_match_gpu_f32(a: &ZoomerValuesScreen, b: &ZoomerValuesScreen) {
        assert_eq!(a.res, b.res);
        assert_eq!(a.values.len(), b.values.len());
        for (i, (av, bv)) in a.values.iter().zip(b.values.iter()).enumerate() {
            match (av, bv) {
                (
                    ScreenValue::Outside {
                        big_time: at,
                        small_time: ast,
                        smallness: as_,
                        gradient_angle: ag,
                    },
                    ScreenValue::Outside {
                        big_time: bt,
                        small_time: bst,
                        smallness: bs,
                        gradient_angle: bg,
                    },
                ) => {
                    assert_eq!(at, bt, "big_time @{i}");
                    assert_eq!(ast, bst, "small_time @{i}");
                    assert!(
                        (as_ - bs).abs() < 1e-4,
                        "smallness @{i}: {as_} vs {bs}"
                    );
                    assert!(
                        (ag - bg).abs() < 1e-4,
                        "gradient @{i}: {ag} vs {bg}"
                    );
                }
                (
                    ScreenValue::Inside {
                        small_time: ast,
                        loop_period: ap,
                        smallness: as_,
                    },
                    ScreenValue::Inside {
                        small_time: bst,
                        loop_period: bp,
                        smallness: bs,
                    },
                ) => {
                    assert_eq!(ast, bst, "in small_time @{i}");
                    assert_eq!(ap, bp, "period @{i}");
                    assert!(
                        (as_ - bs).abs() < 1e-4,
                        "in smallness @{i}: {as_} vs {bs}"
                    );
                }
                _ => panic!("kind mismatch at {i}"),
            }
        }
    }

    /// CPU f32 twin of the WGSL bailout tail (oracle for GPU; OG stays f64).
    fn escape_frame_f32_ref(
        package: &ResultsPackage<f64>,
        radius: f32,
        settings: &Settings,
    ) -> ZoomerValuesScreen {
        let r2 = radius * radius;
        let max_extra = settings.bailout_max_additional_iterations;
        let mut values = Vec::with_capacity(package.results.len());
        for p in &package.results {
            match p {
                CompletedPoint::Repeats {
                    period,
                    smallness: s,
                    small_time: st,
                } => values.push(ScreenValue::Inside {
                    small_time: *st,
                    loop_period: *period,
                    smallness: *s,
                }),
                CompletedPoint::Dummy {} => values.push(ScreenValue::Inside {
                    small_time: 0,
                    loop_period: 0,
                    smallness: 100.0,
                }),
                CompletedPoint::Escapes {
                    escape_time,
                    escape_location: z,
                    escape_derivative: dc,
                    start_location: c,
                    smallness: s,
                    small_time: st,
                } => {
                    let mut zr = z.0 as f32;
                    let mut zi = z.1 as f32;
                    let mut dcr = dc.0 as f32;
                    let mut dci = dc.1 as f32;
                    let cr = c.0 as f32;
                    let ci = c.1 as f32;
                    let mut iters = *escape_time;
                    let mut rs = zr * zr;
                    let mut is_ = zi * zi;
                    let mut ri = zr * zi;
                    let mut extra = 0u32;
                    while rs + is_ <= r2 && extra < max_extra {
                        let d_new_r = 2.0 * (zr * dcr - zi * dci) + 1.0;
                        let d_new_i = 2.0 * (zr * dci + zi * dcr);
                        let nzr = rs - is_ + cr;
                        let nzi = 2.0 * ri + ci;
                        zr = nzr;
                        zi = nzi;
                        dcr = d_new_r;
                        dci = d_new_i;
                        iters += 1;
                        rs = zr * zr;
                        is_ = zi * zi;
                        ri = zr * zi;
                        extra += 1;
                    }
                    let gradient_angle =
                        (-(zi * dcr - zr * dci)).atan2(zr * dcr + zi * dci);
                    values.push(ScreenValue::Outside {
                        big_time: iters,
                        small_time: *st,
                        smallness: *s,
                        gradient_angle,
                    });
                }
            }
        }
        ZoomerValuesScreen {
            values,
            res: package.screen_res,
            objective_location: package.location.clone(),
            hud: package.hud,
        }
    }

    #[test]
    fn gpu_escape_matches_og_home() {
        let _wgpu = crate::debug_agent::WgpuTestLock::acquire();
        let Some(gpu) = GpuEscaper::shared() else {
            let pkg = home_package((32, 18));
            let settings = Settings::DEFAULT;
            let (out, hud) =
                escape_with_gear(&pkg, 2.0, &settings, true, &None, true, None);
            assert_eq!(hud, EscaperHud::GpuFallbackOg);
            assert_eq!(out.values.len(), pkg.results.len());
            return;
        };
        let pkg = home_package((64, 36));
        let settings = Settings::DEFAULT;
        for &radius in &[2.0f32, 4.0, 16.0, 256.0] {
            let og = escape_frame(&pkg, radius, &settings);
            let f32_ref = escape_frame_f32_ref(&pkg, radius, &settings);
            let (gpu_out, hud) =
                escape_with_gear(&pkg, radius, &settings, true, &Some(gpu.clone()), true, None);
            assert_eq!(hud, EscaperHud::Gpu);
            // Exact vs f32 reference (WGSL twin).
            screen_values_match_gpu_f32(&gpu_out, &f32_ref);
            // big_time must match OG (same max_extra / bailout policy).
            for (i, (g, o)) in gpu_out.values.iter().zip(og.values.iter()).enumerate() {
                match (g, o) {
                    (
                        ScreenValue::Outside { big_time: gt, .. },
                        ScreenValue::Outside { big_time: ot, .. },
                    ) => assert_eq!(gt, ot, "OG big_time @{i} r={radius}"),
                    (
                        ScreenValue::Inside { loop_period: gp, .. },
                        ScreenValue::Inside { loop_period: op, .. },
                    ) => assert_eq!(gp, op, "OG period @{i}"),
                    _ => panic!("kind mismatch vs OG @{i}"),
                }
            }
        }
    }

    #[test]
    fn gpu_escape_radius_only_matches_reupload() {
        let _wgpu = crate::debug_agent::WgpuTestLock::acquire();
        let Some(gpu) = GpuEscaper::shared() else {
            return;
        };
        let pkg = home_package((48, 27));
        let settings = Settings::DEFAULT;
        let first = gpu
            .escape_frame(&pkg, 2.0, &settings, true, None)
            .expect("upload");
        let radius_only = gpu
            .escape_frame(&pkg, 32.0, &settings, false, None)
            .expect("radius");
        let reupload = gpu
            .escape_frame(&pkg, 32.0, &settings, true, None)
            .expect("reupload");
        screen_values_match_gpu_f32(&radius_only, &reupload);
        assert_eq!(first.values.len(), radius_only.values.len());
    }
}
