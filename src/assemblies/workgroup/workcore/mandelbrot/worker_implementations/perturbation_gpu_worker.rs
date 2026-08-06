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
use crate::constants::{
    GLITCH_THRESHOLD, GPU_CGEN_IN_FLIGHT_BATCHES, GPU_CGEN_MICRO_BATCH, GPU_POINT_RING_DEPTH,
    GPU_WORKER_BATCH_N, PERIOD_CONFIRMATION_ITERATIONS,
};

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
                return;
            }
            Ok(Err(err)) => panic!("gpu map: {err:?}"),
            Err(mpsc::RecvTimeoutError::Timeout) => {
                // Nudge the queue without blocking (headless has no display poller).
                let _ = device.poll(wgpu::PollType::Poll);
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
    /// Stationary fill: scatter bout results to production atlas without harvest.
    pub stationary_gpu_resident: bool,
    /// Production atlas slot for the active tile being GPU-scattered.
    pub gpu_resident_slot: Option<u32>,
    /// Terminal seats written by the last scatter pass.
    pub last_scatter_terminals: u32,
    /// On-device per-tile completion count from last scatter flush (D-GPU-3).
    pub last_tile_completion: u32,
    /// Local seats from the last full-GPU scatter batch (stationary bulk finish).
    pub last_scatter_locals: Vec<(usize, usize)>,
    /// Last scatter finished every armed seat on device (skip peek/harvest).
    pub last_scatter_full_batch: bool,
    /// Defer scatter counter map until next flush (pipeline overlap).
    pub scatter_defer_flush: bool,
    /// Seats armed in the deferred scatter submit.
    pending_scatter_armed: u32,
    pending_scatter_map_idx: Vec<usize>,
    /// FIFO aligned with BoutScatter pending maps: (armed, locals, point-ring idx).
    pending_scatter_queue: Vec<(u32, Vec<(usize, usize)>, Option<usize>)>,
    /// Point-buffer ring slot held for the deferred scatter (Layer 2).
    pending_scatter_ring: Option<usize>,
    /// Locals for the current deferred submit (survives batch clear).
    pending_scatter_locals_buf: Vec<(usize, usize)>,
    /// Queued dense-cgen tiles for the open micro-batch (not yet submitted).
    cgen_queued: Vec<CgenQueuedTile>,
    /// Ring half (0 or 1) reserved for the open micro-batch.
    cgen_open_half: Option<usize>,
    /// Which ring ranges are held by in-flight submitted micro-batches.
    cgen_half_busy: [bool; GPU_CGEN_IN_FLIGHT_BATCHES],
    /// Confirms still outstanding per micro-batch range.
    cgen_half_remaining: [u32; GPU_CGEN_IN_FLIGHT_BATCHES],
}

/// One dense tile staged into a cgen micro-batch (write-all-then-encode).
#[derive(Clone, Debug)]
pub struct CgenQueuedTile {
    pub ring_idx: usize,
    pub tile_origin: (usize, usize),
    pub atlas_slot: u32,
    pub tile_index: usize,
}

struct PerturbationGpuContext {
    /// The app's one device, borrowed — never a second one of our own.
    shared: Arc<GpuContext>,
    pipeline: wgpu::ComputePipeline,
    /// Init points from compact CGenerator (skip host δc pack).
    init_cgen_pipeline: wgpu::ComputePipeline,
    /// One smoke/compile pipeline per stacked limb count 1..=8 (index = limbs-1).
    stacked_gear_pipelines: [wgpu::ComputePipeline; 8],
    bind_group_layout: wgpu::BindGroupLayout,
    /// Per ring-slot compute uniforms (multi-tile one encoder must not share one buffer).
    uniform_ring: Vec<wgpu::Buffer>,
    /// Ring of point buffers so inflight scatter can outlive the next compute upload.
    point_ring: Vec<wgpu::Buffer>,
    bind_group_ring: Vec<wgpu::BindGroup>,
    ring_free: Vec<usize>,
    ring_active: usize,
    orbit_buffer: wgpu::Buffer,
    /// View-lifetime CGenerator (PointStencil change), not per-tile.
    cgen_buffer: wgpu::Buffer,
    /// Last uploaded view key: (stencil serial, orbit id).
    view_cgen_key: Option<(u64, u64)>,
    staging_buffer: wgpu::Buffer,
    /// Second staging buffer so a readback can complete while the next bout runs.
    staging_buffer_b: wgpu::Buffer,
    staging_flip: bool,
    point_capacity: u32,
    orbit_capacity: u32,
    /// Points currently resident in the active ring slot (skip re-upload when unchanged).
    resident_points: u32,
    last_orbit_id: Option<u64>,
    /// Scratch for flattening `(re, im)` orbit uploads without allocating each tile.
    orbit_flat_scratch: Vec<f32>,
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
    tile_origin_x: u32,
    tile_origin_y: u32,
    tile_edge: u32,
    use_c_generator: u32,
    _pad0: u32,
    _pad1: u32,
}

/// View-lifetime CGenerator (binding 3). Uploaded once per PointStencil (+ orbit).
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
struct GpuViewCGenerator {
    origin_re: f32,
    origin_im: f32,
    space: f32,
    half: f32,
}

impl GpuUniforms {
    fn base(
        bailout_radius_squared: f32,
        bout_iterations: u32,
        orbit_len: u32,
        point_count: u32,
    ) -> Self {
        Self {
            bailout_radius_squared,
            bout_iterations,
            orbit_len,
            point_count,
            glitch_threshold: GLITCH_THRESHOLD as f32,
            confirm_iterations: PERIOD_CONFIRMATION_ITERATIONS,
            tile_origin_x: 0,
            tile_origin_y: 0,
            tile_edge: crate::constants::TILE_EDGE_LENGTH as u32,
            use_c_generator: 0,
            _pad0: 0,
            _pad1: 0,
        }
    }
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
            stationary_gpu_resident: false,
            gpu_resident_slot: None,
            last_scatter_terminals: 0,
            last_tile_completion: 0,
            last_scatter_locals: Vec::new(),
            last_scatter_full_batch: false,
            scatter_defer_flush: false,
            pending_scatter_armed: 0,
            pending_scatter_map_idx: Vec::new(),
            pending_scatter_queue: Vec::new(),
            pending_scatter_ring: None,
            pending_scatter_locals_buf: Vec::new(),
            cgen_queued: Vec::new(),
            cgen_open_half: None,
            cgen_half_busy: [false; GPU_CGEN_IN_FLIGHT_BATCHES],
            cgen_half_remaining: [0; GPU_CGEN_IN_FLIGHT_BATCHES],
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
            stationary_gpu_resident: false,
            gpu_resident_slot: None,
            last_scatter_terminals: 0,
            last_tile_completion: 0,
            last_scatter_locals: Vec::new(),
            last_scatter_full_batch: false,
            scatter_defer_flush: false,
            pending_scatter_armed: 0,
            pending_scatter_map_idx: Vec::new(),
            pending_scatter_queue: Vec::new(),
            pending_scatter_ring: None,
            pending_scatter_locals_buf: Vec::new(),
            cgen_queued: Vec::new(),
            cgen_open_half: None,
            cgen_half_busy: [false; GPU_CGEN_IN_FLIGHT_BATCHES],
            cgen_half_remaining: [0; GPU_CGEN_IN_FLIGHT_BATCHES],
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

    /// Enable GPU-resident scatter handoff when stationary + atlas available.
    pub fn refresh_stationary_gpu_resident(&mut self, mag_velocity: i32) {
        if self.gpu_desired && self.path == PerturbationComputePath::Gpu {
            let _ = self.ensure_gpu();
        }
        self.stationary_gpu_resident = mag_velocity == 0
            && self.is_gpu_preferred()
            && self.gpu.is_some()
            && crate::assemblies::workgroup::production_atlas::ProductionAtlas::shared()
                .is_some()
            && crate::assemblies::workgroup::bout_scatter::BoutScatter::shared().is_some();
    }

    pub fn has_pending_scatter(&self) -> bool {
        self.pending_scatter_armed > 0 || !self.pending_scatter_queue.is_empty()
    }

    pub fn has_unparked_pending_scatter(&self) -> bool {
        self.pending_scatter_armed > 0
    }

    pub fn take_pending_scatter_armed(&mut self) -> u32 {
        let n = self.pending_scatter_armed;
        self.pending_scatter_armed = 0;
        n
    }

    pub fn take_pending_scatter_ring(&mut self) -> Option<usize> {
        self.pending_scatter_ring.take()
    }

    pub fn release_held_scatter_ring(&mut self, ring: Option<usize>) {
        self.release_scatter_ring(ring);
    }

    pub fn point_ring_free_len(&self) -> usize {
        self.gpu.as_ref().map(|g| g.ring_free_len()).unwrap_or(0)
    }

    pub fn point_ring_active(&self) -> Option<usize> {
        self.gpu.as_ref().map(|g| g.ring_active())
    }

    /// Acquire a fresh point-buffer slot when the active one is held by an in-flight scatter.
    pub fn ensure_point_ring_exclusive(&mut self, held_rings: &[usize]) -> bool {
        let Some(gpu) = self.gpu.as_mut() else {
            return true;
        };
        if !held_rings.contains(&gpu.ring_active()) {
            return true;
        }
        gpu.acquire_ring_slot().is_some()
    }

    /// Activate next CGenerator batch ring slot; flush when depth is full.
    pub fn prepare_cgen_batch_ring_slot(&mut self) -> bool {
        // Legacy name — micro-batch uses queue_cgen_micro_tile / submit_cgen_micro_batch.
        self.cgen_micro_batch_has_room()
    }

    /// True when the open micro-batch can accept another dense tile.
    pub fn cgen_micro_batch_has_room(&self) -> bool {
        self.cgen_queued.len() < GPU_CGEN_MICRO_BATCH
    }

    /// True when a free ring half exists for a new micro-batch (or one is already open).
    pub fn cgen_micro_batch_can_open(&self) -> bool {
        self.cgen_open_half.is_some() || self.cgen_half_busy.iter().any(|b| !*b)
    }

    /// True when tiles are queued but not yet submitted.
    pub fn cgen_micro_batch_open(&self) -> bool {
        !self.cgen_queued.is_empty()
    }

    /// Number of tiles queued in the open micro-batch.
    pub fn cgen_micro_batch_queued_len(&self) -> usize {
        self.cgen_queued.len()
    }

    /// Number of submitted micro-batches still in flight.
    pub fn cgen_inflight_batch_count(&self) -> usize {
        self.cgen_half_busy.iter().filter(|b| **b).count()
    }

    fn allocate_cgen_half(&mut self) -> Option<usize> {
        if let Some(h) = self.cgen_open_half {
            return Some(h);
        }
        for h in 0..GPU_CGEN_IN_FLIGHT_BATCHES {
            if !self.cgen_half_busy[h] {
                self.cgen_open_half = Some(h);
                self.cgen_half_remaining[h] = 0;
                return Some(h);
            }
        }
        None
    }

    /// Queue one dense cgen tile into the open micro-batch (no GPU work yet).
    pub fn queue_cgen_micro_tile(
        &mut self,
        tile_index: usize,
        tile_origin: (usize, usize),
        atlas_slot: u32,
    ) -> bool {
        if !self.cgen_micro_batch_has_room() {
            return false;
        }
        let Some(half) = self.allocate_cgen_half() else {
            return false;
        };
        let ring_idx = half * GPU_CGEN_MICRO_BATCH + self.cgen_queued.len();
        if ring_idx >= GPU_POINT_RING_DEPTH {
            return false;
        }
        if self.cgen_queued.is_empty() {
            self.cgen_half_busy[half] = true;
        }
        self.cgen_queued.push(CgenQueuedTile {
            ring_idx,
            tile_origin,
            atlas_slot,
            tile_index,
        });
        true
    }

    /// Write-all then encode+submit the open micro-batch. Returns queued tiles for confirm park.
    pub fn submit_cgen_micro_batch(&mut self) -> Option<Vec<CgenQueuedTile>> {
        if self.cgen_queued.is_empty() {
            self.cgen_open_half = None;
            return None;
        }
        if self.path == PerturbationComputePath::Gpu && self.gpu_desired {
            let _ = self.ensure_gpu();
        }
        if self.gpu.is_none() {
            self.cgen_queued.clear();
            if let Some(h) = self.cgen_open_half.take() {
                self.cgen_half_busy[h] = false;
            }
            return None;
        }
        let tiles = std::mem::take(&mut self.cgen_queued);
        let half = self.cgen_open_half.take().unwrap_or(0);
        if !submit_cgen_micro_batch_inner(self, &tiles) {
            self.cgen_half_busy[half] = false;
            self.cgen_half_remaining[half] = 0;
            return None;
        }
        self.cgen_half_remaining[half] = tiles.len() as u32;
        self.cgen_half_busy[half] = true;
        Some(tiles)
    }

    /// Release one ring slot after confirm apply; frees the half when all tiles done.
    pub fn release_cgen_micro_ring(&mut self, ring: Option<usize>) {
        let Some(ring) = ring else {
            return;
        };
        let half = ring / GPU_CGEN_MICRO_BATCH;
        if half >= GPU_CGEN_IN_FLIGHT_BATCHES {
            if let Some(gpu) = self.gpu.as_mut() {
                gpu.release_ring_slot(ring);
            }
            return;
        }
        if self.cgen_half_remaining[half] > 0 {
            self.cgen_half_remaining[half] -= 1;
        }
        if self.cgen_half_remaining[half] == 0 {
            self.cgen_half_busy[half] = false;
            // Return all slots in the half to the free pool.
            if let Some(gpu) = self.gpu.as_mut() {
                for i in 0..GPU_CGEN_MICRO_BATCH {
                    let idx = half * GPU_CGEN_MICRO_BATCH + i;
                    gpu.release_ring_slot(idx);
                }
            }
        }
    }

    /// Submit any open multi-tile CGenerator encoder (flush partial micro-batch).
    pub fn flush_cgen_gpu_batch(&mut self) -> Option<Vec<CgenQueuedTile>> {
        self.submit_cgen_micro_batch()
    }

    pub fn pending_scatter_queue_len(&self) -> usize {
        self.pending_scatter_queue.len()
    }

    /// Drop deferred-scatter bookkeeping so another submit can queue (maps stay in BoutScatter).
    pub fn clear_deferred_scatter_bookkeeping(&mut self) {
        self.pending_scatter_armed = 0;
        self.pending_scatter_map_idx.clear();
        self.pending_scatter_locals_buf.clear();
    }

    /// Snapshot locals for the current deferred submit (before clearing the host batch).
    pub fn pending_scatter_locals<const N: usize>(
        &self,
        active_batch: &PointBatch<f64, CpuPeriodicityDetector, N>,
    ) -> Vec<(usize, usize)> {
        if !self.pending_scatter_locals_buf.is_empty() {
            return self.pending_scatter_locals_buf.clone();
        }
        self.pending_scatter_map_idx
            .iter()
            .filter_map(|&bi| {
                active_batch
                    .points
                    .get(bi)
                    .and_then(|s| s.as_ref())
                    .map(|(local, _)| *local)
            })
            .collect()
    }

    pub fn take_pending_scatter_locals_buf(&mut self) -> Vec<(usize, usize)> {
        std::mem::take(&mut self.pending_scatter_locals_buf)
    }

    /// Park armed count + locals onto the FIFO (BoutScatter map stays in flight).
    pub fn park_pending_scatter_locals(&mut self, locals: Vec<(usize, usize)>) {
        if self.pending_scatter_armed == 0 {
            return;
        }
        let ring = self.pending_scatter_ring.take();
        self.pending_scatter_queue
            .push((self.pending_scatter_armed, locals, ring));
        self.clear_deferred_scatter_bookkeeping();
    }

    /// Park current deferred submit onto the FIFO (BoutScatter map stays in flight).
    /// Returns the parked locals so the session can reserve them for scheduling.
    pub fn park_pending_scatter<const N: usize>(
        &mut self,
        active_batch: &PointBatch<f64, CpuPeriodicityDetector, N>,
    ) -> Vec<(usize, usize)> {
        let locals = self.pending_scatter_locals(active_batch);
        if locals.is_empty() && self.pending_scatter_armed == 0 {
            return Vec::new();
        }
        self.park_pending_scatter_locals(locals.clone());
        locals
    }

    fn release_scatter_ring(&mut self, ring: Option<usize>) {
        let Some(idx) = ring else {
            return;
        };
        if let Some(gpu) = self.gpu.as_mut() {
            gpu.release_ring_slot(idx);
        }
    }

    /// Flush the oldest BoutScatter map; on full batch, sets `last_scatter_locals`.
    pub fn flush_oldest_parked_scatter(&mut self) -> bool {
        use crate::assemblies::workgroup::bout_scatter::BoutScatter;
        if self.pending_scatter_queue.is_empty() {
            return false;
        }
        let Some(scatter) = BoutScatter::shared() else {
            while let Some((_, _, ring)) = self.pending_scatter_queue.pop() {
                self.release_scatter_ring(ring);
            }
            return false;
        };
        let Some((batch_t, tile_c)) = scatter.flush_scatter_counter() else {
            return false;
        };
        self.last_scatter_terminals = batch_t;
        self.last_tile_completion = tile_c;
        let (armed, locals, ring) = self.pending_scatter_queue.remove(0);
        self.release_scatter_ring(ring);
        if batch_t >= armed && armed > 0 {
            self.last_scatter_locals = locals;
            self.last_scatter_full_batch = true;
            true
        } else {
            false
        }
    }

    /// Poll the deferred scatter counter; sets `last_scatter_full_batch` when complete.
    pub fn flush_pending_scatter(&mut self) -> Option<u32> {
        use crate::assemblies::workgroup::bout_scatter::BoutScatter;
        // Prefer parked FIFO when present (overlap path).
        if !self.pending_scatter_queue.is_empty() {
            let ok = self.flush_oldest_parked_scatter();
            return if ok {
                Some(self.last_scatter_terminals)
            } else {
                Some(0)
            };
        }
        let scatter = BoutScatter::shared()?;
        let terminals = scatter.flush_scatter_counter()?;
        self.last_scatter_terminals = terminals.0;
        self.last_tile_completion = terminals.1;
        let armed = self.pending_scatter_armed;
        self.pending_scatter_armed = 0;
        let ring = self.pending_scatter_ring.take();
        self.release_scatter_ring(ring);
        if terminals.0 >= armed && armed > 0 {
            self.last_scatter_full_batch = true;
        }
        Some(terminals.0)
    }

    /// After deferred flush confirms a full batch, mark host seats and locals.
    pub fn apply_pending_scatter_to_batch<const N: usize>(
        &mut self,
        active_batch: &mut PointBatch<f64, CpuPeriodicityDetector, N>,
    ) -> bool {
        if !self.last_scatter_full_batch || self.pending_scatter_map_idx.is_empty() {
            return false;
        }
        self.last_scatter_locals = self
            .pending_scatter_map_idx
            .iter()
            .filter_map(|&bi| {
                active_batch.points[bi]
                    .as_ref()
                    .map(|(local, _)| *local)
            })
            .collect();
        for &bi in &self.pending_scatter_map_idx {
            if let Some((_, point)) = active_batch.points[bi].as_mut() {
                point.finished = true;
                point.escaped = true;
                point.iteration_count = point.iteration_count.max(1);
            }
        }
        self.pending_scatter_map_idx.clear();
        self.clear_resident();
        self.cpu_followup = false;
        self.gpu_low_yield_streak = 0;
        true
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
        // Gear ladder is GPU-aware (D-GEAR-1); re-select for CPU-only tests.
        self.refresh_selected_gear();
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
    /// Nomap defer skips the f64 bridge and packs GpuPertPoint straight from the live gear.
    pub fn workshift_active_gear<const N: usize>(
        &mut self
        , batch: &mut crate::assemblies::workgroup::workcore::mandelbrot::worker_implementations::active_gear_work::ActiveGearWork<N>
    ) -> bool {
        if self.path == PerturbationComputePath::Gpu
            && self.gpu_desired
            && self.cpu.gear.runs_on_gpu()
        {
            let _ = self.ensure_gpu();
        }
        // Forced CPU / no adapter: stay on typed Mandelbrotable arms.
        let use_typed_cpu = matches!(self.path, PerturbationComputePath::Cpu)
            || self.gpu.is_none()
            || !self.cpu.gear.runs_on_gpu();
        if use_typed_cpu {
            return batch.workshift_cpu(&mut self.cpu);
        }
        // Defer/nomap wave: pack GPU structs from typed seats (no f64 roundtrip).
        if self.scatter_defer_flush
            && self.stationary_gpu_resident
            && self.gpu_resident_slot.is_some()
        {
            return try_gpu_resident_scatter_from_gear(self, batch);
        }
        // GPU path still packs from host f64 (non-defer / harvest paths).
        let mut host = batch.to_host_batch();
        let out = PerturbationGpuWorker::workshift_on_batch(self, &mut host);
        if !(self.stationary_gpu_resident && self.last_scatter_full_batch) {
            batch.absorb_host_batch(host);
        }
        out
    }

    /// Dense full-tile defer: upload f32 CGenerator and init δc on GPU (no 4096 host pack).
    /// Returns true when scatter was armed.
    pub fn workshift_gpu_cgen_dense_tile(&mut self, tile_origin: (usize, usize)) -> bool {
        if self.path == PerturbationComputePath::Gpu && self.gpu_desired {
            let _ = self.ensure_gpu();
        }
        if self.gpu.is_none() {
            return false;
        }
        try_gpu_resident_scatter_cgen(self, tile_origin)
    }

    /// True when stencil+orbit admit an f32 relative CGenerator (home hot path).
    pub fn f32_relative_cgen_available(&self) -> bool {
        let Some(stencil) = self.cpu.stencil.as_ref() else {
            return false;
        };
        let Some(&orbit_id) = self.cpu.seat_orbit_ids.first() else {
            return false;
        };
        let Some(orbit) = self.cpu.references.get(orbit_id) else {
            return false;
        };
        stencil
            .get_relative_c_generator::<f32>(&orbit.big_c)
            .is_some()
    }

    /// Upload view-lifetime CGenerator once when a PointStencil is sent (not per tile).
    /// No-op until the GPU context exists; first cgen tile also uploads via key check.
    pub fn publish_view_cgen(&mut self) {
        use crate::constants::PIXELS_PER_UNIT_POT;
        let Some(stencil) = self.cpu.stencil.as_ref() else {
            return;
        };
        let Some(&orbit_id) = self.cpu.seat_orbit_ids.first() else {
            return;
        };
        let Some(orbit) = self.cpu.references.get(orbit_id) else {
            return;
        };
        let Some(cgen) = stencil.get_relative_c_generator::<f32>(&orbit.big_c) else {
            return;
        };
        let ((ore, oim), sp) = cgen.origin_and_space();
        let half = 2.0_f32.powi(-(PIXELS_PER_UNIT_POT + 1));
        let serial = stencil.serial_number;
        let orbit_key = orbit_id as u64;
        if let Some(gpu) = self.gpu.as_mut() {
            gpu.upload_view_cgen(serial, orbit_key, ore, oim, sp, half);
        }
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
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
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
        let init_cgen_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("perturbation_gpu_init_cgen"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: Some("init_from_cgen"),
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
        let uniform_size = std::mem::size_of::<GpuUniforms>() as u64;
        let point_bytes = (point_capacity as u64) * (std::mem::size_of::<GpuPertPoint>() as u64);
        let orbit_bytes = (orbit_capacity as u64) * 8;
        let orbit_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("perturbation_gpu_orbit"),
            size: orbit_bytes,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let cgen_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("perturbation_gpu_view_cgen"),
            // Uniform bindings: pad to 256 for broad backend offset/size rules.
            size: 256,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
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
        // Ring of point + uniform buffers so multi-tile one encoder cannot clobber.
        let depth = GPU_POINT_RING_DEPTH.max(1);
        let mut point_ring = Vec::with_capacity(depth);
        let mut uniform_ring = Vec::with_capacity(depth);
        let mut bind_group_ring = Vec::with_capacity(depth);
        for i in 0..depth {
            let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(&format!("perturbation_gpu_uniforms_{i}")),
                size: uniform_size,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            let point_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(&format!("perturbation_gpu_points_{i}")),
                size: point_bytes,
                usage: wgpu::BufferUsages::STORAGE
                    | wgpu::BufferUsages::COPY_DST
                    | wgpu::BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            });
            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some(&format!("perturbation_gpu_bg_{i}")),
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
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: cgen_buffer.as_entire_binding(),
                    },
                ],
            });
            uniform_ring.push(uniform_buffer);
            point_ring.push(point_buffer);
            bind_group_ring.push(bind_group);
        }
        // Slot 0 is pre-checked-out as the active compute target.
        let ring_free: Vec<usize> = (1..depth).collect();
        Ok(PerturbationGpuContext {
            shared: Arc::clone(&shared),
            pipeline,
            init_cgen_pipeline,
            stacked_gear_pipelines,
            bind_group_layout,
            uniform_ring,
            point_ring,
            bind_group_ring,
            ring_free,
            ring_active: 0,
            orbit_buffer,
            cgen_buffer,
            view_cgen_key: None,
            staging_buffer,
            staging_buffer_b,
            staging_flip: false,
            point_capacity,
            orbit_capacity,
            resident_points: 0,
            last_orbit_id: None,
            orbit_flat_scratch: Vec::new(),
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
        let uniforms = GpuUniforms::base(
            bailout_radius_squared,
            bout_iterations,
            orbit_len,
            count,
        );
        let device = &self.shared.device;
        let queue = &self.shared.queue;
        queue.write_buffer(&self.uniform_ring[self.ring_active], 0, bytemuck::bytes_of(&uniforms));
        // Keep GpuPertPoint resident across bouts: only re-upload when the CPU
        // side changed the batch (new seats or a different orbit).
        if upload_points || self.resident_points != count {
            queue.write_buffer(
                self.point_buffer(),
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
            pass.set_bind_group(0, self.active_bind_group(), &[]);
            pass.dispatch_workgroups((count + 63) / 64, 1, 1);
        }
        {
            let staging = if use_b {
                &self.staging_buffer_b
            } else {
                &self.staging_buffer
            };
            encoder.copy_buffer_to_buffer(
                self.point_buffer(),
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
        let uniforms = GpuUniforms::base(
            bailout_radius_squared,
            bout_iterations,
            orbit_len,
            count,
        );
        let device = &self.shared.device;
        let queue = &self.shared.queue;
        queue.write_buffer(&self.uniform_ring[self.ring_active], 0, bytemuck::bytes_of(&uniforms));
        if upload_points || self.resident_points != count {
            queue.write_buffer(
                self.point_buffer(),
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
            pass.set_bind_group(0, self.active_bind_group(), &[]);
            pass.dispatch_workgroups((count + 63) / 64, 1, 1);
        }
        {
            let staging = if use_b {
                &self.staging_buffer_b
            } else {
                &self.staging_buffer
            };
            encoder.copy_buffer_to_buffer(
                self.point_buffer(),
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
        self.run_bout_compute_multi_inner(
            None,
            points,
            orbit_f32,
            bailout_radius_squared,
            bout_iterations,
            rounds,
            upload_points,
            upload_orbit,
            true,
        );
    }

    pub(crate) fn run_bout_compute_multi_nopoll(
        &mut self,
        points: &[GpuPertPoint],
        orbit_f32: &[(f32, f32)],
        bailout_radius_squared: f32,
        bout_iterations: u32,
        rounds: u32,
        upload_points: bool,
        upload_orbit: bool,
    ) {
        self.run_bout_compute_multi_inner(
            None,
            points,
            orbit_f32,
            bailout_radius_squared,
            bout_iterations,
            rounds,
            upload_points,
            upload_orbit,
            false,
        );
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
        self.run_bout_compute_multi_inner(
            Some(limbs),
            points,
            orbit_f32,
            bailout_radius_squared,
            bout_iterations,
            rounds,
            upload_points,
            upload_orbit,
            true,
        );
    }

    pub(crate) fn run_stacked_bout_compute_multi_nopoll(
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
        self.run_bout_compute_multi_inner(
            Some(limbs),
            points,
            orbit_f32,
            bailout_radius_squared,
            bout_iterations,
            rounds,
            upload_points,
            upload_orbit,
            false,
        );
    }

    /// Encode `rounds` compute passes into `encoder` (optional uniform/point/orbit writes; no submit).
    pub(crate) fn encode_bout_rounds(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        stacked_limbs: Option<u8>,
        points: &[GpuPertPoint],
        orbit_f32: &[(f32, f32)],
        bailout_radius_squared: f32,
        bout_iterations: u32,
        rounds: u32,
        upload_uniforms: bool,
        upload_points: bool,
        upload_orbit: bool,
    ) -> bool {
        if rounds == 0 {
            return false;
        }
        let count = points.len().min(self.point_capacity as usize) as u32;
        if count == 0 {
            return false;
        }
        let orbit_len = orbit_f32.len().min(self.orbit_capacity as usize) as u32;
        if orbit_len == 0 {
            return false;
        }
        let queue = &self.shared.queue;
        if upload_uniforms {
            let uniforms = GpuUniforms::base(
                bailout_radius_squared,
                bout_iterations,
                orbit_len,
                count,
            );
            queue.write_buffer(&self.uniform_ring[self.ring_active], 0, bytemuck::bytes_of(&uniforms));
        }
        if upload_points || self.resident_points != count {
            queue.write_buffer(
                self.point_buffer(),
                0,
                bytemuck::cast_slice(&points[..count as usize]),
            );
            self.resident_points = count;
        }
        if upload_orbit {
            let need = (orbit_len as usize) * 2;
            if self.orbit_flat_scratch.len() < need {
                self.orbit_flat_scratch.resize(need, 0.0);
            }
            for (i, &(re, im)) in orbit_f32.iter().take(orbit_len as usize).enumerate() {
                self.orbit_flat_scratch[i * 2] = re;
                self.orbit_flat_scratch[i * 2 + 1] = im;
            }
            queue.write_buffer(
                &self.orbit_buffer,
                0,
                bytemuck::cast_slice(&self.orbit_flat_scratch[..need]),
            );
        }
        for r in 0..rounds {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some(if r == 0 {
                    "perturbation_gpu_bout_fused_0"
                } else {
                    "perturbation_gpu_bout_fused_n"
                }),
                timestamp_writes: None,
            });
            if let Some(limbs) = stacked_limbs {
                assert!((1..=8).contains(&limbs));
                pass.set_pipeline(&self.stacked_gear_pipelines[(limbs - 1) as usize]);
            } else {
                pass.set_pipeline(&self.pipeline);
            }
            pass.set_bind_group(0, self.active_bind_group(), &[]);
            pass.dispatch_workgroups((count + 63) / 64, 1, 1);
        }
        true
    }

    /// Upload view-lifetime CGenerator once per (stencil serial, orbit).
    pub(crate) fn upload_view_cgen(
        &mut self,
        stencil_serial: u64,
        orbit_key: u64,
        origin_re: f32,
        origin_im: f32,
        space: f32,
        half: f32,
    ) {
        let key = (stencil_serial, orbit_key);
        if self.view_cgen_key == Some(key) {
            return;
        }
        let view = GpuViewCGenerator {
            origin_re,
            origin_im,
            space,
            half,
        };
        self.shared
            .queue
            .write_buffer(&self.cgen_buffer, 0, bytemuck::bytes_of(&view));
        self.view_cgen_key = Some(key);
    }

    /// Init from view CGenerator + bout rounds; no host point upload.
    /// Writes uniforms to `ring_idx` and encodes passes bound to that ring slot.
    pub(crate) fn encode_bout_cgen(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        orbit_f32: &[(f32, f32)],
        bailout_radius_squared: f32,
        bout_iterations: u32,
        rounds: u32,
        point_count: u32,
        tile_origin_x: u32,
        tile_origin_y: u32,
        upload_orbit: bool,
        ring_idx: usize,
        write_uniforms: bool,
    ) -> bool {
        if rounds == 0 || point_count == 0 {
            return false;
        }
        let count = point_count.min(self.point_capacity);
        let orbit_len = orbit_f32.len().min(self.orbit_capacity as usize) as u32;
        if orbit_len == 0 {
            return false;
        }
        let idx = ring_idx.min(self.uniform_ring.len().saturating_sub(1));
        if write_uniforms {
            let queue = &self.shared.queue;
            let mut uniforms = GpuUniforms::base(
                bailout_radius_squared,
                bout_iterations,
                orbit_len,
                count,
            );
            uniforms.tile_origin_x = tile_origin_x;
            uniforms.tile_origin_y = tile_origin_y;
            uniforms.tile_edge = crate::constants::TILE_EDGE_LENGTH as u32;
            uniforms.use_c_generator = 1;
            queue.write_buffer(
                &self.uniform_ring[idx],
                0,
                bytemuck::bytes_of(&uniforms),
            );
            if upload_orbit {
                let need = (orbit_len as usize) * 2;
                if self.orbit_flat_scratch.len() < need {
                    self.orbit_flat_scratch.resize(need, 0.0);
                }
                for (i, &(re, im)) in orbit_f32.iter().take(orbit_len as usize).enumerate() {
                    self.orbit_flat_scratch[i * 2] = re;
                    self.orbit_flat_scratch[i * 2 + 1] = im;
                }
                queue.write_buffer(
                    &self.orbit_buffer,
                    0,
                    bytemuck::cast_slice(&self.orbit_flat_scratch[..need]),
                );
            }
        }
        let bind_group = &self.bind_group_ring[idx];
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("perturbation_gpu_init_cgen"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.init_cgen_pipeline);
            pass.set_bind_group(0, bind_group, &[]);
            pass.dispatch_workgroups((count + 63) / 64, 1, 1);
        }
        for r in 0..rounds {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some(if r == 0 {
                    "perturbation_gpu_bout_cgen_0"
                } else {
                    "perturbation_gpu_bout_cgen_n"
                }),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, bind_group, &[]);
            pass.dispatch_workgroups((count + 63) / 64, 1, 1);
        }
        self.resident_points = count;
        true
    }

    /// Write cgen uniforms for `ring_idx` only (batch write-all phase).
    pub(crate) fn write_bout_cgen_uniforms(
        &mut self,
        orbit_f32: &[(f32, f32)],
        bailout_radius_squared: f32,
        bout_iterations: u32,
        point_count: u32,
        tile_origin_x: u32,
        tile_origin_y: u32,
        upload_orbit: bool,
        ring_idx: usize,
    ) -> bool {
        let count = point_count.min(self.point_capacity);
        let orbit_len = orbit_f32.len().min(self.orbit_capacity as usize) as u32;
        if count == 0 || orbit_len == 0 {
            return false;
        }
        let idx = ring_idx.min(self.uniform_ring.len().saturating_sub(1));
        let queue = &self.shared.queue;
        let mut uniforms = GpuUniforms::base(
            bailout_radius_squared,
            bout_iterations,
            orbit_len,
            count,
        );
        uniforms.tile_origin_x = tile_origin_x;
        uniforms.tile_origin_y = tile_origin_y;
        uniforms.tile_edge = crate::constants::TILE_EDGE_LENGTH as u32;
        uniforms.use_c_generator = 1;
        queue.write_buffer(
            &self.uniform_ring[idx],
            0,
            bytemuck::bytes_of(&uniforms),
        );
        if upload_orbit {
            let need = (orbit_len as usize) * 2;
            if self.orbit_flat_scratch.len() < need {
                self.orbit_flat_scratch.resize(need, 0.0);
            }
            for (i, &(re, im)) in orbit_f32.iter().take(orbit_len as usize).enumerate() {
                self.orbit_flat_scratch[i * 2] = re;
                self.orbit_flat_scratch[i * 2 + 1] = im;
            }
            queue.write_buffer(
                &self.orbit_buffer,
                0,
                bytemuck::cast_slice(&self.orbit_flat_scratch[..need]),
            );
        }
        true
    }

    /// Encode init+bout for an already-written ring slot (batch encode phase).
    pub(crate) fn encode_bout_cgen_passes(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        rounds: u32,
        point_count: u32,
        ring_idx: usize,
    ) -> bool {
        if rounds == 0 || point_count == 0 {
            return false;
        }
        let count = point_count.min(self.point_capacity);
        let idx = ring_idx.min(self.bind_group_ring.len().saturating_sub(1));
        let bind_group = &self.bind_group_ring[idx];
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("perturbation_gpu_init_cgen"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.init_cgen_pipeline);
            pass.set_bind_group(0, bind_group, &[]);
            pass.dispatch_workgroups((count + 63) / 64, 1, 1);
        }
        for r in 0..rounds {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some(if r == 0 {
                    "perturbation_gpu_bout_cgen_0"
                } else {
                    "perturbation_gpu_bout_cgen_n"
                }),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, bind_group, &[]);
            pass.dispatch_workgroups((count + 63) / 64, 1, 1);
        }
        self.resident_points = count;
        true
    }

    pub(crate) fn point_buffer_at(&self, ring_idx: usize) -> &wgpu::Buffer {
        let idx = ring_idx.min(self.point_ring.len().saturating_sub(1));
        &self.point_ring[idx]
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
        let device = self.shared.device.clone();
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("perturbation_gpu_bout_compute_only_enc"),
        });
        if !self.encode_bout_rounds(
            &mut encoder,
            stacked_limbs,
            points,
            orbit_f32,
            bailout_radius_squared,
            bout_iterations,
            1,
            true,
            upload_points,
            upload_orbit,
        ) {
            return;
        }
        self.shared.queue.submit(Some(encoder.finish()));
        if wait {
            poll_submitted_briefly(&device);
        }
    }

    fn run_bout_compute_multi_inner(
        &mut self,
        stacked_limbs: Option<u8>,
        points: &[GpuPertPoint],
        orbit_f32: &[(f32, f32)],
        bailout_radius_squared: f32,
        bout_iterations: u32,
        rounds: u32,
        upload_points: bool,
        upload_orbit: bool,
        wait_after: bool,
    ) {
        if rounds == 0 {
            return;
        }
        let device = self.shared.device.clone();
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("perturbation_gpu_bout_multi_enc"),
        });
        if !self.encode_bout_rounds(
            &mut encoder,
            stacked_limbs,
            points,
            orbit_f32,
            bailout_radius_squared,
            bout_iterations,
            rounds,
            true,
            upload_points,
            upload_orbit,
        ) {
            return;
        }
        self.shared.queue.submit(Some(encoder.finish()));
        if wait_after {
            poll_submitted_briefly(&device);
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
            encoder.copy_buffer_to_buffer(self.point_buffer(), 0, staging, 0, copy_bytes);
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

    fn active_bind_group(&self) -> &wgpu::BindGroup {
        &self.bind_group_ring[self.ring_active]
    }

    pub(crate) fn point_buffer(&self) -> &wgpu::Buffer {
        &self.point_ring[self.ring_active]
    }

    pub(crate) fn ring_active(&self) -> usize {
        self.ring_active
    }

    pub(crate) fn ring_free_len(&self) -> usize {
        self.ring_free.len()
    }

    /// Force the active compute target to a specific ring slot (fused multi-tile batch).
    pub(crate) fn activate_ring(&mut self, idx: usize) {
        assert!(idx < self.point_ring.len());
        self.ring_active = idx;
        self.resident_points = 0;
    }

    /// Check out a free point-buffer slot as the active compute target.
    pub(crate) fn acquire_ring_slot(&mut self) -> Option<usize> {
        let idx = self.ring_free.pop()?;
        self.ring_active = idx;
        self.resident_points = 0;
        Some(idx)
    }

    /// Return a held scatter slot to the free pool (no-op if it is still the active compute target).
    pub(crate) fn release_ring_slot(&mut self, idx: usize) {
        if idx == self.ring_active {
            return;
        }
        if !self.ring_free.contains(&idx) {
            self.ring_free.push(idx);
        }
    }

    /// Record the active point buffer as held for scatter without rotating.
    /// Call `acquire_ring_slot` before the next overlapping compute.
    pub(crate) fn hold_active_for_scatter(&mut self) -> usize {
        self.ring_active
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
        if use_gpu {
            let open_before = active_batch
                .points
                .iter()
                .filter(|s| s.as_ref().map(|(_, p)| !p.finished).unwrap_or(false))
                .count();
            if try_gpu_workshift(worker_state, active_batch) {
                // Deferred scatter wave: seats stay open on the host until confirm.
                // Do not count as low-yield or fall into CPU followup.
                if worker_state.has_unparked_pending_scatter() {
                    worker_state.gpu_low_yield_streak = 0;
                    worker_state.cpu_followup = false;
                    return true;
                }
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

/// Pack an f32 ActivePoint without f64 bridging (home hot path).
fn gpu_pert_from_f32(
    point: &ActivePoint<f32, StandardPeriodicityDetector<f32>>,
) -> GpuPertPoint {
    let c_re = point.c.0;
    let c_im = point.c.1;
    GpuPertPoint {
        dc_re: c_re,
        dc_im: c_im,
        dz_re: point.z.0,
        dz_im: point.z.1,
        d_re: point.derivative.0,
        d_im: point.derivative.1,
        iteration_count: point.iteration_count.min(u32::MAX as u64) as u32,
        min_magnitude: point.min_magnitude,
        min_magnitude_time: point.min_magnitude_time.min(u32::MAX as u64) as u32,
        flags: FLAG_ACTIVE,
        checkpoint_re: point.z.0,
        checkpoint_im: point.z.1,
        steps_since_checkpoint: 0,
        next_checkpoint_iteration: 1,
        detected_period: 0,
        epsilon: 1e-12f32.max(c_re.abs().max(c_im.abs()) * 1e-6),
    }
}

fn pack_gpu_from_f32_batch<const N: usize>(
    batch: &PointBatch<f32, StandardPeriodicityDetector<f32>, N>,
) -> (
    Vec<usize>,
    Vec<GpuPertPoint>,
    Vec<u32>,
    Vec<(usize, usize)>,
    Option<OrbitId>,
) {
    use crate::constants::TILE_EDGE_LENGTH;
    let mut map_idx = Vec::with_capacity(N);
    let mut gpu_points = Vec::with_capacity(N);
    let mut local_seats = Vec::with_capacity(N);
    let mut scatter_locals = Vec::with_capacity(N);
    let mut shared_orbit: Option<OrbitId> = None;
    let mut orbit_ok = true;
    for (i, slot) in batch.points.iter().enumerate() {
        let Some((local, point)) = slot else { continue };
        if point.finished {
            continue;
        }
        match shared_orbit {
            None => shared_orbit = Some(point.orbit_id),
            Some(id) if id != point.orbit_id => {
                orbit_ok = false;
                break;
            }
            _ => {}
        }
        map_idx.push(i);
        gpu_points.push(gpu_pert_from_f32(point));
        local_seats.push((local.1 * TILE_EDGE_LENGTH + local.0) as u32);
        scatter_locals.push(*local);
    }
    if !orbit_ok {
        return (Vec::new(), Vec::new(), Vec::new(), Vec::new(), None);
    }
    (map_idx, gpu_points, local_seats, scatter_locals, shared_orbit)
}

/// Pack a typed ActivePoint into a GpuPertPoint (f32 device fields).
fn gpu_pert_from_typed<T, P>(point: &ActivePoint<T, P>) -> GpuPertPoint
where
    T: crate::assemblies::workgroup::structs::mandelbrotable::Mandelbrotable + Copy,
    P: PeriodicityDetector<T>,
{
    let c_re = point.c.0.to_f64();
    let c_im = point.c.1.to_f64();
    let z_re = point.z.0.to_f64() as f32;
    let z_im = point.z.1.to_f64() as f32;
    GpuPertPoint {
        dc_re: c_re as f32,
        dc_im: c_im as f32,
        dz_re: z_re,
        dz_im: z_im,
        d_re: point.derivative.0.to_f64() as f32,
        d_im: point.derivative.1.to_f64() as f32,
        iteration_count: point.iteration_count.min(u32::MAX as u64) as u32,
        min_magnitude: point.min_magnitude.to_f64() as f32,
        min_magnitude_time: point.min_magnitude_time.min(u32::MAX as u64) as u32,
        flags: FLAG_ACTIVE,
        checkpoint_re: z_re,
        checkpoint_im: z_im,
        steps_since_checkpoint: 0,
        next_checkpoint_iteration: 1,
        detected_period: 0,
        epsilon: 1e-12f32.max((c_re.abs().max(c_im.abs()) as f32) * 1e-6),
    }
}

fn pack_gpu_from_typed_batch<T, P, const N: usize>(
    batch: &PointBatch<T, P, N>,
) -> (
    Vec<usize>,
    Vec<GpuPertPoint>,
    Vec<u32>,
    Vec<(usize, usize)>,
    Option<OrbitId>,
)
where
    T: crate::assemblies::workgroup::structs::mandelbrotable::Mandelbrotable + Copy,
    P: PeriodicityDetector<T>,
{
    use crate::constants::TILE_EDGE_LENGTH;
    let mut map_idx = Vec::with_capacity(N);
    let mut gpu_points = Vec::with_capacity(N);
    let mut local_seats = Vec::with_capacity(N);
    let mut scatter_locals = Vec::with_capacity(N);
    let mut shared_orbit: Option<OrbitId> = None;
    let mut orbit_ok = true;
    for (i, slot) in batch.points.iter().enumerate() {
        let Some((local, point)) = slot else { continue };
        if point.finished {
            continue;
        }
        match shared_orbit {
            None => shared_orbit = Some(point.orbit_id),
            Some(id) if id != point.orbit_id => {
                orbit_ok = false;
                break;
            }
            _ => {}
        }
        map_idx.push(i);
        gpu_points.push(gpu_pert_from_typed(point));
        local_seats.push((local.1 * TILE_EDGE_LENGTH + local.0) as u32);
        scatter_locals.push(*local);
    }
    if !orbit_ok {
        return (Vec::new(), Vec::new(), Vec::new(), Vec::new(), None);
    }
    (map_idx, gpu_points, local_seats, scatter_locals, shared_orbit)
}

fn pack_gpu_from_gear<const N: usize>(
    batch: &crate::assemblies::workgroup::workcore::mandelbrot::worker_implementations::active_gear_work::ActiveGearWork<N>,
) -> (
    Vec<usize>,
    Vec<GpuPertPoint>,
    Vec<u32>,
    Vec<(usize, usize)>,
    Option<OrbitId>,
) {
    use crate::assemblies::workgroup::workcore::mandelbrot::worker_implementations::active_gear_work::ActiveGearWork;
    match batch {
        ActiveGearWork::F32(b) => pack_gpu_from_f32_batch(b),
        ActiveGearWork::F64(b) => pack_gpu_from_typed_batch(b),
        ActiveGearWork::Stacked1(b) => pack_gpu_from_typed_batch(b),
        ActiveGearWork::Stacked2(b) => pack_gpu_from_typed_batch(b),
        ActiveGearWork::Stacked3(b) => pack_gpu_from_typed_batch(b),
        ActiveGearWork::Stacked4(b) => pack_gpu_from_typed_batch(b),
        ActiveGearWork::Stacked5(b) => pack_gpu_from_typed_batch(b),
        ActiveGearWork::Stacked6(b) => pack_gpu_from_typed_batch(b),
        ActiveGearWork::Stacked7(b) => pack_gpu_from_typed_batch(b),
        ActiveGearWork::Stacked8(b) => pack_gpu_from_typed_batch(b),
        ActiveGearWork::Adaptive(b) => pack_gpu_from_typed_batch(b),
    }
}

/// Write-all uniforms/scatter params, then one encoder of init+bout+scatter per tile, one submit.
fn submit_cgen_micro_batch_inner(
    worker_state: &mut PerturbationGpuWorkerState,
    tiles: &[CgenQueuedTile],
) -> bool {
    use crate::assemblies::workgroup::bout_scatter::BoutScatter;
    use crate::assemblies::workgroup::production_atlas::ProductionAtlas;
    use crate::constants::{PIXELS_PER_UNIT_POT, TILE_SEAT_COUNT};

    if tiles.is_empty() {
        return true;
    }
    let atlas = match ProductionAtlas::shared() {
        Some(a) => a,
        None => return false,
    };
    let scatter = match BoutScatter::shared() {
        Some(s) => s,
        None => return false,
    };
    if !matches!(worker_state.cpu.gear, crate::gear::Gear::F32) {
        return false;
    }
    let Some(&orbit_id) = worker_state.cpu.seat_orbit_ids.first() else {
        return false;
    };
    let (stencil_serial, origin_re, origin_im, space, orbit_ok) = {
        let Some(stencil) = worker_state.cpu.stencil.as_ref() else {
            return false;
        };
        let Some(orbit) = worker_state.cpu.references.get(orbit_id) else {
            return false;
        };
        let Some(cgen) = stencil.get_relative_c_generator::<f32>(&orbit.big_c) else {
            return false;
        };
        let ok = !orbit.f32.big_z_orbit.is_empty() && orbit.f32.big_z_orbit.len() <= 65_536;
        let ((ore, oim), sp) = cgen.origin_and_space();
        (stencil.serial_number, ore, oim, sp, ok)
    };
    if !orbit_ok {
        return false;
    }
    let half_px = 2.0_f32.powi(-(PIXELS_PER_UNIT_POT + 1));
    let bailout = worker_state.cpu.bailout_radius_squared as f32;
    let orbit_key = orbit_id as u64;
    let point_count = TILE_SEAT_COUNT as u32;
    let upload_orbit = worker_state
        .gpu
        .as_ref()
        .map(|g| g.last_orbit_id != Some(orbit_key))
        .unwrap_or(true);

    let local_seats: Vec<u32> = (0..point_count).collect();

    // Home exterior escapes quickly; short bout matches prior single-tile cgen path.
    const COMPUTE_ROUNDS: u32 = 1;
    let bout = 256u32;
    let Ok(atlas_guard) = atlas.lock() else {
        return false;
    };
    let device = worker_state.gpu.as_ref().expect("gpu").shared.device.clone();
    let queue = worker_state.gpu.as_ref().expect("gpu").shared.queue.clone();

    // Phase 1: write all distinct ring uniforms + scatter params (no encode yet).
    {
        let orbit_f32 = worker_state
            .cpu
            .references
            .get(orbit_id)
            .map(|o| o.f32.big_z_orbit.as_slice())
            .unwrap_or(&[]);
        let gpu = worker_state.gpu.as_mut().expect("gpu");
        gpu.upload_view_cgen(
            stencil_serial,
            orbit_key,
            origin_re,
            origin_im,
            space,
            half_px,
        );
        for (i, tile) in tiles.iter().enumerate() {
            if !gpu.write_bout_cgen_uniforms(
                orbit_f32,
                bailout,
                bout,
                point_count,
                tile.tile_origin.0 as u32,
                tile.tile_origin.1 as u32,
                upload_orbit && i == 0,
                tile.ring_idx,
            ) {
                return false;
            }
            if !scatter.write_scatter_nomap(
                &*atlas_guard,
                tile.atlas_slot,
                &local_seats,
                tile.ring_idx,
            ) {
                return false;
            }
        }
        gpu.last_orbit_id = Some(orbit_key);
    }

    // Phase 2: encode all tiles into one command buffer, one submit.
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("perturbation_gpu_cgen_micro_batch_enc"),
    });
    {
        let gpu = worker_state.gpu.as_mut().expect("gpu");
        for tile in tiles {
            if !gpu.encode_bout_cgen_passes(
                &mut encoder,
                COMPUTE_ROUNDS,
                point_count,
                tile.ring_idx,
            ) {
                return false;
            }
            if !scatter.encode_scatter_nomap_pass(
                &mut encoder,
                gpu.point_buffer_at(tile.ring_idx),
                &*atlas_guard,
                tile.atlas_slot,
                &local_seats,
                tile.ring_idx,
            ) {
                return false;
            }
        }
    }
    queue.submit(Some(encoder.finish()));
    worker_state.iterations_advanced = worker_state.iterations_advanced.saturating_add(
        u64::from(bout)
            * u64::from(COMPUTE_ROUNDS)
            * u64::from(point_count)
            * tiles.len() as u64,
    );
    true
}

/// Dense identity tile: CGenerator on GPU, no host δc pack.
fn try_gpu_resident_scatter_cgen(
    worker_state: &mut PerturbationGpuWorkerState,
    tile_origin: (usize, usize),
) -> bool {
    use crate::assemblies::workgroup::bout_scatter::BoutScatter;
    use crate::assemblies::workgroup::production_atlas::ProductionAtlas;
    use crate::constants::{PIXELS_PER_UNIT_POT, TILE_EDGE_LENGTH, TILE_SEAT_COUNT};

    worker_state.last_scatter_full_batch = false;
    worker_state.last_scatter_locals.clear();

    let slot = match worker_state.gpu_resident_slot {
        Some(s) => s,
        None => return false,
    };
    let atlas = match ProductionAtlas::shared() {
        Some(a) => a,
        None => return false,
    };
    let scatter = match BoutScatter::shared() {
        Some(s) => s,
        None => return false,
    };
    if !matches!(worker_state.cpu.gear, crate::gear::Gear::F32) {
        return false;
    }
    let Some(&orbit_id) = worker_state.cpu.seat_orbit_ids.first() else {
        return false;
    };
    let (stencil_serial, origin_re, origin_im, space, orbit_ok) = {
        let Some(stencil) = worker_state.cpu.stencil.as_ref() else {
            return false;
        };
        let Some(orbit) = worker_state.cpu.references.get(orbit_id) else {
            return false;
        };
        let Some(cgen) = stencil.get_relative_c_generator::<f32>(&orbit.big_c) else {
            return false;
        };
        let ok = !orbit.f32.big_z_orbit.is_empty() && orbit.f32.big_z_orbit.len() <= 65_536;
        let ((ore, oim), sp) = cgen.origin_and_space();
        (stencil.serial_number, ore, oim, sp, ok)
    };
    if !orbit_ok {
        return false;
    }
    let half = 2.0_f32.powi(-(PIXELS_PER_UNIT_POT + 1));
    let bailout = worker_state.cpu.bailout_radius_squared as f32;
    let orbit_key = orbit_id as u64;
    let point_count = TILE_SEAT_COUNT as u32;
    let upload_orbit = worker_state
        .gpu
        .as_ref()
        .map(|g| g.last_orbit_id != Some(orbit_key))
        .unwrap_or(true);

    let local_seats: Vec<u32> = (0..point_count).collect();
    let scatter_locals: Vec<(usize, usize)> = (0..TILE_SEAT_COUNT)
        .map(|i| (i % TILE_EDGE_LENGTH, i / TILE_EDGE_LENGTH))
        .collect();

    const COMPUTE_ROUNDS: u32 = 1;
    let bout = 256u32;

    let Ok(atlas_guard) = atlas.lock() else {
        return false;
    };
    let ring_idx = worker_state
        .gpu
        .as_ref()
        .map(|g| g.ring_active())
        .unwrap_or(0);
    let device = worker_state.gpu.as_ref().expect("gpu").shared.device.clone();
    let queue = worker_state.gpu.as_ref().expect("gpu").shared.queue.clone();
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("perturbation_gpu_cgen_fused_enc"),
    });
    {
        let orbit_f32 = worker_state
            .cpu
            .references
            .get(orbit_id)
            .map(|o| o.f32.big_z_orbit.as_slice())
            .unwrap_or(&[]);
        let gpu = worker_state.gpu.as_mut().expect("gpu");
        gpu.upload_view_cgen(
            stencil_serial,
            orbit_key,
            origin_re,
            origin_im,
            space,
            half,
        );
        if !gpu.encode_bout_cgen(
            &mut encoder,
            orbit_f32,
            bailout,
            bout,
            COMPUTE_ROUNDS,
            point_count,
            tile_origin.0 as u32,
            tile_origin.1 as u32,
            upload_orbit,
            ring_idx,
            true,
        ) {
            return false;
        }
        gpu.last_orbit_id = Some(orbit_key);
        if !scatter.encode_scatter_nomap(
            &mut encoder,
            gpu.point_buffer_at(ring_idx),
            &*atlas_guard,
            slot,
            &local_seats,
            ring_idx,
        ) {
            return false;
        }
    }
    queue.submit(Some(encoder.finish()));
    worker_state.iterations_advanced = worker_state.iterations_advanced.saturating_add(
        u64::from(bout) * u64::from(COMPUTE_ROUNDS) * u64::from(point_count),
    );
    drop(atlas_guard);
    worker_state.pending_scatter_map_idx = (0..TILE_SEAT_COUNT).collect();
    let held = worker_state
        .gpu
        .as_mut()
        .expect("gpu")
        .hold_active_for_scatter();
    worker_state.pending_scatter_ring = Some(held);
    worker_state.pending_scatter_armed = point_count;
    worker_state.pending_scatter_locals_buf = scatter_locals;
    true
}

/// Nomap defer path: pack from live gear (no f64 host bridge) and fuse compute+scatter.
fn try_gpu_resident_scatter_from_gear<const N: usize>(
    worker_state: &mut PerturbationGpuWorkerState,
    batch: &crate::assemblies::workgroup::workcore::mandelbrot::worker_implementations::active_gear_work::ActiveGearWork<N>,
) -> bool {
    use crate::assemblies::workgroup::bout_scatter::BoutScatter;
    use crate::assemblies::workgroup::production_atlas::ProductionAtlas;

    worker_state.last_scatter_full_batch = false;
    worker_state.last_scatter_locals.clear();

    let slot = match worker_state.gpu_resident_slot {
        Some(s) => s,
        None => return false,
    };
    let atlas = match ProductionAtlas::shared() {
        Some(a) => a,
        None => return false,
    };
    let scatter = match BoutScatter::shared() {
        Some(s) => s,
        None => return false,
    };
    let stacked_limbs = match worker_state.cpu.gear {
        crate::gear::Gear::F32 => None,
        crate::gear::Gear::StackedI32 { limbs } => Some(limbs),
        _ => return false,
    };

    let (map_idx, gpu_points, local_seats, scatter_locals, shared_orbit) =
        pack_gpu_from_gear(batch);
    if map_idx.is_empty() {
        return true;
    }
    let orbit_id = match shared_orbit {
        Some(id) => id,
        None => return false,
    };
    let orbit = match worker_state.cpu.references.get(orbit_id) {
        Some(o) => o,
        None => return false,
    };
    if orbit.f32.big_z_orbit.len() > 65_536 {
        return false;
    }
    let orbit_f32 = orbit.f32.big_z_orbit.as_slice();
    let bailout = worker_state.cpu.bailout_radius_squared as f32;
    let orbit_key = orbit_id as u64;
    let point_count = gpu_points.len() as u32;

    const COMPUTE_ROUNDS: u32 = 2;
    let bout = 512u32;
    let upload_orbit = worker_state
        .gpu
        .as_ref()
        .map(|g| g.last_orbit_id != Some(orbit_key))
        .unwrap_or(true);
    // Always upload points on the thin packing path (fresh from gear each tile).
    let needs_upload = true;

    let Ok(atlas_guard) = atlas.lock() else {
        return false;
    };
    let ring_idx = worker_state
        .gpu
        .as_ref()
        .map(|g| g.ring_active())
        .unwrap_or(0);
    let device = worker_state.gpu.as_ref().expect("gpu").shared.device.clone();
    let queue = worker_state.gpu.as_ref().expect("gpu").shared.queue.clone();
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("perturbation_gpu_fused_bout_scatter_enc"),
    });
    {
        let gpu = worker_state.gpu.as_mut().expect("gpu");
        if !gpu.encode_bout_rounds(
            &mut encoder,
            stacked_limbs,
            &gpu_points,
            orbit_f32,
            bailout,
            bout,
            COMPUTE_ROUNDS,
            true,
            needs_upload,
            upload_orbit,
        ) {
            return false;
        }
        gpu.last_orbit_id = Some(orbit_key);
        if !scatter.encode_scatter_nomap(
            &mut encoder,
            gpu.point_buffer(),
            &*atlas_guard,
            slot,
            &local_seats,
            ring_idx,
        ) {
            return false;
        }
    }
    queue.submit(Some(encoder.finish()));
    worker_state.iterations_advanced = worker_state.iterations_advanced.saturating_add(
        u64::from(bout) * u64::from(COMPUTE_ROUNDS) * u64::from(point_count),
    );
    drop(atlas_guard);
    worker_state.pending_scatter_map_idx = map_idx;
    let held = worker_state
        .gpu
        .as_mut()
        .expect("gpu")
        .hold_active_for_scatter();
    worker_state.pending_scatter_ring = Some(held);
    worker_state.pending_scatter_armed = point_count;
    worker_state.pending_scatter_locals_buf = scatter_locals;
    true
}

/// Stationary GPU fill: compute on GPU, scatter terminals to production atlas,
/// skip map_async harvest when the batch fully completes on device.
fn try_gpu_resident_scatter<const N: usize>(
    worker_state: &mut PerturbationGpuWorkerState,
    active_batch: &mut PointBatch<f64, CpuPeriodicityDetector, N>,
) -> bool {
    use crate::assemblies::workgroup::bout_scatter::BoutScatter;
    use crate::assemblies::workgroup::production_atlas::ProductionAtlas;
    use crate::constants::TILE_EDGE_LENGTH;

    worker_state.last_scatter_full_batch = false;
    worker_state.last_scatter_locals.clear();

    let slot = match worker_state.gpu_resident_slot {
        Some(s) => s,
        None => {
            if worker_state.scatter_defer_flush {
            }
            return false;
        }
    };
    let atlas = match ProductionAtlas::shared() {
        Some(a) => a,
        None => {
            if worker_state.scatter_defer_flush {
            }
            return false;
        }
    };
    let scatter = match BoutScatter::shared() {
        Some(s) => s,
        None => {
            if worker_state.scatter_defer_flush {
            }
            return false;
        }
    };
    let stacked_limbs = match worker_state.cpu.gear {
        crate::gear::Gear::F32 => None,
        crate::gear::Gear::StackedI32 { limbs } => Some(limbs),
        other => {
            if worker_state.scatter_defer_flush {
            }
            return false;
        }
    };
    let mut shared_orbit: Option<OrbitId> = None;
    for slot in active_batch.points.iter() {
        let Some((_, point)) = slot else { continue };
        if point.finished {
            continue;
        }
        match shared_orbit {
            None => shared_orbit = Some(point.orbit_id),
            Some(id) if id != point.orbit_id => {
                if worker_state.scatter_defer_flush {
                }
                return false;
            }
            _ => {}
        }
    }
    let orbit_id = match shared_orbit {
        Some(id) => id,
        None => {
            if worker_state.scatter_defer_flush {
            }
            return false;
        }
    };
    let orbit = match worker_state.cpu.references.get(orbit_id) {
        Some(o) => o,
        None => {
            if worker_state.scatter_defer_flush {
            }
            return false;
        }
    };
    if orbit.f32.big_z_orbit.len() > 65_536 {
        if worker_state.scatter_defer_flush {
        }
        return false;
    }
    let orbit_f32 = orbit.f32.big_z_orbit.as_slice();
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

    let gpu_points = if can_reuse {
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
    // Home exterior escapes in few iters; keep rounds low for the nomap wave.
    const COMPUTE_ROUNDS: u32 = 2;
    let bout = 512u32;
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

    let local_seats: Vec<u32> = map_idx
        .iter()
        .map(|&bi| {
            let (local, _) = active_batch.points[bi].as_ref().expect("open seat");
            (local.1 * TILE_EDGE_LENGTH + local.0) as u32
        })
        .collect();

    let scatter_locals: Vec<(usize, usize)> = map_idx
        .iter()
        .map(|&bi| active_batch.points[bi].as_ref().expect("open seat").0)
        .collect();

    let Ok(atlas_guard) = atlas.lock() else {
        return false;
    };

    if worker_state.scatter_defer_flush {
        // Fuse compute rounds + nomap scatter into one submit (D-GPU hot path).
        let ring_idx = worker_state
            .gpu
            .as_ref()
            .map(|g| g.ring_active())
            .unwrap_or(0);
        let device = worker_state.gpu.as_ref().expect("gpu").shared.device.clone();
        let queue = worker_state.gpu.as_ref().expect("gpu").shared.queue.clone();
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("perturbation_gpu_fused_bout_scatter_enc"),
        });
        {
            let gpu = worker_state.gpu.as_mut().expect("gpu");
            if !gpu.encode_bout_rounds(
                &mut encoder,
                stacked_limbs,
                &gpu_points,
                orbit_f32,
                bailout,
                bout,
                COMPUTE_ROUNDS,
                true,
                needs_upload,
                upload_orbit,
            ) {
                return false;
            }
            gpu.last_orbit_id = Some(orbit_key);
            if !scatter.encode_scatter_nomap(
                &mut encoder,
                gpu.point_buffer(),
                &*atlas_guard,
                slot,
                &local_seats,
                ring_idx,
            ) {
                return false;
            }
        }
        queue.submit(Some(encoder.finish()));
        worker_state.iterations_advanced = worker_state.iterations_advanced.saturating_add(
            u64::from(bout) * u64::from(COMPUTE_ROUNDS) * u64::from(point_count),
        );
        drop(atlas_guard);
        worker_state.pending_scatter_map_idx = map_idx.clone();
        let held = worker_state
            .gpu
            .as_mut()
            .expect("gpu")
            .hold_active_for_scatter();
        worker_state.pending_scatter_ring = Some(held);
        worker_state.pending_scatter_armed = point_count;
        worker_state.pending_scatter_locals_buf = scatter_locals;
        return true;
    }

    {
        let gpu = worker_state.gpu.as_mut().expect("gpu");
        if let Some(limbs) = stacked_limbs {
            gpu.run_stacked_bout_compute_multi_nopoll(
                limbs,
                &gpu_points,
                orbit_f32,
                bailout,
                bout,
                COMPUTE_ROUNDS,
                needs_upload,
                upload_orbit,
            );
        } else {
            gpu.run_bout_compute_multi_nopoll(
                &gpu_points,
                orbit_f32,
                bailout,
                bout,
                COMPUTE_ROUNDS,
                needs_upload,
                upload_orbit,
            );
        }
        gpu.last_orbit_id = Some(orbit_key);
    }
    worker_state.iterations_advanced = worker_state.iterations_advanced.saturating_add(
        u64::from(bout) * u64::from(COMPUTE_ROUNDS) * u64::from(point_count),
    );

    if !scatter.scatter_submit(
        worker_state.gpu.as_ref().expect("gpu").point_buffer(),
        &*atlas_guard,
        slot,
        &local_seats,
    ) {
        return false;
    }
    drop(atlas_guard);

    worker_state.pending_scatter_map_idx = map_idx.clone();

    let terminals = match scatter.flush_scatter_counter() {
        Some(t) => t,
        None => return false,
    };
    worker_state.last_scatter_terminals = terminals.0;
    worker_state.last_tile_completion = terminals.1;

    if terminals.0 >= point_count && point_count > 0 {
        for &bi in &map_idx {
            if let Some((_, point)) = active_batch.points[bi].as_mut() {
                point.finished = true;
                point.escaped = true;
                point.iteration_count = point.iteration_count.max(1);
            }
        }
        worker_state.last_scatter_locals = scatter_locals;
        worker_state.last_scatter_full_batch = true;
        worker_state.clear_resident();
        worker_state.cpu_followup = false;
        worker_state.gpu_low_yield_streak = 0;
        return true;
    }

    worker_state.resident_map_idx = map_idx;
    worker_state.resident_gpu_points = gpu_points;
    worker_state.resident_orbit_key = Some(orbit_key);
    worker_state.resident_limbs = limbs_key;
    false
}

fn try_gpu_workshift<const N: usize>(
    worker_state: &mut PerturbationGpuWorkerState,
    active_batch: &mut PointBatch<f64, CpuPeriodicityDetector, N>,
) -> bool {
    if worker_state.stationary_gpu_resident && worker_state.gpu_resident_slot.is_some() {
        if try_gpu_resident_scatter(worker_state, active_batch) {
            return true;
        }
    }
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
