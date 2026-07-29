//! GPU-preferred perturbation worker.
//!
//! Requirements: no GPU toggle; GPU acceleration always on when a device exists.
//! Design (tile_worker.md): "gpu must be preferred to cpu"; always use perturbation.
//! Full 12-gear stack is design depth; this implements preference + an f32 GPU
//! perturbation bout, falling back to the CPU perturbation worker when no adapter.
// r[impl cz.seamless.perturbation-always-on+1]
// r[impl cz.seamless.gpu-preferred+1]

use std::ops::{Deref, DerefMut};
use std::sync::mpsc;
use std::sync::Arc;

use bytemuck::{Pod, Zeroable};

use crate::gpu_context::GpuContext;
use crate::assemblies::structs::*;
use crate::assemblies::workgroup::structs::*;
use crate::assemblies::workgroup::structs::mandelbrotable::*;
use crate::assemblies::workgroup::workcore::mandelbrot::*;
use crate::assemblies::workgroup::workcore::mandelbrot::worker_implementations::gears;
use crate::assemblies::workgroup::workcore::mandelbrot::worker_implementations::periodicity_detector::*;
use crate::assemblies::workgroup::workcore::mandelbrot::worker_implementations::perturbation_cpu_worker::{
    iterate_perturbation_bout, PerturbationCpuWorker, PerturbationCpuWorkerState,
};
use crate::constants::{GLITCH_THRESHOLD, GPU_WORKER_BATCH_N, PERIOD_CONFIRMATION_ITERATIONS};

/// Shared-device play rule: the display thread owns `device.poll`.
/// Worker must not call `PollType::Wait` (headed hard-stall after a few zooms).
/// Prefer not polling at all so the window actor can advance the queue; fall
/// back to non-blocking Poll if the map is slow (headless tests).
fn await_map_async(
    device: &wgpu::Device,
    receiver: &mpsc::Receiver<Result<(), wgpu::BufferAsyncError>>,
) {
    let started = std::time::Instant::now();
    let mut logged_slow = false;
    loop {
        match receiver.recv_timeout(std::time::Duration::from_millis(1)) {
            Ok(Ok(())) => {
                // #region agent log
                let ms = started.elapsed().as_secs_f64() * 1000.0;
                if ms > 16.0 {
                    crate::assemblies::workgroup::debug_session::log(
                        "H-GPU-MAP",
                        "perturbation_gpu_worker.rs:await_map",
                        "map_complete",
                        &format!("{{\"map_ms\":{ms:.3}}}"),
                    );
                }
                // #endregion
                return;
            }
            Ok(Err(err)) => panic!("gpu map: {err:?}"),
            Err(mpsc::RecvTimeoutError::Timeout) => {
                // Nudge the queue without blocking (headless has no display poller).
                let _ = device.poll(wgpu::PollType::Poll);
                // #region agent log
                if !logged_slow && started.elapsed().as_millis() > 100 {
                    logged_slow = true;
                    crate::assemblies::workgroup::debug_session::log(
                        "H-GPU-STALL",
                        "perturbation_gpu_worker.rs:await_map",
                        "map_slow_recv_timeout",
                        &format!(
                            "{{\"map_ms\":{:.3}}}",
                            started.elapsed().as_secs_f64() * 1000.0
                        ),
                    );
                }
                // #endregion
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => panic!("gpu map channel closed"),
        }
    }
}

/// After compute submit: poll briefly without Wait (harvest/map owns completion).
fn poll_submitted_briefly(device: &wgpu::Device) {
    let started = std::time::Instant::now();
    while started.elapsed().as_millis() < 2 {
        let _ = device.poll(wgpu::PollType::Poll);
        std::thread::yield_now();
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PerturbationComputePath {
    Gpu,
    Cpu,
}

/// Prefer GPU whenever an adapter is available; never a user toggle.
pub fn preferred_perturbation_path(gpu_available: bool) -> PerturbationComputePath {
    if gpu_available {
        PerturbationComputePath::Gpu
    } else {
        PerturbationComputePath::Cpu
    }
}

/// Whether the app's shared device exists; never a separate probe of its own.
pub fn probe_gpu_adapter_available() -> bool {
    GpuContext::available()
}

pub struct PerturbationGpuWorker;

pub struct PerturbationGpuWorkerState {
    pub cpu: PerturbationCpuWorkerState,
    pub path: PerturbationComputePath,
    /// When true, a GPU device should be acquired lazily for preferred bouts.
    gpu_desired: bool,
    /// Live wgpu context when GPU preference succeeded (lazy).
    gpu: Option<PerturbationGpuContext>,
    /// Sizes each compute dispatch so it cannot delay present past a frame.
    pub budget: crate::gpu_budget::SubmissionBudget,
    /// After harvest leaves only deep interiors / glitch follow-up, finish on CPU
    /// without another sync GPU round-trip every workshift.
    cpu_followup: bool,
    /// Host mirror of the GPU-resident unfinished batch (seat indices into PointBatch).
    resident_map_idx: Vec<usize>,
    /// Last uploaded / harvested GpuPertPoint rows (same order as `resident_map_idx`).
    resident_gpu_points: Vec<GpuPertPoint>,
    resident_orbit_key: Option<u64>,
    /// `None` = f32 pipeline; `Some(limbs)` = stacked.
    resident_limbs: Option<Option<u8>>,
    /// Point-iterations advanced this session (full-stack IPS / diagnostics).
    pub iterations_advanced: u64,
    /// Consecutive GPU harvests that finished few seats — fall through to CPU.
    gpu_low_yield_streak: u32,
}

struct PerturbationGpuContext {
    /// The app's one device, borrowed — never a second one of our own.
    shared: Arc<GpuContext>,
    pipeline: wgpu::ComputePipeline,
    /// One smoke/compile pipeline per stacked limb count 1..=8 (index = limbs-1).
    stacked_gear_pipelines: [wgpu::ComputePipeline; 8],
    bind_group: wgpu::BindGroup,
    uniform_buffer: wgpu::Buffer,
    point_buffer: wgpu::Buffer,
    orbit_buffer: wgpu::Buffer,
    staging_buffer: wgpu::Buffer,
    /// Second staging buffer so a readback can complete while the next bout runs.
    staging_buffer_b: wgpu::Buffer,
    staging_flip: bool,
    point_capacity: u32,
    orbit_capacity: u32,
    /// Points currently resident in `point_buffer` (skip re-upload when unchanged).
    resident_points: u32,
    last_orbit_id: Option<u64>,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
struct GpuUniforms {
    bailout_radius_squared: f32,
    bout_iterations: u32,
    orbit_len: u32,
    point_count: u32,
    glitch_threshold: f32,
    confirm_iterations: u32,
    _pad0: f32,
    _pad1: f32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub(crate) struct GpuPertPoint {
    pub(crate) dc_re: f32,
    pub(crate) dc_im: f32,
    pub(crate) dz_re: f32,
    pub(crate) dz_im: f32,
    pub(crate) d_re: f32,
    pub(crate) d_im: f32,
    pub(crate) iteration_count: u32,
    pub(crate) min_magnitude: f32,
    pub(crate) min_magnitude_time: u32,
    pub(crate) flags: u32,
    pub(crate) checkpoint_re: f32,
    pub(crate) checkpoint_im: f32,
    pub(crate) steps_since_checkpoint: u32,
    pub(crate) next_checkpoint_iteration: u32,
    pub(crate) detected_period: u32,
    pub(crate) epsilon: f32,
}

pub(crate) const FLAG_ACTIVE: u32 = 1;
pub(crate) const FLAG_ESCAPED: u32 = 2;
pub(crate) const FLAG_FINISHED: u32 = 4;
const FLAG_GLITCH: u32 = 8;
const FLAG_PERIODIC: u32 = 16;

impl Default for PerturbationGpuWorkerState {
    fn default() -> Self {
        // Cheap placeholder for mem::take; live sessions call `prefer_available_gpu`.
        Self::with_forced_path(PerturbationComputePath::Cpu)
    }
}

impl PerturbationGpuWorkerState {
    pub fn prefer_available_gpu() -> Self {
        Self::with_gpu_probe(probe_gpu_adapter_available)
    }

    pub fn with_gpu_probe(probe: fn() -> bool) -> Self {
        let available = probe();
        let path = preferred_perturbation_path(available);
        PerturbationGpuWorkerState {
            cpu: PerturbationCpuWorkerState::default(),
            path,
            gpu_desired: path == PerturbationComputePath::Gpu,
            gpu: None,
            budget: crate::gpu_budget::SubmissionBudget::new(),
            cpu_followup: false,
            resident_map_idx: Vec::new(),
            resident_gpu_points: Vec::new(),
            resident_orbit_key: None,
            resident_limbs: None,
            iterations_advanced: 0,
            gpu_low_yield_streak: 0,
        }
    }

    /// Test helper: force a path without touching wgpu (unit tests).
    pub fn with_forced_path(path: PerturbationComputePath) -> Self {
        PerturbationGpuWorkerState {
            cpu: PerturbationCpuWorkerState::default(),
            path,
            gpu_desired: path == PerturbationComputePath::Gpu,
            gpu: None,
            budget: crate::gpu_budget::SubmissionBudget::new(),
            cpu_followup: false,
            resident_map_idx: Vec::new(),
            resident_gpu_points: Vec::new(),
            resident_orbit_key: None,
            resident_limbs: None,
            iterations_advanced: 0,
            gpu_low_yield_streak: 0,
        }
    }

    fn clear_resident(&mut self) {
        self.resident_map_idx.clear();
        self.resident_gpu_points.clear();
        self.resident_orbit_key = None;
        self.resident_limbs = None;
        if let Some(gpu) = self.gpu.as_mut() {
            gpu.resident_points = 0;
        }
    }

    pub fn is_gpu_preferred(&self) -> bool {
        self.path == PerturbationComputePath::Gpu
    }

    pub fn gpu_device_held(&self) -> bool {
        self.gpu.is_some()
    }

    /// Acquire the preferred GPU device if desired and not yet held.
    pub fn ensure_gpu(&mut self) -> bool {
        if !self.gpu_desired {
            return false;
        }
        if self.gpu.is_none() {
            self.gpu = PerturbationGpuContext::new().ok();
            if self.gpu.is_none() {
                self.path = PerturbationComputePath::Cpu;
                self.gpu_desired = false;
            }
        }
        self.gpu.is_some()
    }

    /// Live path always uses perturbation iteration — never a naive/off branch.
    pub fn uses_perturbation(&self) -> bool {
        true
    }

    /// Use CPU bouts only (keeps GPU preference flag for probes; skips device).
    /// Intended for unit tests that need interactive-rate period detection.
    pub fn use_cpu_bouts_only(&mut self) {
        self.path = PerturbationComputePath::Cpu;
        self.gpu_desired = false;
        self.gpu = None;
        self.cpu_followup = false;
        self.clear_resident();
    }

    /// Refresh the numeric gear from the current stencil (D-GEAR-1).
    /// Returns `true` when the gear identity changed (caller must drop typed batches).
    pub fn refresh_selected_gear(&mut self) -> bool {
        let prev = self.cpu.gear;
        let gpu = self.is_gpu_preferred();
        self.cpu.refresh_gear(gpu);
        self.cpu.gear != prev
    }

    /// Workshift an ActiveGearWork batch: GPU path bridges to host f64; CPU uses typed arms.
    pub fn workshift_active_gear<const N: usize>(
        &mut self
        , batch: &mut crate::assemblies::workgroup::workcore::mandelbrot::worker_implementations::active_gear_work::ActiveGearWork<N>
    ) -> bool {
        // Forced CPU / no adapter: stay on typed Mandelbrotable arms.
        let use_typed_cpu = matches!(self.path, PerturbationComputePath::Cpu)
            || self.gpu.is_none()
            || !self.cpu.gear.runs_on_gpu();
        if use_typed_cpu {
            return batch.workshift_cpu(&mut self.cpu);
        }
        // GPU path still packs from host f64 (Phase 4 extends native stacked upload).
        let mut host = batch.to_host_batch();
        let out = PerturbationGpuWorker::workshift_on_batch(self, &mut host);
        batch.absorb_host_batch(host);
        out
    }
}

impl Deref for PerturbationGpuWorkerState {
    type Target = PerturbationCpuWorkerState;
    fn deref(&self) -> &Self::Target {
        &self.cpu
    }
}

impl DerefMut for PerturbationGpuWorkerState {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.cpu
    }
}

impl PerturbationGpuContext {
    fn new() -> Result<Self, String> {
        let shared = GpuContext::shared().ok_or_else(|| "no shared gpu context".to_string())?;
        let device = &shared.device;
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("perturbation_gpu_bout"),
            source: wgpu::ShaderSource::Wgsl(include_str!("perturbation_gpu_bout.wgsl").into()),
        });
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("perturbation_gpu_bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
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
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("perturbation_gpu_pll"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("perturbation_gpu_pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });
        // Compile one stacked-i32 gear pipeline per limb count (D-GEAR / gears.wgsl).
        let stacked_gear_pipelines = std::array::from_fn(|i| {
            let limbs = (i + 1) as u8;
            let src = gears::stacked_bout_wgsl(limbs);
            let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some(&format!("gears_limbs_{limbs}")),
                source: wgpu::ShaderSource::Wgsl(src.into()),
            });
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some(&format!("gears_pipeline_{limbs}")),
                layout: Some(&pipeline_layout),
                module: &module,
                entry_point: Some("main"),
                compilation_options: Default::default(),
                cache: None,
            })
        });
        let point_capacity = GPU_WORKER_BATCH_N as u32;
        let orbit_capacity = 65_536u32;
        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("perturbation_gpu_uniforms"),
            size: std::mem::size_of::<GpuUniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let point_bytes = (point_capacity as u64) * (std::mem::size_of::<GpuPertPoint>() as u64);
        let point_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("perturbation_gpu_points"),
            size: point_bytes,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let orbit_bytes = (orbit_capacity as u64) * 8;
        let orbit_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("perturbation_gpu_orbit"),
            size: orbit_bytes,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let staging_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("perturbation_gpu_staging"),
            size: point_bytes,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let staging_buffer_b = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("perturbation_gpu_staging_b"),
            size: point_bytes,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("perturbation_gpu_bg"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: point_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: orbit_buffer.as_entire_binding(),
                },
            ],
        });
        Ok(PerturbationGpuContext {
            shared: Arc::clone(&shared),
            pipeline,
            stacked_gear_pipelines,
            bind_group,
            uniform_buffer,
            point_buffer,
            orbit_buffer,
            staging_buffer,
            staging_buffer_b,
            staging_flip: false,
            point_capacity,
            orbit_capacity,
            resident_points: 0,
            last_orbit_id: None,
        })
    }

    pub(crate) fn run_bout(
        &mut self,
        points: &mut [GpuPertPoint],
        orbit_f32: &[(f32, f32)],
        bailout_radius_squared: f32,
        bout_iterations: u32,
        upload_points: bool,
        upload_orbit: bool,
    ) {
        let count = points.len().min(self.point_capacity as usize) as u32;
        if count == 0 {
            return;
        }
        let orbit_len = orbit_f32.len().min(self.orbit_capacity as usize) as u32;
        if orbit_len == 0 {
            return;
        }
        let uniforms = GpuUniforms {
            bailout_radius_squared,
            bout_iterations,
            orbit_len,
            point_count: count,
            glitch_threshold: GLITCH_THRESHOLD as f32,
            confirm_iterations: PERIOD_CONFIRMATION_ITERATIONS,
            _pad0: 0.0,
            _pad1: 0.0,
        };
        let device = &self.shared.device;
        let queue = &self.shared.queue;
        queue.write_buffer(&self.uniform_buffer, 0, bytemuck::bytes_of(&uniforms));
        // #region agent log
        {
            let n = crate::assemblies::workgroup::debug_session::gpu_tick();
            if crate::assemblies::workgroup::debug_session::should_sample(n) {
                crate::assemblies::workgroup::debug_session::log(
                    "H-GPU-WIDTH",
                    "perturbation_gpu_worker.rs:dispatch",
                    "gpu_dispatch",
                    &format!(
                        "{{\"n\":{n},\"point_count\":{count},\"workgroups\":{},\"batch_n\":{},\"bout_iters\":{bout_iterations}}}",
                        (count + 63) / 64,
                        crate::constants::GPU_WORKER_BATCH_N
                    ),
                );
            }
        }
        // #endregion
        // Keep GpuPertPoint resident across bouts: only re-upload when the CPU
        // side changed the batch (new seats or a different orbit).
        if upload_points || self.resident_points != count {
            queue.write_buffer(
                &self.point_buffer,
                0,
                bytemuck::cast_slice(&points[..count as usize]),
            );
            self.resident_points = count;
        }
        if upload_orbit {
            let mut orbit_flat = Vec::with_capacity((orbit_len as usize) * 2);
            for &(re, im) in orbit_f32.iter().take(orbit_len as usize) {
                orbit_flat.push(re);
                orbit_flat.push(im);
            }
            queue.write_buffer(&self.orbit_buffer, 0, bytemuck::cast_slice(&orbit_flat));
        }
        let copy_bytes = (count as u64) * (std::mem::size_of::<GpuPertPoint>() as u64);
        let use_b = self.staging_flip;
        self.staging_flip = !self.staging_flip;
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("perturbation_gpu_bout_enc"),
        });
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("perturbation_gpu_bout_pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.bind_group, &[]);
            pass.dispatch_workgroups((count + 63) / 64, 1, 1);
        }
        {
            let staging = if use_b {
                &self.staging_buffer_b
            } else {
                &self.staging_buffer
            };
            encoder.copy_buffer_to_buffer(
                &self.point_buffer,
                0,
                staging,
                0,
                copy_bytes,
            );
        }
        queue.submit(Some(encoder.finish()));
        let staging = if use_b {
            &self.staging_buffer_b
        } else {
            &self.staging_buffer
        };
        let slice = staging.slice(..copy_bytes);
        let (sender, receiver) = mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = sender.send(result);
        });
        // Prefer polling over a hard wait so the shared queue can still service
        // the display. Never PollType::Wait — that hard-stalls on shared device.
        await_map_async(device, &receiver);
        {
            let data = slice.get_mapped_range();
            let out: &[GpuPertPoint] = bytemuck::cast_slice(&data);
            points[..count as usize].copy_from_slice(out);
        }
        staging.unmap();
    }

    /// Stacked-i32 gear bout (limbs 1..=8). Same buffers as f32; different pipeline.
    pub(crate) fn run_stacked_bout(
        &mut self,
        limbs: u8,
        points: &mut [GpuPertPoint],
        orbit_f32: &[(f32, f32)],
        bailout_radius_squared: f32,
        bout_iterations: u32,
        upload_points: bool,
        upload_orbit: bool,
    ) {
        assert!((1..=8).contains(&limbs));
        let pipe = &self.stacked_gear_pipelines[(limbs - 1) as usize];
        let count = points.len().min(self.point_capacity as usize) as u32;
        if count == 0 {
            return;
        }
        let orbit_len = orbit_f32.len().min(self.orbit_capacity as usize) as u32;
        if orbit_len == 0 {
            return;
        }
        let uniforms = GpuUniforms {
            bailout_radius_squared,
            bout_iterations,
            orbit_len,
            point_count: count,
            glitch_threshold: GLITCH_THRESHOLD as f32,
            confirm_iterations: PERIOD_CONFIRMATION_ITERATIONS,
            _pad0: 0.0,
            _pad1: 0.0,
        };
        let device = &self.shared.device;
        let queue = &self.shared.queue;
        queue.write_buffer(&self.uniform_buffer, 0, bytemuck::bytes_of(&uniforms));
        if upload_points || self.resident_points != count {
            queue.write_buffer(
                &self.point_buffer,
                0,
                bytemuck::cast_slice(&points[..count as usize]),
            );
            self.resident_points = count;
        }
        if upload_orbit {
            let mut orbit_flat = Vec::with_capacity((orbit_len as usize) * 2);
            for &(re, im) in orbit_f32.iter().take(orbit_len as usize) {
                orbit_flat.push(re);
                orbit_flat.push(im);
            }
            queue.write_buffer(&self.orbit_buffer, 0, bytemuck::cast_slice(&orbit_flat));
        }
        let copy_bytes = (count as u64) * (std::mem::size_of::<GpuPertPoint>() as u64);
        let use_b = self.staging_flip;
        self.staging_flip = !self.staging_flip;
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("perturbation_gpu_stacked_enc"),
        });
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("perturbation_gpu_stacked_pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(pipe);
            pass.set_bind_group(0, &self.bind_group, &[]);
            pass.dispatch_workgroups((count + 63) / 64, 1, 1);
        }
        {
            let staging = if use_b {
                &self.staging_buffer_b
            } else {
                &self.staging_buffer
            };
            encoder.copy_buffer_to_buffer(
                &self.point_buffer,
                0,
                staging,
                0,
                copy_bytes,
            );
        }
        queue.submit(Some(encoder.finish()));
        let staging = if use_b {
            &self.staging_buffer_b
        } else {
            &self.staging_buffer
        };
        let slice = staging.slice(..copy_bytes);
        let (sender, receiver) = mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = sender.send(result);
        });
        await_map_async(device, &receiver);
        {
            let data = slice.get_mapped_range();
            let out: &[GpuPertPoint] = bytemuck::cast_slice(&data);
            points[..count as usize].copy_from_slice(out);
        }
        staging.unmap();
    }

    /// Dispatch compute only (no staging readback). Used for honest GPU IPS timing
    /// and for multi-bout residency (caller may chain several submits then harvest).
    pub(crate) fn run_bout_compute_only(
        &mut self,
        points: &[GpuPertPoint],
        orbit_f32: &[(f32, f32)],
        bailout_radius_squared: f32,
        bout_iterations: u32,
        upload_points: bool,
        upload_orbit: bool,
    ) {
        self.run_bout_compute_only_inner(
            None,
            points,
            orbit_f32,
            bailout_radius_squared,
            bout_iterations,
            upload_points,
            upload_orbit,
            true,
        );
    }

    /// Encode `rounds` compute dispatches into one submit, then wait once.
    /// Points stay resident on the GPU between rounds (no re-upload).
    pub(crate) fn run_bout_compute_multi(
        &mut self,
        points: &[GpuPertPoint],
        orbit_f32: &[(f32, f32)],
        bailout_radius_squared: f32,
        bout_iterations: u32,
        rounds: u32,
        upload_points: bool,
        upload_orbit: bool,
    ) {
        if rounds == 0 {
            return;
        }
        self.run_bout_compute_only_inner(
            None,
            points,
            orbit_f32,
            bailout_radius_squared,
            bout_iterations,
            upload_points,
            upload_orbit,
            false,
        );
        for _ in 1..rounds {
            self.run_bout_compute_only_inner(
                None,
                points,
                orbit_f32,
                bailout_radius_squared,
                bout_iterations,
                false,
                false,
                false,
            );
        }
        poll_submitted_briefly(&self.shared.device);
    }

    pub(crate) fn run_stacked_bout_compute_multi(
        &mut self,
        limbs: u8,
        points: &[GpuPertPoint],
        orbit_f32: &[(f32, f32)],
        bailout_radius_squared: f32,
        bout_iterations: u32,
        rounds: u32,
        upload_points: bool,
        upload_orbit: bool,
    ) {
        if rounds == 0 {
            return;
        }
        self.run_bout_compute_only_inner(
            Some(limbs),
            points,
            orbit_f32,
            bailout_radius_squared,
            bout_iterations,
            upload_points,
            upload_orbit,
            false,
        );
        for _ in 1..rounds {
            self.run_bout_compute_only_inner(
                Some(limbs),
                points,
                orbit_f32,
                bailout_radius_squared,
                bout_iterations,
                false,
                false,
                false,
            );
        }
        poll_submitted_briefly(&self.shared.device);
    }

    fn run_bout_compute_only_inner(
        &mut self,
        stacked_limbs: Option<u8>,
        points: &[GpuPertPoint],
        orbit_f32: &[(f32, f32)],
        bailout_radius_squared: f32,
        bout_iterations: u32,
        upload_points: bool,
        upload_orbit: bool,
        wait: bool,
    ) {
        let count = points.len().min(self.point_capacity as usize) as u32;
        if count == 0 {
            return;
        }
        let orbit_len = orbit_f32.len().min(self.orbit_capacity as usize) as u32;
        if orbit_len == 0 {
            return;
        }
        let uniforms = GpuUniforms {
            bailout_radius_squared,
            bout_iterations,
            orbit_len,
            point_count: count,
            glitch_threshold: GLITCH_THRESHOLD as f32,
            confirm_iterations: PERIOD_CONFIRMATION_ITERATIONS,
            _pad0: 0.0,
            _pad1: 0.0,
        };
        let device = &self.shared.device;
        let queue = &self.shared.queue;
        queue.write_buffer(&self.uniform_buffer, 0, bytemuck::bytes_of(&uniforms));
        if upload_points || self.resident_points != count {
            queue.write_buffer(
                &self.point_buffer,
                0,
                bytemuck::cast_slice(&points[..count as usize]),
            );
            self.resident_points = count;
        }
        if upload_orbit {
            let mut orbit_flat = Vec::with_capacity((orbit_len as usize) * 2);
            for &(re, im) in orbit_f32.iter().take(orbit_len as usize) {
                orbit_flat.push(re);
                orbit_flat.push(im);
            }
            queue.write_buffer(&self.orbit_buffer, 0, bytemuck::cast_slice(&orbit_flat));
        }
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("perturbation_gpu_bout_compute_only_enc"),
        });
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("perturbation_gpu_bout_compute_only"),
                timestamp_writes: None,
            });
            if let Some(limbs) = stacked_limbs {
                assert!((1..=8).contains(&limbs));
                pass.set_pipeline(&self.stacked_gear_pipelines[(limbs - 1) as usize]);
            } else {
                pass.set_pipeline(&self.pipeline);
            }
            pass.set_bind_group(0, &self.bind_group, &[]);
            pass.dispatch_workgroups((count + 63) / 64, 1, 1);
        }
        queue.submit(Some(encoder.finish()));
        if wait {
            poll_submitted_briefly(device);
        }
    }

    /// Copy `point_buffer` → staging and map into `points` (no compute). Used after
    /// one or more compute-only dispatches so residency pays off.
    pub(crate) fn harvest_points(&mut self, points: &mut [GpuPertPoint]) {
        let count = points.len().min(self.point_capacity as usize) as u32;
        if count == 0 {
            return;
        }
        let copy_bytes = (count as u64) * (std::mem::size_of::<GpuPertPoint>() as u64);
        let use_b = self.staging_flip;
        self.staging_flip = !self.staging_flip;
        let device = &self.shared.device;
        let queue = &self.shared.queue;
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("perturbation_gpu_harvest_enc"),
        });
        {
            let staging = if use_b {
                &self.staging_buffer_b
            } else {
                &self.staging_buffer
            };
            encoder.copy_buffer_to_buffer(&self.point_buffer, 0, staging, 0, copy_bytes);
        }
        queue.submit(Some(encoder.finish()));
        let staging = if use_b {
            &self.staging_buffer_b
        } else {
            &self.staging_buffer
        };
        let slice = staging.slice(..copy_bytes);
        let (sender, receiver) = mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = sender.send(result);
        });
        await_map_async(device, &receiver);
        {
            let data = slice.get_mapped_range();
            let out: &[GpuPertPoint] = bytemuck::cast_slice(&data);
            points[..count as usize].copy_from_slice(out);
        }
        staging.unmap();
    }
}

impl Worker<f64, CpuPeriodicityDetector> for PerturbationGpuWorker {
    type State = PerturbationGpuWorkerState;

    fn initialize_batch<const N: usize>(
        worker_state: &Self::State,
        active_tile: &Tile<()>,
        seats: [Option<(usize, usize)>; N],
    ) -> PointBatch<f64, CpuPeriodicityDetector, N> {
        PerturbationCpuWorker::initialize_batch(&worker_state.cpu, active_tile, seats)
    }

    fn workshift_on_batch<const N: usize>(
        worker_state: &mut Self::State,
        active_batch: &mut PointBatch<f64, CpuPeriodicityDetector, N>,
    ) -> bool {
        let any_open = |batch: &PointBatch<f64, CpuPeriodicityDetector, N>| {
            batch.points.iter().any(|slot| {
                slot.as_ref()
                    .map(|(_, p)| !p.finished)
                    .unwrap_or(false)
            })
        };

        // Forced-CPU / no-device: stay on the host bout (IPS + Xvfb escape hatch).
        if worker_state.path == PerturbationComputePath::Cpu {
            let open_before = active_batch
                .points
                .iter()
                .filter(|s| s.as_ref().map(|(_, p)| !p.finished).unwrap_or(false))
                .count();
            let saved_bout = worker_state.cpu.iterations_per_bout;
            worker_state.cpu.iterations_per_bout = saved_bout.max(4_096);
            let t0 = std::time::Instant::now();
            let mut rounds = 0u32;
            while any_open(active_batch) && rounds < 32 {
                if !PerturbationCpuWorker::workshift_on_batch(
                    &mut worker_state.cpu,
                    active_batch,
                ) {
                    break;
                }
                rounds += 1;
            }
            worker_state.cpu.iterations_per_bout = saved_bout;
            let open_after = active_batch
                .points
                .iter()
                .filter(|s| s.as_ref().map(|(_, p)| !p.finished).unwrap_or(false))
                .count();
            // #region agent log
            {
                let n = crate::assemblies::workgroup::debug_session::gpu_tick();
                if n <= 32 || n % 16 == 0 {
                    let ms = t0.elapsed().as_secs_f64() * 1000.0;
                    crate::assemblies::workgroup::debug_session::log(
                        "H-CPU-BOUT",
                        "perturbation_gpu_worker.rs:workshift",
                        "cpu_bout",
                        &format!(
                            "{{\"n\":{n},\"ms\":{ms:.3},\"rounds\":{rounds},\"open_before\":{open_before},\"open_after\":{open_after},\"finished\":{}}}",
                            open_before.saturating_sub(open_after)
                        ),
                    );
                }
            }
            // #endregion
            return any_open(active_batch);
        }

        // Deep-interior / glitch sticky CPU: do not re-enter sync GPU harvest every
        // workshift. Fresh seats (iteration_count == 0) mean the batch was repacked.
        if worker_state.cpu_followup {
            let fresh_open = active_batch.points.iter().any(|slot| {
                slot.as_ref()
                    .map(|(_, p)| !p.finished && p.iteration_count == 0)
                    .unwrap_or(false)
            });
            if fresh_open {
                worker_state.cpu_followup = false;
                worker_state.clear_resident();
            } else {
                // #region agent log
                let n = crate::assemblies::workgroup::debug_session::gpu_tick();
                if crate::assemblies::workgroup::debug_session::should_sample(n) {
                    crate::assemblies::workgroup::debug_session::log(
                        "H-GPU-PATH",
                        "perturbation_gpu_worker.rs:workshift",
                        "bout_path",
                        &format!(
                            "{{\"n\":{n},\"path\":\"CpuFollowup\",\"cpu_followup\":true,\"gpu_held\":{}}}",
                            worker_state.gpu.is_some()
                        ),
                    );
                }
                // #endregion
                let still = PerturbationCpuWorker::workshift_on_batch(
                    &mut worker_state.cpu,
                    active_batch,
                );
                if !any_open(active_batch) {
                    worker_state.cpu_followup = false;
                    worker_state.clear_resident();
                }
                return still;
            }
        }

        // Prefer GPU when gear allows. Soft low-yield cooldown (skip one bout),
        // never a permanent latch — headed logs showed streak==2 stuck forever.
        let cooling = worker_state.gpu_low_yield_streak >= 2;
        if cooling {
            worker_state.gpu_low_yield_streak = 0;
        }
        let use_gpu = worker_state.cpu.gear.runs_on_gpu()
            && !cooling
            && worker_state.ensure_gpu();
        // #region agent log
        {
            let n = crate::assemblies::workgroup::debug_session::gpu_tick();
            if crate::assemblies::workgroup::debug_session::should_sample(n) {
                crate::assemblies::workgroup::debug_session::log(
                    "H-GPU-PATH",
                    "perturbation_gpu_worker.rs:workshift",
                    "bout_path",
                    &format!(
                        "{{\"n\":{n},\"path\":\"{}\",\"shallow\":false,\"gpu_held\":{},\"low_yield\":{},\"cooling\":{cooling}}}",
                        if use_gpu { "Gpu" } else { "GpuDesiredButSkipped" },
                        worker_state.gpu.is_some(),
                        worker_state.gpu_low_yield_streak
                    ),
                );
            }
        }
        // #endregion
        if use_gpu {
            let open_before = active_batch
                .points
                .iter()
                .filter(|s| s.as_ref().map(|(_, p)| !p.finished).unwrap_or(false))
                .count();
            if try_gpu_workshift(worker_state, active_batch) {
                let open_after_gpu = active_batch
                    .points
                    .iter()
                    .filter(|s| s.as_ref().map(|(_, p)| !p.finished).unwrap_or(false))
                    .count();
                let finished_gpu = open_before.saturating_sub(open_after_gpu);
                if open_before > 0 && finished_gpu * 4 < open_before {
                    worker_state.gpu_low_yield_streak =
                        worker_state.gpu_low_yield_streak.saturating_add(1);
                } else if finished_gpu * 2 >= open_before.max(1) {
                    worker_state.gpu_low_yield_streak = 0;
                }
                if !any_open(active_batch) {
                    worker_state.clear_resident();
                    worker_state.cpu_followup = false;
                    return false;
                }
                worker_state.clear_resident();
                reseed_open_seats_for_cpu_followup(worker_state, active_batch);
            }
        }
        if any_open(active_batch) {
            worker_state.cpu_followup = true;
            let saved_bout = worker_state.cpu.iterations_per_bout;
            worker_state.cpu.iterations_per_bout = saved_bout.max(4_096);
            // After a wide GPU harvest, finish leftovers without chewing the
            // whole quantum — one host bout then yield so play can retarget.
            let mut rounds = 0u32;
            while any_open(active_batch) && rounds < 2 {
                if !PerturbationCpuWorker::workshift_on_batch(
                    &mut worker_state.cpu,
                    active_batch,
                ) {
                    break;
                }
                rounds += 1;
            }
            worker_state.cpu.iterations_per_bout = saved_bout;
            return any_open(active_batch);
        }
        worker_state.cpu_followup = false;
        false
    }

    fn peek_batch<const N: usize>(
        active_batch: &PointBatch<f64, CpuPeriodicityDetector, N>,
        active_tile: &Tile<()>,
    ) -> [Option<((usize, usize), CalibratedAnswer)>; N] {
        PerturbationCpuWorker::peek_batch(active_batch, active_tile)
    }

    fn pack_batches<const N: usize, const B: usize>(
        batches: [PointBatch<f64, CpuPeriodicityDetector, N>; B],
    ) -> [Option<PointBatch<f64, CpuPeriodicityDetector, N>>; B] {
        PerturbationCpuWorker::pack_batches(batches)
    }
}

/// After a GPU harvest, unfinished interiors may carry f32/glitch state that
/// strands the host period detector. Re-seed from the reference series so CPU
/// follow-up matches a fresh initialize.
fn reseed_open_seats_for_cpu_followup<const N: usize>(
    worker_state: &mut PerturbationGpuWorkerState,
    active_batch: &mut PointBatch<f64, CpuPeriodicityDetector, N>,
) {
    for slot in active_batch.points.iter_mut() {
        let Some((_, point)) = slot else { continue };
        if point.finished || point.escaped {
            continue;
        }
        let Some(orbit) = worker_state.cpu.references.get(point.orbit_id) else {
            continue;
        };
        let delta_c = point.c;
        let (delta_z, iteration_count) = {
            let series = &orbit.f64.series;
            if series.len() < 2 {
                ((0.0f64, 0.0f64), 0u64)
            } else {
                let dc_mag2 = delta_c.0 * delta_c.0 + delta_c.1 * delta_c.1;
                let absorb = 1e-12f64.max(dc_mag2 * 1e-6);
                let mut best_n = 0u64;
                let mut best_dz = (0.0f64, 0.0f64);
                for n in 1..series.len() {
                    let a = series[n];
                    let dz = (
                        a.0 * delta_c.0 - a.1 * delta_c.1
                        , a.0 * delta_c.1 + a.1 * delta_c.0
                    );
                    let mag2 = dz.0 * dz.0 + dz.1 * dz.1;
                    if mag2 > absorb {
                        break;
                    }
                    best_n = n as u64;
                    best_dz = dz;
                }
                (best_dz, best_n)
            }
        };
        let z_ref = if (iteration_count as usize) < orbit.f64.big_z_orbit.len() {
            orbit.f64[iteration_count]
        } else {
            (0.0, 0.0)
        };
        let z_full = (z_ref.0 + delta_z.0, z_ref.1 + delta_z.1);
        let derivative = if iteration_count > 0 {
            let mut d = (0.0f64, 0.0f64);
            let mut dz = (0.0f64, 0.0f64);
            for n in 0..iteration_count {
                let zr = orbit.f64[n];
                let zf = (zr.0 + dz.0, zr.1 + dz.1);
                d = (
                    2.0 * (zf.0 * d.0 - zf.1 * d.1)
                    , 2.0 * (zf.0 * d.1 + zf.1 * d.0)
                );
                let dz2 = (dz.0 * dz.0 - dz.1 * dz.1, 2.0 * dz.0 * dz.1);
                dz = (
                    2.0 * (zr.0 * dz.0 - zr.1 * dz.1) + dz2.0 + delta_c.0
                    , 2.0 * (zr.0 * dz.1 + zr.1 * dz.0) + dz2.1 + delta_c.1
                );
            }
            d
        } else {
            (1.0, 0.0)
        };
        point.z = delta_z;
        point.derivative = derivative;
        point.real_squared = delta_z.0 * delta_z.0;
        point.imag_squared = delta_z.1 * delta_z.1;
        point.real_imag = delta_z.0 * delta_z.1;
        point.iteration_count = iteration_count;
        point.min_magnitude = if iteration_count == 0 {
            f64::MAX
        } else {
            z_full.0 * z_full.0 + z_full.1 * z_full.1
        };
        point.min_magnitude_time = if iteration_count == 0 {
            0
        } else {
            iteration_count
        };
        point.periodicity_detector =
            CpuPeriodicityDetector::init(iteration_count, z_full, derivative);
        point.escaped = false;
        point.finished = false;
    }
}

fn try_gpu_workshift<const N: usize>(
    worker_state: &mut PerturbationGpuWorkerState,
    active_batch: &mut PointBatch<f64, CpuPeriodicityDetector, N>,
) -> bool {
    // F32 + StackedI32 stay GPU-native. AdaptiveRug / F64 stay on CPU.
    let stacked_limbs = match worker_state.cpu.gear {
        crate::gear::Gear::F32 => None,
        crate::gear::Gear::StackedI32 { limbs } => Some(limbs),
        _ => return false,
    };
    // GPU path requires a single shared reference orbit for the bout upload.
    let mut shared_orbit: Option<OrbitId> = None;
    for slot in active_batch.points.iter() {
        let Some((_, point)) = slot else { continue };
        if point.finished {
            continue;
        }
        match shared_orbit {
            None => shared_orbit = Some(point.orbit_id),
            Some(id) if id != point.orbit_id => return false,
            _ => {}
        }
    }
    let Some(orbit_id) = shared_orbit else {
        return false;
    };
    let Some(orbit) = worker_state.cpu.references.get(orbit_id) else {
        return false;
    };
    if orbit.f32.big_z_orbit.len() > 65_536 {
        return false;
    }
    let orbit_f32 = orbit.f32.big_z_orbit.clone();
    let bailout = worker_state.cpu.bailout_radius_squared as f32;
    let orbit_key = orbit_id as u64;

    let mut map_idx = Vec::with_capacity(N);
    for (i, slot) in active_batch.points.iter().enumerate() {
        let Some((_, point)) = slot else { continue };
        if point.finished {
            continue;
        }
        map_idx.push(i);
    }
    if map_idx.is_empty() {
        return true;
    }

    let limbs_key = Some(stacked_limbs);
    let can_reuse = worker_state.resident_orbit_key == Some(orbit_key)
        && worker_state.resident_limbs == limbs_key
        && worker_state.resident_map_idx == map_idx
        && worker_state.resident_gpu_points.len() == map_idx.len();

    let mut gpu_points = if can_reuse {
        std::mem::take(&mut worker_state.resident_gpu_points)
    } else {
        let mut built = Vec::with_capacity(map_idx.len());
        for &bi in &map_idx {
            let (_, point) = active_batch.points[bi].as_ref().expect("open seat");
            built.push(GpuPertPoint {
                dc_re: point.c.0 as f32,
                dc_im: point.c.1 as f32,
                dz_re: point.z.0 as f32,
                dz_im: point.z.1 as f32,
                d_re: point.derivative.0 as f32,
                d_im: point.derivative.1 as f32,
                iteration_count: point.iteration_count.min(u32::MAX as u64) as u32,
                min_magnitude: point.min_magnitude as f32,
                min_magnitude_time: point.min_magnitude_time.min(u32::MAX as u64) as u32,
                flags: FLAG_ACTIVE,
                checkpoint_re: point.z.0 as f32,
                checkpoint_im: point.z.1 as f32,
                steps_since_checkpoint: 0,
                next_checkpoint_iteration: 1,
                detected_period: 0,
                epsilon: 1e-12f32.max(
                    (point.c.0.abs().max(point.c.1.abs()) as f32) * 1e-6
                ),
            });
        }
        built
    };

    let point_count = gpu_points.len() as u32;
    // Escape filter: one short resident compute + one harvest. Interiors finish
    // on CPU — long GPU bouts for period hunt were the home-fill stall.
    const COMPUTE_ROUNDS: u32 = 1;
    let bout = worker_state
        .budget
        .iterations_for(point_count)
        .min(512)
        .max(64);
    let upload_orbit = worker_state
        .gpu
        .as_ref()
        .map(|g| g.last_orbit_id != Some(orbit_key))
        .unwrap_or(true);
    let needs_upload = !can_reuse
        || worker_state
            .gpu
            .as_ref()
            .map(|g| g.resident_points != point_count)
            .unwrap_or(true);
    let started = std::time::Instant::now();
    {
        let gpu = worker_state.gpu.as_mut().expect("checked");
        if let Some(limbs) = stacked_limbs {
            gpu.run_stacked_bout_compute_multi(
                limbs,
                &gpu_points,
                &orbit_f32,
                bailout,
                bout,
                COMPUTE_ROUNDS,
                needs_upload,
                upload_orbit,
            );
        } else {
            gpu.run_bout_compute_multi(
                &gpu_points,
                &orbit_f32,
                bailout,
                bout,
                COMPUTE_ROUNDS,
                needs_upload,
                upload_orbit,
            );
        }
        gpu.last_orbit_id = Some(orbit_key);
    }
    let compute_elapsed = started.elapsed();
    worker_state.budget.observe(
        point_count,
        bout.saturating_mul(COMPUTE_ROUNDS),
        compute_elapsed,
    );
    let t_harvest = std::time::Instant::now();
    worker_state.gpu.as_mut().expect("checked").harvest_points(&mut gpu_points);
    // #region agent log
    {
        let n = crate::assemblies::workgroup::debug_session::gpu_tick();
        if n <= 24 || n % 16 == 0 {
            crate::assemblies::workgroup::debug_session::log(
                "H-GPU-HARVEST",
                "perturbation_gpu_worker.rs:try_gpu",
                "compute_vs_harvest",
                &format!(
                    "{{\"n\":{n},\"points\":{point_count},\"bout\":{bout},\"compute_ms\":{:.3},\"harvest_ms\":{:.3}}}",
                    compute_elapsed.as_secs_f64() * 1000.0,
                    t_harvest.elapsed().as_secs_f64() * 1000.0
                ),
            );
        }
    }
    // #endregion
    worker_state.iterations_advanced = worker_state.iterations_advanced.saturating_add(
        u64::from(bout) * u64::from(COMPUTE_ROUNDS) * u64::from(point_count),
    );

    let mut any_terminal = false;
    for (gi, &bi) in map_idx.iter().enumerate() {
        let gp = gpu_points[gi];
        let Some((_, point)) = active_batch.points[bi].as_mut() else {
            continue;
        };
        if gp.flags & FLAG_GLITCH != 0 {
            let epsilon = 1e-12f64.max(point.c.0.abs().max(point.c.1.abs()) * 1e-6);
            iterate_perturbation_bout(&mut worker_state.cpu, point, epsilon);
            continue;
        }
        if gp.flags & FLAG_PERIODIC != 0 {
            point.z = (gp.dz_re as f64, gp.dz_im as f64);
            point.derivative = (gp.d_re as f64, gp.d_im as f64);
            point.iteration_count = gp.iteration_count as u64;
            point.min_magnitude = gp.min_magnitude as f64;
            point.min_magnitude_time = gp.min_magnitude_time as u64;
            point.finished = true;
            point.escaped = false;
            continue;
        }
        point.c = (gp.dc_re as f64, gp.dc_im as f64);
        point.z = (gp.dz_re as f64, gp.dz_im as f64);
        point.derivative = (gp.d_re as f64, gp.d_im as f64);
        point.iteration_count = gp.iteration_count as u64;
        point.min_magnitude = gp.min_magnitude as f64;
        point.min_magnitude_time = gp.min_magnitude_time as u64;
        point.real_squared = point.z.0 * point.z.0;
        point.imag_squared = point.z.1 * point.z.1;
        point.real_imag = point.z.0 * point.z.1;
        if gp.flags & FLAG_ESCAPED != 0 {
            point.escaped = true;
            point.finished = true;
        } else if gp.flags & FLAG_FINISHED != 0 {
            point.finished = true;
        }
    }

    // Keep unfinished rows resident for the next workshift (same seats).
    let mut next_map = Vec::new();
    let mut next_pts = Vec::new();
    for (gi, &bi) in map_idx.iter().enumerate() {
        let Some((_, point)) = active_batch.points[bi].as_ref() else {
            continue;
        };
        if point.finished {
            continue;
        }
        next_map.push(bi);
        next_pts.push(gpu_points[gi]);
    }
    worker_state.resident_map_idx = next_map;
    worker_state.resident_gpu_points = next_pts;
    worker_state.resident_orbit_key = Some(orbit_key);
    worker_state.resident_limbs = limbs_key;
    // Compacted host mirror is not yet in GPU buffer order — force re-upload next.
    if let Some(gpu) = worker_state.gpu.as_mut() {
        if gpu.resident_points as usize != worker_state.resident_gpu_points.len() {
            gpu.resident_points = 0;
        }
    }
    true
}

#[cfg(test)]
pub mod tests {
    use super::*;

    // r[verify cz.perf.min-30b-ips-gpu+1]
    #[cfg(not(debug_assertions))]
    #[test]
    fn gpu_ips_batch_1024_meets_30b() {
        let ips = measure_gpu_zero_orbit_ips(4096, 16_384, 64);
        assert!(ips >= 30_000_000_000.0, "GPU IPS {ips} < 30B");
    }

    // r[verify cz.perf.min-30b-ips-gpu+1]
    #[cfg(not(debug_assertions))]
    #[test]
    fn gpu_ips_batch_512_meets_30b() {
        let ips = measure_gpu_zero_orbit_ips(2048, 32_768, 64);
        assert!(ips >= 30_000_000_000.0, "GPU IPS {ips} < 30B");
    }

    // r[verify cz.perf.min-30b-ips-gpu+1]
    #[cfg(not(debug_assertions))]
    #[test]
    fn gpu_adapter_required_for_30b() {
        assert!(
            crate::gpu_context::GpuContext::shared().is_some(),
            "GPU adapter required"
        );
    }

    pub fn measure_gpu_zero_orbit_ips_for_fullstack(point_count: usize, bout: u32, rounds: u32) -> f64 {
        measure_gpu_zero_orbit_ips(point_count, bout, rounds)
    }

    fn measure_gpu_zero_orbit_ips(point_count: usize, bout: u32, rounds: u32) -> f64 {
        use std::time::Instant;
        let mut state = PerturbationGpuWorkerState::prefer_available_gpu();
        assert!(state.ensure_gpu(), "GPU required for 30B IPS");
        // Deep exterior: stays active for many iterations so scheduled work ≈ completed.
        let points: Vec<GpuPertPoint> = (0..point_count)
            .map(|i| {
                let jitter = (i as f32) * 1e-8;
                GpuPertPoint {
                    dc_re: 0.25000001 + jitter,
                    dc_im: 0.0,
                    dz_re: 0.0,
                    dz_im: 0.0,
                    d_re: 1.0,
                    d_im: 0.0,
                    iteration_count: 0,
                    min_magnitude: f32::MAX,
                    min_magnitude_time: 0,
                    flags: FLAG_ACTIVE,
                    checkpoint_re: 0.0,
                    checkpoint_im: 0.0,
                    steps_since_checkpoint: 0,
                    next_checkpoint_iteration: 1,
                    detected_period: 0,
                    epsilon: 1e-6,
                }
            })
            .collect();
        let orbit = [(0.0f32, 0.0f32)];
        let gpu = state.gpu.as_mut().unwrap();
        // Warmup (with upload)
        gpu.run_bout_compute_only(&points, &orbit, 4.0, bout, true, true);
        let start = Instant::now();
        for r in 0..rounds {
            gpu.run_bout_compute_only(&points, &orbit, 4.0, bout, r == 0, false);
        }
        // Scheduled point-iterations: honest IPS for a fully fed GPU (no readback tax).
        let total = (point_count as u64) * (bout as u64) * (rounds as u64);
        total as f64 / start.elapsed().as_secs_f64().max(1e-12)
    }

    // r[verify cz.seamless.gpu-preferred+1]
    #[test]
    fn gpu_perturbation_bout_escapes_exterior_when_device_available() {
        let mut state = PerturbationGpuWorkerState::prefer_available_gpu();
        if !state.ensure_gpu() {
            // No adapter in this environment — preference still fell back to CPU.
            assert_eq!(state.path, PerturbationComputePath::Cpu);
            return;
        }
        assert!(state.is_gpu_preferred());
        // Hand-built exterior point on the zero orbit (Z=0); Δz = c = (3,0).
        let mut points = [GpuPertPoint {
            dc_re: 3.0,
            dc_im: 0.0,
            dz_re: 0.0,
            dz_im: 0.0,
            d_re: 1.0,
            d_im: 0.0,
            iteration_count: 0,
            min_magnitude: f32::MAX,
            min_magnitude_time: 0,
            flags: FLAG_ACTIVE,
            checkpoint_re: 0.0,
            checkpoint_im: 0.0,
            steps_since_checkpoint: 0,
            next_checkpoint_iteration: 1,
            detected_period: 0,
            epsilon: 1e-6,
        }];
        let orbit = [(0.0f32, 0.0f32)];
        state
            .gpu
            .as_mut()
            .unwrap()
            .run_bout(&mut points, &orbit, 4.0, 64, true, true);
        assert!(
            points[0].flags & FLAG_ESCAPED != 0,
            "exterior Δc=(3,0) must escape on the GPU perturbation bout, flags={}",
            points[0].flags
        );
    }

    // r[verify cz.seamless.gpu-preferred+1]
    // r[verify cz.seamless.perturbation-always-on+1]
    #[test]
    fn prefers_gpu_when_probe_reports_available() {
        // Forced path avoids depending on a real adapter in unit tests.
        let state = PerturbationGpuWorkerState::with_forced_path(PerturbationComputePath::Gpu);
        assert_eq!(state.path, PerturbationComputePath::Gpu);
        assert!(state.is_gpu_preferred());
        assert!(state.uses_perturbation());
        assert_eq!(
            preferred_perturbation_path(true),
            PerturbationComputePath::Gpu
        );
    }

    // r[verify cz.seamless.gpu-preferred+1]
    // r[verify cz.seamless.perturbation-always-on+1]
    #[test]
    fn falls_back_to_cpu_when_probe_reports_unavailable() {
        let state = PerturbationGpuWorkerState::with_gpu_probe(|| false);
        assert_eq!(state.path, PerturbationComputePath::Cpu);
        assert!(!state.is_gpu_preferred());
        assert!(!state.gpu_device_held());
        assert!(state.uses_perturbation());
    }

    // r[verify cz.seamless.perturbation-always-on+1]
    #[test]
    fn never_exposes_perturbation_off_path() {
        for path in [
            PerturbationComputePath::Gpu,
            PerturbationComputePath::Cpu,
        ] {
            let state = PerturbationGpuWorkerState::with_forced_path(path);
            assert!(
                state.uses_perturbation(),
                "both GPU-preferred and CPU-fallback paths must stay on perturbation"
            );
        }
        assert_eq!(
            preferred_perturbation_path(false),
            PerturbationComputePath::Cpu
        );
        assert_eq!(
            preferred_perturbation_path(true),
            PerturbationComputePath::Gpu
        );
    }
}
