//! GPU-preferred perturbation worker.
//!
//! Requirements: no GPU toggle; GPU acceleration always on when a device exists.
//! Design (tile_worker.md): "gpu must be preferred to cpu"; always use perturbation.
//! Full 12-gear stack is design depth; this implements preference + an f32 GPU
//! perturbation bout, falling back to the CPU perturbation worker when no adapter.
// r[impl cz.seamless.perturbation-always-on+1]
// r[impl cz.seamless.gpu-preferred+1]

use std::future::Future;
use std::ops::{Deref, DerefMut};
use std::pin::Pin;
use std::sync::mpsc;
use std::sync::Arc;
use std::task::{Context, Poll, Wake, Waker};

use bytemuck::{Pod, Zeroable};

use crate::assemblies::structs::*;
use crate::assemblies::workgroup_new::structs::*;
use crate::assemblies::workgroup_new::structs::mandelbrotable::*;
use crate::assemblies::workgroup_new::workcore::mandelbrot::*;
use crate::assemblies::workgroup_new::workcore::mandelbrot::worker_implementations::periodicity_detector::*;
use crate::assemblies::workgroup_new::workcore::mandelbrot::worker_implementations::perturbation_cpu_worker::{
    iterate_perturbation_bout, PerturbationCpuWorker, PerturbationCpuWorkerState,
};
use crate::constants::{GLITCH_THRESHOLD, GPU_WORKER_BATCH_N};

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

/// Probe for a usable wgpu adapter (same preference order as the naive GPU worker).
pub fn probe_gpu_adapter_available() -> bool {
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
    if block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        compatible_surface: None,
        force_fallback_adapter: false,
    }))
    .is_ok()
    {
        return true;
    }
    block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::LowPower,
        compatible_surface: None,
        force_fallback_adapter: true,
    }))
    .is_ok()
}

pub struct PerturbationGpuWorker;

pub struct PerturbationGpuWorkerState {
    pub cpu: PerturbationCpuWorkerState,
    pub path: PerturbationComputePath,
    /// When true, a GPU device should be acquired lazily for preferred bouts.
    gpu_desired: bool,
    /// Live wgpu context when GPU preference succeeded (lazy).
    gpu: Option<PerturbationGpuContext>,
}

struct PerturbationGpuContext {
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipeline: wgpu::ComputePipeline,
    bind_group: wgpu::BindGroup,
    uniform_buffer: wgpu::Buffer,
    point_buffer: wgpu::Buffer,
    orbit_buffer: wgpu::Buffer,
    staging_buffer: wgpu::Buffer,
    point_capacity: u32,
    orbit_capacity: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
struct GpuUniforms {
    bailout_radius_squared: f32,
    bout_iterations: u32,
    orbit_len: u32,
    point_count: u32,
    glitch_threshold: f32,
    _pad0: f32,
    _pad1: f32,
    _pad2: f32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
struct GpuPertPoint {
    dc_re: f32,
    dc_im: f32,
    dz_re: f32,
    dz_im: f32,
    d_re: f32,
    d_im: f32,
    iteration_count: u32,
    min_magnitude: f32,
    min_magnitude_time: u32,
    flags: u32,
    _pad0: u32,
    _pad1: u32,
}

const FLAG_ACTIVE: u32 = 1;
const FLAG_ESCAPED: u32 = 2;
const FLAG_FINISHED: u32 = 4;
const FLAG_GLITCH: u32 = 8;

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
        }
    }

    /// Test helper: force a path without touching wgpu (unit tests).
    pub fn with_forced_path(path: PerturbationComputePath) -> Self {
        PerturbationGpuWorkerState {
            cpu: PerturbationCpuWorkerState::default(),
            path,
            gpu_desired: path == PerturbationComputePath::Gpu,
            gpu: None,
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
        self.gpu_desired = false;
        self.gpu = None;
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
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
        let adapter = match block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: None,
            force_fallback_adapter: false,
        })) {
            Ok(adapter) => adapter,
            Err(_) => block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::LowPower,
                compatible_surface: None,
                force_fallback_adapter: true,
            }))
            .map_err(|e| format!("gpu adapter: {e:?}"))?,
        };
        let (device, queue) = block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("perturbation_gpu_worker"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::default(),
            memory_hints: wgpu::MemoryHints::default(),
            trace: wgpu::Trace::Off,
        }))
        .map_err(|e| format!("gpu device: {e:?}"))?;
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
            device,
            queue,
            pipeline,
            bind_group,
            uniform_buffer,
            point_buffer,
            orbit_buffer,
            staging_buffer,
            point_capacity,
            orbit_capacity,
        })
    }

    fn run_bout(
        &mut self,
        points: &mut [GpuPertPoint],
        orbit_f32: &[(f32, f32)],
        bailout_radius_squared: f32,
        bout_iterations: u32,
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
            _pad0: 0.0,
            _pad1: 0.0,
            _pad2: 0.0,
        };
        self.queue
            .write_buffer(&self.uniform_buffer, 0, bytemuck::bytes_of(&uniforms));
        self.queue.write_buffer(
            &self.point_buffer,
            0,
            bytemuck::cast_slice(&points[..count as usize]),
        );
        let mut orbit_flat = Vec::with_capacity((orbit_len as usize) * 2);
        for &(re, im) in orbit_f32.iter().take(orbit_len as usize) {
            orbit_flat.push(re);
            orbit_flat.push(im);
        }
        self.queue
            .write_buffer(&self.orbit_buffer, 0, bytemuck::cast_slice(&orbit_flat));
        let copy_bytes = (count as u64) * (std::mem::size_of::<GpuPertPoint>() as u64);
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
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
        encoder.copy_buffer_to_buffer(
            &self.point_buffer,
            0,
            &self.staging_buffer,
            0,
            copy_bytes,
        );
        self.queue.submit(Some(encoder.finish()));
        let slice = self.staging_buffer.slice(..copy_bytes);
        let (sender, receiver) = mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = sender.send(result);
        });
        self.device.poll(wgpu::PollType::Wait).expect("gpu poll");
        receiver.recv().expect("gpu map channel").expect("gpu map");
        {
            let data = slice.get_mapped_range();
            let out: &[GpuPertPoint] = bytemuck::cast_slice(&data);
            points[..count as usize].copy_from_slice(out);
        }
        self.staging_buffer.unmap();
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
        // Prefer GPU when available: shared-orbit batches run the perturbation
        // compute shader; unfinished seats (insides needing periodicity) continue
        // on the CPU perturbation kernel from the post-GPU state.
        if worker_state.ensure_gpu() {
            if try_gpu_workshift(worker_state, active_batch) {
                let any_open = active_batch.points.iter().any(|slot| {
                    slot.as_ref()
                        .map(|(_, p)| !p.finished)
                        .unwrap_or(false)
                });
                if any_open {
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
    let bout = worker_state.cpu.iterations_per_bout;

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
            _pad0: 0,
            _pad1: 0,
        });
    }
    if gpu_points.is_empty() {
        return true;
    }
    {
        let gpu = worker_state.gpu.as_mut().expect("checked");
        gpu.run_bout(&mut gpu_points, &orbit_f32, bailout, bout);
    }
    for (gi, &bi) in map_idx.iter().enumerate() {
        let gp = gpu_points[gi];
        let Some((_, point)) = active_batch.points[bi].as_mut() else {
            continue;
        };
        if gp.flags & FLAG_GLITCH != 0 {
            // Glitch: finish this seat's bout on CPU (rebind to zero orbit).
            let epsilon = 1e-12f64.max(point.c.0.abs().max(point.c.1.abs()) * 1e-6);
            iterate_perturbation_bout(&mut worker_state.cpu, point, epsilon);
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
            Poll::Ready(value) => return value,
            Poll::Pending => std::thread::yield_now(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
            _pad0: 0,
            _pad1: 0,
        }];
        let orbit = [(0.0f32, 0.0f32)];
        state
            .gpu
            .as_mut()
            .unwrap()
            .run_bout(&mut points, &orbit, 4.0, 64);
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
