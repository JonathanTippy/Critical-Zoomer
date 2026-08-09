/// Wide enough that shallow home amortizes one upload/harvest sync over many seats.
pub const MAX_WAVE: u32 = 32768;
/// Soft target for iterate-heavy compact maps; shallow floods may copy the full wave.
pub const SPARSE_FINISH_CAP: u32 = 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GpuPrecision {
    F32,
    F64,
}

pub struct NaiveGpuContext {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub precision: GpuPrecision,
    pub generation: u32,
    wave_n: u32,
    /// WIP count of the last submitted dispatch (for sparse finish map).
    pub(crate) last_wip_count: std::sync::atomic::AtomicU32,
    pub(crate) pipeline: wgpu::ComputePipeline,
    pub(crate) bind_group_layout: wgpu::BindGroupLayout,
    pub(crate) bind_group: wgpu::BindGroup,
    /// Optional F64 gear for mid-session escalate when F32 collapses seats.
    f64_pipeline: Option<wgpu::ComputePipeline>,
    f64_bind_group_layout: Option<wgpu::BindGroupLayout>,
    f64_bind_group: Option<wgpu::BindGroup>,
    f32_pipeline: Option<wgpu::ComputePipeline>,
    f32_bind_group_layout: Option<wgpu::BindGroupLayout>,
    f32_bind_group: Option<wgpu::BindGroup>,
    pub(crate) seats_buf: wgpu::Buffer,
    pub(crate) finishes_buf: wgpu::Buffer,
    pub(crate) finish_count_buf: wgpu::Buffer,
    pub(crate) iter_total_buf: wgpu::Buffer,
    pub(crate) params_buf: wgpu::Buffer,
    pub(crate) finish_staging: wgpu::Buffer,
    pub(crate) seat_staging: wgpu::Buffer,
    /// Ping-pong sparse harvest staging so a shallow re-upload can submit the next
    /// wave before mapping the previous (overlap GPU with host publish).
    pub(crate) sparse_staging: [wgpu::Buffer; 2],
    /// Next staging slot to write (0|1).
    pub(crate) sparse_write: std::cell::Cell<u8>,
    /// WIP count recorded per staging slot at submit time.
    pub(crate) sparse_wip: [std::cell::Cell<u32>; 2],
    pub(crate) header_staging: wgpu::Buffer,
    pub(crate) seat_stride: u64,
    pub(crate) finish_stride: u64,
    /// Unfinished seat indices carried across workshifts (on-device resume).
    pub(crate) carry_indices: std::cell::RefCell<Vec<usize>>,
    pub(crate) carry_n: std::cell::Cell<u32>,
    /// Prior shift hit Stec BufferFull after apply_finish — republish orphans first.
    pub(crate) orphan_publish: std::cell::Cell<bool>,
}

impl NaiveGpuContext {
    pub fn try_new() -> Option<Self> {
        if std::env::var("CZ_FORCE_CPU_NAIVE").ok().as_deref() == Some("1") {
            let _ = std::fs::write(
                "/tmp/cz_naive_gpu_status.txt",
                "forced_off CZ_FORCE_CPU_NAIVE=1\n",
            );
            return None;
        }
        // try_new_async writes detailed status; only fill a fallback if still missing.
        let result = pollster::block_on(Self::try_new_async());
        if result.is_none() {
            let existing = std::fs::read_to_string("/tmp/cz_naive_gpu_status.txt").unwrap_or_default();
            if existing.trim().is_empty() || existing.starts_with("forced_off") {
                let _ = std::fs::write(
                    "/tmp/cz_naive_gpu_status.txt",
                    "failed try_new_async returned None (no prior status)\n",
                );
            }
        }
        result
    }

    async fn try_new_async() -> Option<Self> {
        // Prefer Vulkan, then fall back to GL (common under Xvfb/Mesa).
        let backend_attempts = [
            wgpu::Backends::VULKAN,
            wgpu::Backends::GL,
            wgpu::Backends::PRIMARY,
        ];
        let mut adapter = None;
        let mut last_err = String::from("no backends tried");
        for backends in backend_attempts {
            let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
                backends,
                ..Default::default()
            });
            match instance
                .request_adapter(&wgpu::RequestAdapterOptions {
                    power_preference: wgpu::PowerPreference::HighPerformance,
                    compatible_surface: None,
                    force_fallback_adapter: false,
                })
                .await
            {
                Ok(a) => {
                    adapter = Some(a);
                    break;
                }
                Err(e) => {
                    last_err = format!("backends={backends:?}: {e}");
                }
            }
        }
        let adapter = match adapter {
            Some(a) => a,
            None => {
                let _ = std::fs::write(
                    "/tmp/cz_naive_gpu_status.txt",
                    format!("no_adapter: {last_err}\n"),
                );
                return None;
            }
        };
        let info = adapter.get_info();
        // F32 is the IPS baseline; keep F64 pipeline ready when the adapter allows
        // so we can escalate past the F32 precision wall without restarting.
        let prefer_f64 = std::env::var("CZ_NAIVE_GPU_F64").ok().as_deref() == Some("1");
        let adapter_has_f64 = adapter.features().contains(wgpu::Features::SHADER_F64);
        let mut required = wgpu::Features::empty();
        if adapter_has_f64 {
            required |= wgpu::Features::SHADER_F64;
        }

        let (device, queue) = match adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("cz_naive_gpu"),
                required_features: required,
                required_limits: wgpu::Limits::default(),
                memory_hints: Default::default(),
                trace: Default::default(),
            })
            .await
        {
            Ok(dq) => dq,
            Err(e) => {
                let _ = std::fs::write(
                    "/tmp/cz_naive_gpu_status.txt",
                    format!(
                        "request_device failed name={:?} backend={:?}: {e}\n",
                        info.name, info.backend
                    ),
                );
                return None;
            }
        };

        let (bgl_f32, pipe_f32) =
            create_pipeline(&device, include_str!("bout_f32.wgsl")).await?;
        let f64_gear = if device.features().contains(wgpu::Features::SHADER_F64) {
            create_pipeline(&device, include_str!("bout_f64.wgsl")).await
        } else {
            None
        };

        const SEAT_F32: u64 = 72;
        const FIN_F32: u64 = 64;
        const SEAT_F64: u64 = 120;
        const FIN_F64: u64 = 96;
        // Buffers sized for the wider gear so F32↔F64 switches do not realloc.
        let seat_stride_max = if f64_gear.is_some() { SEAT_F64 } else { SEAT_F32 };
        let finish_stride_max = if f64_gear.is_some() { FIN_F64 } else { FIN_F32 };

        let seats_buf = make_buf(
            &device,
            "seats",
            seat_stride_max * MAX_WAVE as u64,
            wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::COPY_SRC,
        );
        let finishes_buf = make_buf(
            &device,
            "finishes",
            finish_stride_max * MAX_WAVE as u64,
            wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC | wgpu::BufferUsages::COPY_DST,
        );
        let finish_count_buf = make_buf(
            &device,
            "finish_count",
            4,
            wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC | wgpu::BufferUsages::COPY_DST,
        );
        let iter_total_buf = make_buf(
            &device,
            "iter_total",
            4,
            wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC | wgpu::BufferUsages::COPY_DST,
        );
        let params_buf = make_buf(
            &device,
            "params",
            32,
            wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        );
        let finish_staging = make_buf(
            &device,
            "finish_staging",
            finish_stride_max * MAX_WAVE as u64,
            wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        );
        let seat_staging = make_buf(
            &device,
            "seat_staging",
            seat_stride_max * MAX_WAVE as u64,
            wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        );
        let sparse_staging_0 = make_buf(
            &device,
            "sparse_staging_0",
            16 + finish_stride_max * MAX_WAVE as u64,
            wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        );
        let sparse_staging_1 = make_buf(
            &device,
            "sparse_staging_1",
            16 + finish_stride_max * MAX_WAVE as u64,
            wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        );
        let header_staging = make_buf(
            &device,
            "header_staging",
            8,
            wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        );

        let bg_f32 = make_bind_group(
            &device,
            &bgl_f32,
            &seats_buf,
            &finishes_buf,
            &finish_count_buf,
            &params_buf,
            &iter_total_buf,
        );
        let (f64_pipeline, f64_bgl, f64_bg) = match f64_gear {
            Some((bgl, pipe)) => {
                let bg = make_bind_group(
                    &device,
                    &bgl,
                    &seats_buf,
                    &finishes_buf,
                    &finish_count_buf,
                    &params_buf,
                    &iter_total_buf,
                );
                (Some(pipe), Some(bgl), Some(bg))
            }
            None => (None, None, None),
        };

        let use_f64 = prefer_f64 && f64_pipeline.is_some();
        let (precision, seat_stride, finish_stride, bind_group_layout, pipeline, bind_group) =
            if use_f64 {
                (
                    GpuPrecision::F64,
                    SEAT_F64,
                    FIN_F64,
                    f64_bgl.clone().unwrap(),
                    f64_pipeline.clone().unwrap(),
                    f64_bg.clone().unwrap(),
                )
            } else {
                (
                    GpuPrecision::F32,
                    SEAT_F32,
                    FIN_F32,
                    bgl_f32.clone(),
                    pipe_f32.clone(),
                    bg_f32.clone(),
                )
            };

        let _ = std::fs::write(
            "/tmp/cz_naive_gpu_status.txt",
            format!(
                "ok precision={precision:?} has_f64={} adapter={:?} backend={:?} wave_n={}\n",
                f64_pipeline.is_some(),
                info.name,
                info.backend,
                MAX_WAVE
            ),
        );

        Some(Self {
            device,
            queue,
            precision,
            generation: 0,
            wave_n: MAX_WAVE,
            last_wip_count: std::sync::atomic::AtomicU32::new(0),
            pipeline,
            bind_group_layout,
            bind_group,
            f64_pipeline,
            f64_bind_group_layout: f64_bgl,
            f64_bind_group: f64_bg,
            f32_pipeline: Some(pipe_f32),
            f32_bind_group_layout: Some(bgl_f32),
            f32_bind_group: Some(bg_f32),
            seats_buf,
            finishes_buf,
            finish_count_buf,
            iter_total_buf,
            params_buf,
            finish_staging,
            seat_staging,
            sparse_staging: [sparse_staging_0, sparse_staging_1],
            sparse_write: std::cell::Cell::new(0),
            sparse_wip: [std::cell::Cell::new(0), std::cell::Cell::new(0)],
            header_staging,
            seat_stride,
            finish_stride,
            carry_indices: std::cell::RefCell::new(Vec::new()),
            carry_n: std::cell::Cell::new(0),
            orphan_publish: std::cell::Cell::new(false),
        })
    }

    pub fn wave_n(&self) -> u32 {
        self.wave_n
    }

    pub fn set_wave_n(&mut self, n: u32) {
        self.wave_n = n.clamp(64, MAX_WAVE);
    }

    pub fn bump_generation(&mut self) {
        self.generation = self.generation.wrapping_add(1);
    }

    pub fn has_f64(&self) -> bool {
        self.f64_pipeline.is_some()
    }

    /// Switch active shader gear. Returns false if the requested gear is unavailable.
    /// Bumps generation so resident WIP is re-uploaded after a switch.
    pub fn ensure_precision(&mut self, want: GpuPrecision) -> bool {
        if self.precision == want {
            return true;
        }
        match want {
            GpuPrecision::F32 => {
                let (Some(pipe), Some(bgl), Some(bg)) = (
                    self.f32_pipeline.clone(),
                    self.f32_bind_group_layout.clone(),
                    self.f32_bind_group.clone(),
                ) else {
                    return false;
                };
                self.pipeline = pipe;
                self.bind_group_layout = bgl;
                self.bind_group = bg;
                self.seat_stride = 72;
                self.finish_stride = 64;
                self.precision = GpuPrecision::F32;
                self.bump_generation();
                self.carry_indices.borrow_mut().clear();
                self.carry_n.set(0);
                true
            }
            GpuPrecision::F64 => {
                let (Some(pipe), Some(bgl), Some(bg)) = (
                    self.f64_pipeline.clone(),
                    self.f64_bind_group_layout.clone(),
                    self.f64_bind_group.clone(),
                ) else {
                    return false;
                };
                self.pipeline = pipe;
                self.bind_group_layout = bgl;
                self.bind_group = bg;
                self.seat_stride = 120;
                self.finish_stride = 96;
                self.precision = GpuPrecision::F64;
                self.bump_generation();
                self.carry_indices.borrow_mut().clear();
                self.carry_n.set(0);
                true
            }
        }
    }

    /// Clear finish/iter atomics after a harvest so the next resident continue
    /// does not re-report the same finals (was re-applying every wave → CPU-like).
    pub fn clear_finish_accumulators(&self) {
        self.queue
            .write_buffer(&self.finish_count_buf, 0, &0u32.to_ne_bytes());
        self.queue
            .write_buffer(&self.iter_total_buf, 0, &0u32.to_ne_bytes());
    }

    pub fn end_shift_keep_generation(&mut self) {}
}

fn make_bind_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    seats: &wgpu::Buffer,
    finishes: &wgpu::Buffer,
    finish_count: &wgpu::Buffer,
    params: &wgpu::Buffer,
    iter_total: &wgpu::Buffer,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("naive_bg"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: seats.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: finishes.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: finish_count.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: params.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 4,
                resource: iter_total.as_entire_binding(),
            },
        ],
    })
}

fn make_buf(device: &wgpu::Device, label: &str, size: u64, usage: wgpu::BufferUsages) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        size,
        usage,
        mapped_at_creation: false,
    })
}

fn bgl_storage(binding: u32, read_write: bool) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Storage { read_only: !read_write },
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}

fn bgl_uniform(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}

async fn create_pipeline(
    device: &wgpu::Device,
    src: &str,
) -> Option<(wgpu::BindGroupLayout, wgpu::ComputePipeline)> {
    device.push_error_scope(wgpu::ErrorFilter::Validation);
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("naive_bout"),
        source: wgpu::ShaderSource::Wgsl(src.into()),
    });
    let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("naive_bgl"),
        entries: &[
            bgl_storage(0, true),
            bgl_storage(1, true),
            bgl_storage(2, true),
            bgl_uniform(3),
            bgl_storage(4, true),
        ],
    });
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("naive_pl"),
        bind_group_layouts: &[&bind_group_layout],
        push_constant_ranges: &[],
    });
    let _pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("naive_pipe"),
        layout: Some(&pipeline_layout),
        module: &shader,
        entry_point: Some("main"),
        compilation_options: Default::default(),
        cache: None,
    });
    if let Some(err) = device.pop_error_scope().await {
        let _ = std::fs::write(
            "/tmp/cz_naive_gpu_status.txt",
            format!("shader/pipeline validation failed: {err}\n"),
        );
        return None;
    }
    // Recreate pipeline after successful validation (previous may be invalid if errored mid-way).
    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("naive_pipe"),
        layout: Some(&pipeline_layout),
        module: &shader,
        entry_point: Some("main"),
        compilation_options: Default::default(),
        cache: None,
    });
    Some((bind_group_layout, pipeline))
}
