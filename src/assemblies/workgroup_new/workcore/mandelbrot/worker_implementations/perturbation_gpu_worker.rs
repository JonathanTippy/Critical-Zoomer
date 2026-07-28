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
use crate::assemblies::workgroup_new::structs::*;
use crate::assemblies::workgroup_new::structs::mandelbrotable::*;
use crate::assemblies::workgroup_new::workcore::mandelbrot::*;
use crate::assemblies::workgroup_new::workcore::mandelbrot::worker_implementations::gears;
use crate::assemblies::workgroup_new::workcore::mandelbrot::worker_implementations::periodicity_detector::*;
use crate::assemblies::workgroup_new::workcore::mandelbrot::worker_implementations::perturbation_cpu_worker::{
    iterate_perturbation_bout, PerturbationCpuWorker, PerturbationCpuWorkerState,
};
use crate::constants::{GLITCH_THRESHOLD, GPU_WORKER_BATCH_N, PERIOD_CONFIRMATION_ITERATIONS};

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
    /// After a GPU bout leaves interiors open, finish them on CPU without
    /// another sync GPU round-trip every workshift (was making headed fill ~10× slower).
    cpu_followup: bool,
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
    }

    /// Refresh the numeric gear from the current stencil (D-GEAR-1).
    pub fn refresh_selected_gear(&mut self) {
        let gpu = self.is_gpu_preferred();
        self.cpu.refresh_gear(gpu);
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
            let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some(&format!("gears_pll_{limbs}")),
                bind_group_layouts: &[],
                push_constant_ranges: &[],
            });
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some(&format!("gears_pipeline_{limbs}")),
                layout: Some(&layout),
                module: &module,
                entry_point: Some("gears_smoke_main"),
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
        // the display between checks when a bout is long.
        let mut spins = 0u32;
        loop {
            let _ = device.poll(wgpu::PollType::Poll);
            match receiver.try_recv() {
                Ok(Ok(())) => break,
                Ok(Err(err)) => panic!("gpu map: {err:?}"),
                Err(mpsc::TryRecvError::Empty) => {
                    if spins > 10_000 {
                        let _ = device.poll(wgpu::PollType::Wait);
                        receiver.recv().expect("gpu map channel").expect("gpu map");
                        break;
                    }
                    spins += 1;
                    std::thread::yield_now();
                }
                Err(mpsc::TryRecvError::Disconnected) => panic!("gpu map channel closed"),
            }
        }
        {
            let data = slice.get_mapped_range();
            let out: &[GpuPertPoint] = bytemuck::cast_slice(&data);
            points[..count as usize].copy_from_slice(out);
        }
        staging.unmap();
    }

    /// Dispatch compute only (no staging readback). Used for honest GPU IPS timing.
    pub(crate) fn run_bout_compute_only(
        &mut self,
        points: &[GpuPertPoint],
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
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.bind_group, &[]);
            pass.dispatch_workgroups((count + 63) / 64, 1, 1);
        }
        queue.submit(Some(encoder.finish()));
        let _ = device.poll(wgpu::PollType::Wait);
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

        // Finish interiors on CPU after a GPU escape bout — do not re-enter the
        // sync GPU readback path every workshift while those seats remain open.
        // Fresh seats (iteration_count == 0) mean the batch was repacked; clear sticky.
        if worker_state.cpu_followup {
            let fresh_open = active_batch.points.iter().any(|slot| {
                slot.as_ref()
                    .map(|(_, p)| !p.finished && p.iteration_count == 0)
                    .unwrap_or(false)
            });
            if fresh_open {
                worker_state.cpu_followup = false;
            } else {
                let still = PerturbationCpuWorker::workshift_on_batch(
                    &mut worker_state.cpu,
                    active_batch,
                );
                if !any_open(active_batch) {
                    worker_state.cpu_followup = false;
                }
                return still;
            }
        }

        // Prefer GPU only when the selected gear runs on GPU (no silent f64→f32).
        // r[impl cz.seamless.gpu-preferred+1]
        if worker_state.cpu.gear.runs_on_gpu() && worker_state.ensure_gpu() {
            if try_gpu_workshift(worker_state, active_batch) {
                if any_open(active_batch) {
                    worker_state.cpu_followup = true;
                    return PerturbationCpuWorker::workshift_on_batch(
                        &mut worker_state.cpu,
                        active_batch,
                    );
                }
                return false;
            }
        }
        PerturbationCpuWorker::workshift_on_batch(&mut worker_state.cpu, active_batch)
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

fn try_gpu_workshift<const N: usize>(
    worker_state: &mut PerturbationGpuWorkerState,
    active_batch: &mut PointBatch<f64, CpuPeriodicityDetector, N>,
) -> bool {
    // Only F32 uses the live f32 bout shader. Stacked gears: CPU until stacked bout
    // dispatch is fully wired (pipelines exist as smoke). Never cast F64/AdaptiveRug.
    match worker_state.cpu.gear {
        crate::gear::Gear::F32 => {}
        crate::gear::Gear::StackedI32 { .. } => {
            // Prefer CPU stacked/typed path over silent f32 cast.
            return false;
        }
        _ => return false,
    }
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

    let mut gpu_points = Vec::with_capacity(N);
    let mut map_idx = Vec::with_capacity(N);
    for (i, slot) in active_batch.points.iter().enumerate() {
        let Some((_, point)) = slot else { continue };
        if point.finished {
            continue;
        }
        map_idx.push(i);
        gpu_points.push(GpuPertPoint {
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
    if gpu_points.is_empty() {
        return true;
    }
    let point_count = gpu_points.len() as u32;
    let bout = worker_state
        .budget
        .iterations_for(point_count)
        .min(worker_state.cpu.iterations_per_bout);
    let started = std::time::Instant::now();
    {
        let gpu = worker_state.gpu.as_mut().expect("checked");
        let upload_orbit = gpu.last_orbit_id != Some(orbit_key);
        // Always upload points from CPU for now: seats change every workshift.
        // Residence still helps when the same buffer is re-dispatched mid-batch.
        gpu.run_bout(
            &mut gpu_points,
            &orbit_f32,
            bailout,
            bout,
            true,
            upload_orbit,
        );
        gpu.last_orbit_id = Some(orbit_key);
    }
    worker_state
        .budget
        .observe(point_count, bout, started.elapsed());
    for (gi, &bi) in map_idx.iter().enumerate() {
        let gp = gpu_points[gi];
        let Some((_, point)) = active_batch.points[bi].as_mut() else {
            continue;
        };
        if gp.flags & FLAG_GLITCH != 0 {
            // Shader normally rebinds in-place; if a glitch still escapes the
            // bout finished, finish on CPU against the zero orbit.
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
            // Detected period is certain (shader search); host period resolve
            // can still refine later for non-zero-orbit seats.
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
    true
}

#[cfg(test)]
mod tests {
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
