use std::future::Future;
use std::pin::Pin;
use std::sync::mpsc;
use std::task::{Context, Poll, Wake, Waker};
use std::sync::Arc;

use bytemuck::{Pod, Zeroable};
use wgpu::util::DeviceExt;

use crate::assemblies::structs::*;
use crate::assemblies::workgroup_new::structs::*;
use crate::assemblies::workgroup_new::structs::mandelbrotable::*;
use crate::assemblies::workgroup_new::workcore::mandelbrot::*;
use crate::assemblies::workgroup_new::workcore::mandelbrot::worker_implementations::periodicity_detector::*;
use crate::constants::*;
use crate::range::*;

pub struct NaiveGpuWorker;

pub type GpuWorkerBatchN = [(); GPU_WORKER_BATCH_N];

pub struct NaiveGpuWorkerState {
    pub bailout_radius_squared: f32
    , pub iterations_per_bout: u32
    , pub stencil: Option<PointStencil>
    , gpu: Option<GpuContext>
}

struct GpuContext {
    device: wgpu::Device
    , queue: wgpu::Queue
    , pipeline: wgpu::ComputePipeline
    , bind_group: wgpu::BindGroup
    , uniform_buffer: wgpu::Buffer
    , point_buffer: wgpu::Buffer
    , staging_buffer: wgpu::Buffer
    , point_capacity: u32
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
struct GpuUniforms {
    bailout_radius_squared: f32
    , bout_iterations: u32
    , confirm_iterations: u32
    , point_count: u32
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
struct GpuPoint {
    c_re: f32
    , c_im: f32
    , z_re: f32
    , z_im: f32
    , d_re: f32
    , d_im: f32
    , checkpoint_re: f32
    , checkpoint_im: f32
    , min_magnitude: f32
    , epsilon: f32
    , iteration_count: u32
    , min_magnitude_time: u32
    , steps_since_checkpoint: u32
    , next_checkpoint_iteration: u32
    , detected_period: u32
    , flags: u32
    , local_x: u32
    , local_y: u32
}

const FLAG_ACTIVE: u32 = 1;
const FLAG_ESCAPED: u32 = 2;
const FLAG_FINISHED: u32 = 4;

impl Default for NaiveGpuWorkerState {
    fn default() -> Self {
        NaiveGpuWorkerState {
            bailout_radius_squared: 4.0
            , iterations_per_bout: 1000
            , stencil: None
            , gpu: None
        }
    }
}

impl NaiveGpuWorkerState {
    fn ensure_gpu(&mut self) -> Option<&mut GpuContext> {
        if self.gpu.is_none() {
            self.gpu = GpuContext::new().ok();
        }
        self.gpu.as_mut()
    }
}

impl GpuContext {
    fn new() -> Result<Self, String> {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
        let adapter = match block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance
            , compatible_surface: None
            , force_fallback_adapter: false
        })) {
            Ok(adapter) => adapter
            , Err(_) => {
                block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
                    power_preference: wgpu::PowerPreference::LowPower
                    , compatible_surface: None
                    , force_fallback_adapter: true
                })).map_err(|e| format!("gpu adapter: {e:?}"))?
            }
        };
        let (device, queue) = block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("naive_gpu_worker")
            , required_features: wgpu::Features::empty()
            , required_limits: wgpu::Limits::default()
            , memory_hints: wgpu::MemoryHints::default()
            , trace: wgpu::Trace::Off
        })).map_err(|e| format!("gpu device: {e:?}"))?;
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("naive_gpu_bout")
            , source: wgpu::ShaderSource::Wgsl(include_str!("naive_gpu_bout.wgsl").into())
        });
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("naive_gpu_bout_bgl")
            , entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0
                    , visibility: wgpu::ShaderStages::COMPUTE
                    , ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform
                        , has_dynamic_offset: false
                        , min_binding_size: None
                    }
                    , count: None
                }
                , wgpu::BindGroupLayoutEntry {
                    binding: 1
                    , visibility: wgpu::ShaderStages::COMPUTE
                    , ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false }
                        , has_dynamic_offset: false
                        , min_binding_size: None
                    }
                    , count: None
                }
            ]
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("naive_gpu_bout_pll")
            , bind_group_layouts: &[&bind_group_layout]
            , push_constant_ranges: &[]
        });
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("naive_gpu_bout_pipeline")
            , layout: Some(&pipeline_layout)
            , module: &shader
            , entry_point: Some("main")
            , compilation_options: Default::default()
            , cache: None
        });
        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("naive_gpu_uniforms")
            , size: std::mem::size_of::<GpuUniforms>() as u64
            , usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST
            , mapped_at_creation: false
        });
        let point_capacity = GPU_WORKER_BATCH_N as u32;
        let point_bytes = (point_capacity as u64) * (std::mem::size_of::<GpuPoint>() as u64);
        let point_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("naive_gpu_points")
            , size: point_bytes
            , usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC
            , mapped_at_creation: false
        });
        let staging_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("naive_gpu_staging")
            , size: point_bytes
            , usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST
            , mapped_at_creation: false
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("naive_gpu_bout_bg")
            , layout: &bind_group_layout
            , entries: &[
                wgpu::BindGroupEntry {
                    binding: 0
                    , resource: uniform_buffer.as_entire_binding()
                }
                , wgpu::BindGroupEntry {
                    binding: 1
                    , resource: point_buffer.as_entire_binding()
                }
            ]
        });
        Ok(GpuContext {
            device
            , queue
            , pipeline
            , bind_group
            , uniform_buffer
            , point_buffer
            , staging_buffer
            , point_capacity
        })
    }

    fn run_bout(
        &mut self
        , points: &mut [GpuPoint]
        , bailout_radius_squared: f32
        , bout_iterations: u32
    ) {
        let count = points.len().min(self.point_capacity as usize) as u32;
        if count == 0 {
            return;
        }
        let uniforms = GpuUniforms {
            bailout_radius_squared
            , bout_iterations
            , confirm_iterations: PERIOD_CONFIRMATION_ITERATIONS
            , point_count: count
        };
        self.queue.write_buffer(
            &self.uniform_buffer
            , 0
            , bytemuck::bytes_of(&uniforms)
        );
        self.queue.write_buffer(
            &self.point_buffer
            , 0
            , bytemuck::cast_slice(&points[..count as usize])
        );
        let copy_bytes = (count as u64) * (std::mem::size_of::<GpuPoint>() as u64);
        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("naive_gpu_bout_enc")
        });
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("naive_gpu_bout_pass")
                , timestamp_writes: None
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.bind_group, &[]);
            pass.dispatch_workgroups((count + 63) / 64, 1, 1);
        }
        encoder.copy_buffer_to_buffer(
            &self.point_buffer
            , 0
            , &self.staging_buffer
            , 0
            , copy_bytes
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
            let out: &[GpuPoint] = bytemuck::cast_slice(&data);
            points[..count as usize].copy_from_slice(out);
        }
        self.staging_buffer.unmap();
    }
}

impl Worker<f32, GpuPeriodicityDetector> for NaiveGpuWorker {
    type State = NaiveGpuWorkerState;

    fn initialize_batch<const N: usize>(
        worker_state: &Self::State
        , active_tile: &Tile<()>
        , seats: [Option<(usize, usize)>; N]
    ) -> PointBatch<f32, GpuPeriodicityDetector, N> {
        let Some(stencil) = worker_state.stencil.as_ref() else {
            return PointBatch { points: [const { None }; N] };
        };
        let generator = match stencil.get_c_generator::<f32>() {
            Some(g) => g
            , None => {
                return PointBatch { points: [const { None }; N] };
            }
        };
        let mut points: [Option<((usize, usize), ActivePoint<f32, GpuPeriodicityDetector>)>; N] =
            [const { None }; N];
        for i in 0..N {
            if let Some(local) = seats[i] {
                let seat = active_tile.screen_seat(local);
                if seat.0 >= stencil.resolution.0
                    || seat.1 >= stencil.resolution.1
                {
                    continue;
                }
                let c = generator.get_c((
                    seat.0.min(u16::MAX as usize) as u16
                    , seat.1.min(u16::MAX as usize) as u16
                ));
                let z = (f32::ZERO, f32::ZERO);
                let derivative = (f32::ONE, f32::ZERO);
                points[i] = Some((
                    local
                    , ActivePoint {
                        c
                        , z
                        , derivative
                        , real_squared: f32::ZERO
                        , imag_squared: f32::ZERO
                        , real_imag: f32::ZERO
                        , iteration_count: 0
                        , min_magnitude: f32::MAX
                        , min_magnitude_time: 0
                        , periodicity_detector: GpuPeriodicityDetector::init(0, z, derivative)
                        , escaped: false
                        , finished: false
                        , orbit_id: ZERO_ORBIT_ID
                        , seat_linear: 0
                    }
                ));
            }
        }
        PointBatch { points }
    }

    fn workshift_on_batch<const N: usize>(
        worker_state: &mut Self::State
        , active_batch: &mut PointBatch<f32, GpuPeriodicityDetector, N>
    ) -> bool {
        let mut packed = Vec::with_capacity(N);
        let mut slots = Vec::with_capacity(N);
        for (i, slot) in active_batch.points.iter().enumerate() {
            if let Some((local, point)) = slot {
                if point.finished {
                    continue;
                }
                slots.push(i);
                packed.push(point_to_gpu(local, point));
            }
        }
        if packed.is_empty() {
            return false;
        }
        let bailout_radius_squared = worker_state.bailout_radius_squared;
        let iterations_per_bout = worker_state.iterations_per_bout;
        if let Some(gpu) = worker_state.ensure_gpu() {
            gpu.run_bout(
                &mut packed
                , bailout_radius_squared
                , iterations_per_bout
            );
            for (packed_i, slot_i) in slots.iter().enumerate() {
                if let Some((_, point)) = active_batch.points[*slot_i].as_mut() {
                    apply_gpu(point, &packed[packed_i]);
                }
            }
        } else {
            for slot_i in slots {
                if let Some((_, point)) = active_batch.points[slot_i].as_mut() {
                    let epsilon = 1e-6f32.max(point.c.0.abs().max(point.c.1.abs()) * 1e-6);
                    iterate_point_bout_f32(
                        point
                        , worker_state.bailout_radius_squared
                        , epsilon
                        , worker_state.iterations_per_bout
                    );
                }
            }
        }
        active_batch.points.iter().any(|slot| {
            matches!(slot, Some((_, point)) if !point.finished)
        })
    }

    fn peek_batch<const N: usize>(
        active_batch: &PointBatch<f32, GpuPeriodicityDetector, N>
        , active_tile: &Tile<()>
    ) -> [Option<((usize, usize), CalibratedAnswer)>; N] {
        let _ = active_tile;
        let mut out: [Option<((usize, usize), CalibratedAnswer)>; N] = [const { None }; N];
        for i in 0..N {
            if let Some((seat, point)) = &active_batch.points[i] {
                if point.finished {
                    out[i] = Some((*seat, point_to_calibrated_answer(point)));
                }
            }
        }
        out
    }

    fn pack_batches<const N: usize, const B: usize>(
        batches: [PointBatch<f32, GpuPeriodicityDetector, N>; B]
    ) -> [Option<PointBatch<f32, GpuPeriodicityDetector, N>>; B] {
        batches.map(Some)
    }
}

fn point_to_gpu(
    local: &(usize, usize)
    , point: &ActivePoint<f32, GpuPeriodicityDetector>
) -> GpuPoint {
    let mut flags = FLAG_ACTIVE;
    if point.escaped {
        flags |= FLAG_ESCAPED;
    }
    if point.finished {
        flags |= FLAG_FINISHED;
    }
    GpuPoint {
        c_re: point.c.0
        , c_im: point.c.1
        , z_re: point.z.0
        , z_im: point.z.1
        , d_re: point.derivative.0
        , d_im: point.derivative.1
        , checkpoint_re: point.periodicity_detector.checkpoint_z.0
        , checkpoint_im: point.periodicity_detector.checkpoint_z.1
        , min_magnitude: point.min_magnitude
        , epsilon: 1e-6f32.max(point.c.0.abs().max(point.c.1.abs()) * 1e-6)
        , iteration_count: point.iteration_count.min(u32::MAX as u64) as u32
        , min_magnitude_time: point.min_magnitude_time.min(u32::MAX as u64) as u32
        , steps_since_checkpoint: point.periodicity_detector.steps_since_checkpoint.min(u32::MAX as u64) as u32
        , next_checkpoint_iteration: point.periodicity_detector.next_checkpoint_iteration.min(u32::MAX as u64) as u32
        , detected_period: point.periodicity_detector.detected_period().unwrap_or(0).min(u32::MAX as u64) as u32
        , flags
        , local_x: local.0 as u32
        , local_y: local.1 as u32
    }
}

fn apply_gpu(point: &mut ActivePoint<f32, GpuPeriodicityDetector>, gpu: &GpuPoint) {
    point.c = (gpu.c_re, gpu.c_im);
    point.z = (gpu.z_re, gpu.z_im);
    point.derivative = (gpu.d_re, gpu.d_im);
    point.real_squared = gpu.z_re * gpu.z_re;
    point.imag_squared = gpu.z_im * gpu.z_im;
    point.real_imag = gpu.z_re * gpu.z_im;
    point.iteration_count = gpu.iteration_count as u64;
    point.min_magnitude = gpu.min_magnitude;
    point.min_magnitude_time = gpu.min_magnitude_time as u64;
    point.escaped = (gpu.flags & FLAG_ESCAPED) != 0;
    point.finished = (gpu.flags & FLAG_FINISHED) != 0;
    point.periodicity_detector.checkpoint_z = (gpu.checkpoint_re, gpu.checkpoint_im);
    point.periodicity_detector.steps_since_checkpoint = gpu.steps_since_checkpoint as u64;
    point.periodicity_detector.next_checkpoint_iteration = gpu.next_checkpoint_iteration as u64;
    point.periodicity_detector.detected_period = if gpu.detected_period == 0 {
        None
    } else {
        Some(gpu.detected_period as u64)
    };
}

fn iterate_point_bout_f32(
    point: &mut ActivePoint<f32, GpuPeriodicityDetector>
    , bailout_radius_squared: f32
    , epsilon: f32
    , bout_iterations: u32
) {
    let mut z_re = point.z.0;
    let mut z_im = point.z.1;
    let mut d_re = point.derivative.0;
    let mut d_im = point.derivative.1;
    let c_re = point.c.0;
    let c_im = point.c.1;
    for _ in 0..bout_iterations {
        if point.finished { break; }
        let old_re = z_re;
        let old_im = z_im;
        let new_d_re = 2.0 * (old_re * d_re - old_im * d_im);
        let new_d_im = 2.0 * (old_re * d_im + old_im * d_re);
        d_re = new_d_re;
        d_im = new_d_im;
        z_re = old_re * old_re - old_im * old_im + c_re;
        z_im = 2.0 * old_re * old_im + c_im;
        point.iteration_count += 1;
        point.real_squared = z_re * z_re;
        point.imag_squared = z_im * z_im;
        point.real_imag = z_re * z_im;
        let rad = z_re * z_re + z_im * z_im;
        if rad < point.min_magnitude {
            point.min_magnitude = rad;
            point.min_magnitude_time = point.iteration_count;
        }
        if rad > bailout_radius_squared {
            point.z = (z_re, z_im);
            point.derivative = (d_re, d_im);
            point.escaped = true;
            point.finished = true;
            break;
        }
        if point.periodicity_detector.check_periodicity(
            point.c
            , (z_re, z_im)
            , (d_re, d_im)
            , point.iteration_count
            , epsilon
        ).is_some() {
            point.z = (z_re, z_im);
            point.derivative = (d_re, d_im);
            point.finished = true;
            break;
        }
    }
    if !point.finished {
        point.z = (z_re, z_im);
        point.derivative = (d_re, d_im);
    }
}

fn exact_range<T: crate::range::Value>(value: T) -> crate::range::Range<T> {
    crate::range::Range { lower_bound: value, upper_bound: value }
}

fn point_to_calibrated_answer(point: &ActivePoint<f32, GpuPeriodicityDetector>) -> CalibratedAnswer {
    if point.escaped {
        CalibratedAnswer {
            result: CalibratedMandelbrotResult::Outside {
                escape_time_r2: exact_range(point.iteration_count)
                , escape_z: (exact_range(point.z.0), exact_range(point.z.1))
            }
            , min_magnitude_time: exact_range(point.min_magnitude_time)
            , min_magnitude: exact_range(point.min_magnitude as f64)
            , highlights: CalibratedHighlights {
                in_filament: exact_range(false)
                , out_filament: exact_range(false)
                , small_time_edge: exact_range(false)
                , node: exact_range(false)
            }
        }
    } else {
        CalibratedAnswer {
            result: CalibratedMandelbrotResult::Inside {
                period: exact_range(0)
            }
            , min_magnitude_time: exact_range(point.min_magnitude_time)
            , min_magnitude: exact_range(point.min_magnitude as f64)
            , highlights: CalibratedHighlights {
                in_filament: exact_range(false)
                , out_filament: exact_range(false)
                , small_time_edge: exact_range(false)
                , node: exact_range(false)
            }
        }
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
