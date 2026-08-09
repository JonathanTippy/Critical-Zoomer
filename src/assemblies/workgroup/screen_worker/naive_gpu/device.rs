pub const MAX_WAVE: u32 = 4096;

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
    pub(crate) pipeline: wgpu::ComputePipeline,
    pub(crate) bind_group_layout: wgpu::BindGroupLayout,
    pub(crate) bind_group: wgpu::BindGroup,
    pub(crate) seats_buf: wgpu::Buffer,
    pub(crate) finishes_buf: wgpu::Buffer,
    pub(crate) finish_count_buf: wgpu::Buffer,
    pub(crate) iter_total_buf: wgpu::Buffer,
    pub(crate) params_buf: wgpu::Buffer,
    pub(crate) finish_staging: wgpu::Buffer,
    pub(crate) header_staging: wgpu::Buffer,
    pub(crate) seat_stride: u64,
    pub(crate) finish_stride: u64,
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
        // F32 is the IPS baseline (consumer FP64 is often ~1/32). Opt into F64 with CZ_NAIVE_GPU_F64=1.
        let prefer_f64 = std::env::var("CZ_NAIVE_GPU_F64").ok().as_deref() == Some("1");
        let want_f64 = prefer_f64 && adapter.features().contains(wgpu::Features::SHADER_F64);
        let mut required = wgpu::Features::empty();
        if want_f64 {
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

        let (precision, seat_stride, finish_stride, bind_group_layout, pipeline) =
            if want_f64 && device.features().contains(wgpu::Features::SHADER_F64) {
                match create_pipeline(&device, include_str!("bout_f64.wgsl")).await {
                    Some((bgl, pipe)) => (GpuPrecision::F64, 120u64, 96u64, bgl, pipe),
                    None => {
                        let (bgl, pipe) =
                            create_pipeline(&device, include_str!("bout_f32.wgsl")).await?;
                        (GpuPrecision::F32, 72, 64, bgl, pipe)
                    }
                }
            } else {
                let (bgl, pipe) = create_pipeline(&device, include_str!("bout_f32.wgsl")).await?;
                (GpuPrecision::F32, 72, 64, bgl, pipe)
            };

        let seats_buf = make_buf(
            &device,
            "seats",
            seat_stride * MAX_WAVE as u64,
            wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::COPY_SRC,
        );
        let finishes_buf = make_buf(
            &device,
            "finishes",
            finish_stride * MAX_WAVE as u64,
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
            finish_stride * MAX_WAVE as u64,
            wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        );
        // [finish_count:u32][iter_total:u32] — one map instead of two.
        let header_staging = make_buf(
            &device,
            "header_staging",
            8,
            wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        );

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("naive_bg"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: seats_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: finishes_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: finish_count_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: params_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: iter_total_buf.as_entire_binding(),
                },
            ],
        });

        let _ = std::fs::write(
            "/tmp/cz_naive_gpu_status.txt",
            format!(
                "ok precision={precision:?} adapter={:?} backend={:?} wave_n=2048\n",
                info.name, info.backend
            ),
        );

        Some(Self {
            device,
            queue,
            precision,
            generation: 0,
            wave_n: 2048.min(MAX_WAVE),
            pipeline,
            bind_group_layout,
            bind_group,
            seats_buf,
            finishes_buf,
            finish_count_buf,
            iter_total_buf,
            params_buf,
            finish_staging,
            header_staging,
            seat_stride,
            finish_stride,
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

    pub fn end_shift_keep_generation(&mut self) {}
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
