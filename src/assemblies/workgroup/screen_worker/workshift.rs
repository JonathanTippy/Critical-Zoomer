
use rand::prelude::SliceRandom;

use std::sync::Arc;
use std::time::Instant;
use std::collections::*;
use std::cmp::*;
use crate::assemblies::workgroup::c_generator::{admit_generator, CGenerator, GeneratorAdmission, Mandelbrotable};
use crate::assemblies::workgroup::reference_worker::PublishedReference;
use crate::delta_gear::{ComputeGear, view_gear_from_generators};
use crate::floatexp::{ComplexFloatExp, FloatExp};
use crate::constants::PIXELS_PER_UNIT_POT;
use crate::reference::{bits_for_zoom, ReferenceOrbit};
use crate::utils::*;
pub const NUMBER_OF_LOOP_CHECK_POINTS: usize = 5;

pub const MAX_PIXELS:usize = 1920*1080*4;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Step {Scredge, In, Out, Edge, Attention}


pub trait Floaty: Sub<Output=Self> + Add<Output=Self> + Mul<Output=Self> + Into<f64> + PartialOrd + Finite + Gt + Abs + From<f32> + Into<f64> + Copy {}

#[derive(Clone, Debug)]
pub struct Stec<T: Copy> {
    pub stuff: Vec<T>
    , pub len: usize
}

impl<T: Copy> Stec<T> {
    pub fn with_capacity(cap: usize, fill: T) -> Self {
        let mut stuff = Vec::with_capacity(cap);
        stuff.resize(cap, fill);
        Self { stuff, len: 0 }
    }

    pub fn try_push(&mut self, thing:T) -> bool {
        if self.len < self.stuff.len() {
            self.stuff[self.len] = thing;
            self.len+=1;
            true
        } else {
            false
        }
    }
    pub fn try_pop(&mut self) -> Option<T> {
        if self.len > 0 {
            self.len-=1;
            Some(self.stuff[self.len])
        } else {
            None
        }
    }
}


/// A completion staged for the buffer. `Provisional` answers (period-0 scredge
/// guesses) publish data but must never mark a seat delivered; only a `Final`
/// answer may. The type makes "guess blocks truth" unrepresentable.
// r[impl cz.craft.provisional-not-delivered+1]
#[derive(Clone, Copy, Debug)]
pub enum Delivery<T> {
    Provisional(T),
    Final(T),
}

/// Result of attempting to stage a delivery. `#[must_use]` so backpressure
/// (buffer full) cannot be silently dropped.
// r[impl cz.craft.undeliver-on-full+1]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[must_use = "BufferFull must be handled: the seat must stay undelivered and the shift break"]
pub enum PushOutcome {
    Published,
    BufferFull,
}

impl<T: Mandelbrotable> WorkContext<T> {
    /// Atomically stage a delivery into the completion buffer and update the
    /// seat's `delivered` flag. The two can never disagree:
    /// - `Final` + room -> delivered = true, Published
    /// - `Final` + full -> delivered = false, BufferFull (backpressure re-queue)
    /// - `Provisional` + room -> delivered unchanged, Published
    /// - `Provisional` + full -> delivered unchanged, BufferFull
    // r[impl cz.craft.provisional-not-delivered+1]
    // r[impl cz.craft.undeliver-on-full+1]
    pub fn push_delivery(&mut self, delivery: Delivery<CompletedPoint<T>>, index: usize) -> PushOutcome {
        let (point, is_final) = match delivery {
            Delivery::Provisional(p) => (p, false),
            Delivery::Final(p) => (p, true),
        };
        if self.completed_points.try_push((point, index)) {
            if is_final {
                self.points[index].delivered = true;
            }
            PushOutcome::Published
        } else {
            if is_final {
                self.points[index].delivered = false;
            }
            PushOutcome::BufferFull
        }
    }
}


use std::collections::*;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Motion {
    /// Zoom level changed: lead with attention (direct navigation).
    Zoomed,
    /// Position changed at constant zoom: lead with scredge (smearing border).
    Panned,
    /// Fresh shell or no motion: lead with attention.
    Neither,
}

#[derive(Clone, Debug)]
pub struct WorkContext<T: Mandelbrotable> {
    pub points: Vec<Point<T>>
    , pub completed_points: Stec<(CompletedPoint<T>, usize)>
    , pub last_update: usize
    , pub index: usize
    , pub random_index: usize
    , pub time_created: Instant
    , pub time_workshift_started: Instant
    , pub percent_completed:f64
    , pub random_map: Vec<usize>
    , pub workshifts: u32
    , pub total_iterations: u32
    , pub total_iterations_today: u32
    , pub total_bouts_today: u32
    , pub total_points_today: u32
    , pub spent_tokens_today: u32
    , pub res: (u32, u32)
    , pub scredge_poses: VecDeque<(i32, i32)>
    , pub edge_queue: VecDeque<((i32, i32), u32)>
    , pub out_queue: VecDeque<((i32, i32), u32)>
    , pub in_queue: VecDeque<((i32, i32), u32)>
    , pub motion: Motion
    , pub attention: Option<(i32, i32)>
    // Spiral center: cursor when present, else screen center.
    , pub attention_anchor: (i32, i32)
    // Flattened square-ring index; advances only when a held seat completes.
    , pub attention_index: u64
    // Seat the attention phase is tenaciously working; cleared on completion.
    , pub attention_current: Option<(i32, i32)>
    , pub c_generator: CGenerator<T>
    , pub pitch_epsilon: T
    // Newest published reference for the live shell. Shared so the clone-happy
    // context type does not deep-copy rug Float state on every shell clone.
    , pub latest_reference: Option<Arc<PublishedReference>>
    // IntExp anchor for relative seat samples (view center in compute space).
    // r[impl cz.depth.floatexp-host-coords+1]
    , pub coord_anchor: (IntExp, IntExp)
    // Default delta gear for this view (seats may promote individually).
    // r[impl cz.depth.compute-gear+1]
    , pub view_gear: ComputeGear
    // Rolling HUD aggregate across active seats.
    , pub active_gear: ComputeGear
    // True when `c_generator` emits relative-to-`coord_anchor` samples.
    , pub coords_are_relative: bool
    // Rolling ~1s HUD completion window for perturbation display gating.
    , pub hud_points_window: u32
    , pub hud_window_started: Instant
    // True when seats may bind to `latest_reference`; trial-only, never sticky.
    , pub reference_floor_active: bool
    , pub pert_trial_shifts_left: u8
    , pub pert_trial_cooldown: u32
    // Bumped when `c_generator` is rebuilt for a new reference generation.
    , pub generator_generation: u64
    // Set when the last workshift ran the Naive GPU wave path (HUD).
    , pub last_used_naive_gpu: bool
    // Debug override: force an entire compute kernel. `None` = automatic
    // PPS / depth policy. Host stack type remains auto from admission.
    , pub manual_gear: Option<crate::assemblies::structs::KernelMode>
    // PPS race winner for this view (`None` = still probing / not started).
    // r[impl cz.perf.pps-selected-kernel+1]
    , pub pps_locked_kernel: Option<crate::assemblies::structs::KernelMode>
    // When the current lock was taken — re-probe after `PPS_REEVAL_INTERVAL`.
    , pub pps_lock_started: Instant
    // Remaining candidates to sample (front = current probe target).
    , pub pps_probe_queue: Vec<crate::assemblies::structs::KernelMode>
    , pub pps_probe_shifts_left: u8
    , pub pps_probe_points: u32
    , pub pps_probe_started: Instant
    , pub pps_probe_samples: Vec<(crate::assemblies::structs::KernelMode, f64)>
    // Greedy kept references (off-screen still useful). `latest_reference` is
    // the preferred bind for the view generator.
    , pub reference_library: Vec<Arc<PublishedReference>>
}

/// Brief perturbation probe when direct is genuinely stuck (>2s to clear remaining).
const PERT_TRIAL_SHIFTS: u8 = 3;
const PERT_TRIAL_COOLDOWN_SHIFTS: u32 = 40;
/// Seconds of remaining work at current PPS before a trial is warranted.
const PERT_PROMOTE_REMAINING_SECS: f64 = 2.0;

impl<T: Mandelbrotable> WorkContext<T> {
    fn end_pert_trial(&mut self) {
        self.reference_floor_active = false;
        self.pert_trial_shifts_left = 0;
        self.pert_trial_cooldown = PERT_TRIAL_COOLDOWN_SHIFTS;
    }

    fn struggling_to_clear(&self, remaining: u64, pps: f64) -> bool {
        remaining > 0 && pps >= 1.0 && remaining as f64 / pps > PERT_PROMOTE_REMAINING_SECS
    }

    /// Called once per workshift after the kernel runs.
    pub fn tick_pert_trial(&mut self) -> Option<&'static str> {
        if !self.reference_floor_active {
            return None;
        }
        if self.pert_trial_shifts_left > 0 {
            self.pert_trial_shifts_left -= 1;
        }
        if self.pert_trial_shifts_left == 0 {
            self.end_pert_trial();
            return Some("trial_expired");
        }
        None
    }
    /// Reference published for seat binding this shift (zero-orbit when inactive).
    pub fn floor_reference(&self) -> Option<&crate::assemblies::workgroup::reference_worker::PublishedReference> {
        if self.reference_floor_active {
            self.latest_reference.as_deref()
        } else {
            None
        }
    }

    /// Read-only policy label for HUD telemetry (no side effects).
    pub fn floor_policy_label(&self) -> &'static str {
        if self.reference_floor_active {
            return "trial_active";
        }
        if self.pert_trial_cooldown > 0 {
            return "cooldown";
        }
        if self.latest_reference.as_ref().is_none_or(|r| r.orbit.escaped) {
            return if self.latest_reference.is_some() {
                "ref_escaped"
            } else {
                "no_ref"
            };
        }
        // Only scan seats when a usable reference exists (trial decisions).
        let remaining = self.points.iter().filter(|p| !p.delivered).count() as u64;
        if remaining == 0 {
            return "complete";
        }
        let pps = self.hud_pps_estimate();
        let min_samples = (self.screen_point_count() as u32 / 200).max(200);
        if self.hud_points_window < min_samples {
            return "warming_up";
        }
        if pps < 1.0 {
            return "no_pps";
        }
        if self.struggling_to_clear(remaining, pps) {
            return "would_trial";
        }
        "direct_fast_enough"
    }

    /// Brief perturbation trial when direct fill would take >2s at current PPS.
    pub fn update_reference_floor_policy(&mut self) -> &'static str {
        if self.reference_floor_active {
            return "trial_active";
        }

        if self.pert_trial_cooldown > 0 {
            return "cooldown";
        }

        let Some(ref published) = self.latest_reference else {
            return "no_ref";
        };
        if published.orbit.escaped {
            return "ref_escaped";
        }
        // Seat scan only when a live reference could justify a trial.
        let remaining = self.points.iter().filter(|p| !p.delivered).count() as u64;
        if remaining == 0 {
            return "complete";
        }
        let pps = self.hud_pps_estimate();
        let min_samples = (self.screen_point_count() as u32 / 200).max(200);
        if self.hud_points_window < min_samples {
            return "warming_up";
        }
        if pps < 1.0 {
            return "no_pps";
        }
        if self.struggling_to_clear(remaining, pps) {
            self.reference_floor_active = true;
            self.pert_trial_shifts_left = PERT_TRIAL_SHIFTS;
            return "promote_trial";
        }
        "direct_fast_enough"
    }
    /// Record completed seats for HUD points-per-second estimate.
    pub fn record_hud_completion_batch(&mut self, n: u32) {
        const WINDOW: std::time::Duration = std::time::Duration::from_secs(1);
        if self.hud_window_started.elapsed() >= WINDOW {
            self.hud_points_window = 0;
            self.hud_window_started = Instant::now();
        }
        self.hud_points_window += n;
    }

    pub fn hud_pps_estimate(&self) -> f64 {
        let secs = self.hud_window_started.elapsed().as_secs_f64().max(0.05);
        self.hud_points_window as f64 / secs
    }

    pub fn screen_point_count(&self) -> u64 {
        self.res.0 as u64 * self.res.1 as u64
    }

    /// Hard bump at f64 precision wall (relative generator) or soft trial floor.
    // r[impl cz.perf.pps-selected-kernel+1]
    pub fn perturbation_kernel_required(&self) -> bool {
        self.coords_are_relative || self.reference_floor_active
    }

    /// Bind published reference orbit (trial or hard-bump relative shell).
    /// Relative shells keep an escaped view-center orbit: its pre-escape iterates
    /// give seats precision via generator delta_c; soft-continue after the tip.
    pub fn perturbation_reference_active(&self) -> bool {
        self.reference_floor_active
            || (self.coords_are_relative && self.latest_reference.is_some())
    }

    /// Kernel the next workshift should run (manual → honesty → soft trial → PPS).
    // r[impl cz.perf.pps-selected-kernel+1]
    pub fn dispatch_kernel(
        &self,
        _gpu_available: bool,
    ) -> crate::assemblies::structs::KernelMode {
        use crate::assemblies::structs::KernelMode;
        if let Some(m) = self.manual_gear {
            return m;
        }
        if self.coords_are_relative {
            return KernelMode::Pert;
        }
        // Soft trial / PPS-locked Pert with an active floor.
        if self.reference_floor_active {
            return KernelMode::Pert;
        }
        if let Some(m) = self.pps_locked_kernel {
            return m;
        }
        if let Some(&m) = self.pps_probe_queue.first() {
            return m;
        }
        KernelMode::Naive
    }

    /// Start or continue the PPS race; lock when every legal candidate is sampled.
    /// Each candidate runs one workshift (`PPS_PROBE_SHIFTS_PER_CANDIDATE`); the race
    /// re-opens every `PPS_REEVAL_INTERVAL` so slowing gears (Naive GPU) can lose.
    // r[impl cz.perf.pps-selected-kernel+1]
    pub fn ensure_pps_probe(&mut self, gpu_available: bool) {
        use crate::gearbox::{legal_kernels, PPS_PROBE_SHIFTS_PER_CANDIDATE, PPS_REEVAL_INTERVAL};
        if self.manual_gear.is_some() {
            return;
        }
        if self.coords_are_relative {
            self.pps_locked_kernel = Some(crate::assemblies::structs::KernelMode::Pert);
            self.pps_probe_queue.clear();
            return;
        }
        if let Some(_) = self.pps_locked_kernel {
            if self.pps_lock_started.elapsed() >= PPS_REEVAL_INTERVAL {
                self.pps_locked_kernel = None;
                self.pps_probe_queue.clear();
                self.pps_probe_samples.clear();
                self.pps_probe_shifts_left = 0;
                // Drop soft trial floor so the race can re-pick Naive/GPU.
                self.reference_floor_active = false;
            } else {
                return;
            }
        }
        if self.pps_probe_queue.is_empty() && self.pps_probe_samples.is_empty() {
            self.pps_probe_queue = legal_kernels(false, gpu_available);
            self.pps_probe_shifts_left = PPS_PROBE_SHIFTS_PER_CANDIDATE;
            self.pps_probe_points = 0;
            self.pps_probe_started = Instant::now();
            self.pps_probe_samples.clear();
        }
    }

    /// After a probe shift: accumulate completions and advance / lock.
    // r[impl cz.perf.pps-selected-kernel+1]
    pub fn finish_pps_probe_shift(&mut self, points_completed: u32) {
        use crate::gearbox::{best_pps_kernel, PPS_PROBE_SHIFTS_PER_CANDIDATE};
        if self.manual_gear.is_some() || self.pps_locked_kernel.is_some() {
            return;
        }
        if self.pps_probe_queue.is_empty() {
            return;
        }
        self.pps_probe_points = self.pps_probe_points.saturating_add(points_completed);
        if self.pps_probe_shifts_left > 0 {
            self.pps_probe_shifts_left -= 1;
        }
        if self.pps_probe_shifts_left > 0 {
            return;
        }
        let mode = self.pps_probe_queue.remove(0);
        let secs = self.pps_probe_started.elapsed().as_secs_f64().max(1e-4);
        let pps = self.pps_probe_points as f64 / secs;
        self.pps_probe_samples.push((mode, pps));
        if self.pps_probe_queue.is_empty() {
            self.pps_locked_kernel = best_pps_kernel(&self.pps_probe_samples);
            self.pps_lock_started = Instant::now();
            // Bind a published ref when Perturbation wins the race.
            if self.pps_locked_kernel
                == Some(crate::assemblies::structs::KernelMode::Pert)
                && self.latest_reference.as_ref().is_some_and(|r| !r.orbit.escaped)
            {
                self.reference_floor_active = true;
            }
            return;
        }
        self.pps_probe_shifts_left = PPS_PROBE_SHIFTS_PER_CANDIDATE;
        self.pps_probe_points = 0;
        self.pps_probe_started = Instant::now();
    }

    /// Remember a published reference for greedy reuse (including off-screen).
    pub fn remember_reference(&mut self, published: Arc<PublishedReference>) {
        if !self
            .reference_library
            .iter()
            .any(|r| r.generation == published.generation)
        {
            self.reference_library.push(published.clone());
        }
        self.latest_reference = Some(published);
    }

    /// Prefer the library member with the smallest |δc| to this absolute seat c.
    /// Falls back to `latest_reference`. Glitched seats use zero-orbit via
    /// `direct_only` and do not call this for binding.
    pub fn best_reference_for_c(
        &self,
        seat_c: &crate::floatexp::ComplexFloatExp,
    ) -> Option<Arc<PublishedReference>> {
        use crate::floatexp::{ComplexFloatExp, FloatExp};
        let mut best: Option<(Arc<PublishedReference>, FloatExp)> = None;
        for r in &self.reference_library {
            if r.orbit.escaped && r.orbit.period.is_none() {
                // Escaped refs still useful for pre-escape iterates; keep.
            }
            let rc = ComplexFloatExp::new(
                FloatExp::from(r.c.0.clone()),
                FloatExp::from(r.c.1.clone()),
            );
            let d = seat_c.clone() - rc;
            let mag = d.re.abs() + d.im.abs();
            match best {
                None => best = Some((r.clone(), mag)),
                Some((_, ref best_mag)) if mag < *best_mag => {
                    best = Some((r.clone(), mag));
                }
                _ => {}
            }
        }
        best.map(|(r, _)| r).or_else(|| self.latest_reference.clone())
    }
}


#[derive(Clone, Copy, Debug)]
pub enum CompletedPoint<T> {
    Repeats{
        period: u32,
        smallness: T,
        small_time: u32
    }
    , Escapes{
        escape_time: u32
        , escape_location: (T, T)
        , escape_derivative: (T, T)
        , start_location: (T, T)
        , smallness: T
        , small_time: u32
    }
    , Dummy{}
}


/// Per-seat perturbation state, resumable across bounded bouts.
///
/// Deltas are concretely `ComplexFloatExp` (floatexp). They are the kernel's
/// internal representation; `Point<T>` stays generic over the view math only.
#[derive(Clone, Debug)]
pub struct DeltaState {
    /// δz — perturbation offset; absolute z = reference_z + delta_z while on a live ref.
    /// On zero-orbit / soft-continue this slot holds absolute z.
    pub delta_z: ComplexFloatExp,
    pub checkpoint: ComplexFloatExp,
    pub checkpoint_n: u32,
    /// δc — seat−reference sample while on a live ref; absolute c on zero-orbit / soft-continue.
    pub delta_c: ComplexFloatExp,
    /// ∂δ/∂c so escape_derivative stays meaningful for filament detection.
    pub dd: ComplexFloatExp,
    /// Reference generation this delta belongs to (0 = zero-orbit floor).
    pub generation: u64,
    /// Active recurrence gear for this seat.
    // r[impl cz.depth.compute-gear+1]
    pub gear: ComputeGear,
    /// Wide exponent for scaled-f64 inner recurrence.
    pub scale: FloatExp,
    /// Absolute c (anchor + generator delta_c when relative) for rebind and completion export.
    pub c: ComplexFloatExp,
}

//pub const SpeedTestPoint
#[derive(Clone, Debug)]

pub struct Point<T> {
    /// Generator sample: `delta_c` (anchor-relative) when the shell is relative;
    /// absolute `c` at seat precision when the shell is absolute.
    pub delta_c: (T, T),
    /// Absolute `c` used for naive recurrence and completion export (anchor + delta_c
    /// when relative). May be narrowed f64; perturbation stores exact c in `delta`.
    pub c: (T, T),
    /// Absolute iterate `z` (never δz).
    pub z: (T, T),
    /// Escape-time derivative ∂z/∂c (not delta_c).
    pub dc: (T, T),
    pub real_squared: T
    , pub imag_squared: T
    , pub real_imag: T
    , pub iterations: u32
    , pub loop_detection_point: ((T, T), u32)
    , pub escapes: bool
    , pub repeats: bool
    , pub delivered: bool
    // Seat coordinates materialize at first start from `c_generator`.
    , pub initialized: bool
    , pub period: u32
    , pub smallness_squared: T
    , pub small_time: u32
    , pub delta: Option<DeltaState>
    // After a Pauldelbrot glitch or exhausted published orbit: bound to the
    // zero-orbit floor for `bound_zero_generation`. A newer published generation
    // may clear `direct_only` and retry the reference.
    // r[impl cz.depth.glitch-is-unfinished+1]
    , pub direct_only: bool
    , pub bound_zero_generation: u64
}




pub trait Abs {
    fn abs(self) -> Self;
}
impl Abs for f32 {
    fn abs(self) -> Self {
        self.abs()
    }
}
impl Abs for f64 {
    fn abs(self) -> Self {
        self.abs()
    }
}
pub trait Gt {
    fn gt(self, a:Self) -> bool;
}

impl Gt for f32 {
    fn gt(self, a:Self) -> bool {
        self > a
    }
}
impl Gt for f64 {
    fn gt(self, a:Self) -> bool {
        self > a
    }
}


// r[impl cz.craft.epsilon-pixel-pitch+1]
pub fn pitch_epsilon<T:Sub<Output=T> + Abs + From<f32> + Mul<Output=T> + Copy>(points: &Vec<Point<T>>) -> T {
    (points[0].delta_c.0 - points[1].delta_c.0).abs() * (T::from(1.0 * (1.0/256.0)))
}

pub fn placeholder_point<T: From<f32> + Copy>() -> Point<T> {
    Point {
        delta_c: (0.0.into(), 0.0.into()),
        c: (0.0.into(), 0.0.into()),
        z: (0.0.into(), 0.0.into()),
        dc: (1.0.into(), 0.0.into()),
        real_squared: 0.0.into(),
        imag_squared: 0.0.into(),
        real_imag: 0.0.into(),
        iterations: 0,
        loop_detection_point: ((0.0.into(), 0.0.into()), 0),
        escapes: false,
        repeats: false,
        delivered: false,
        initialized: false,
        period: 0,
        smallness_squared: 100.0.into(),
        small_time: 0,
        delta: None,
        direct_only: false,
        bound_zero_generation: 0,
    }
}

// r[impl cz.craft.mixmap-shuffle+1]
pub(crate) fn get_random_mixmap(size: usize) -> Vec<usize> {
    let mut rng = rand::rng();
    let mut indices: Vec<usize> = (0..size).collect();
    indices.shuffle(&mut rng);
    indices
}

/// Whether an f64 grid admits this stencil (absolute, or relative fallback).
pub fn f64_stencil_admits(
    compute_loc: &(IntExp, IntExp),
    zoom_pot: i64,
    res: (u32, u32),
) -> bool {
    let view_center = view_center_compute(compute_loc, zoom_pot as i32, res);
    admit_generator::<f64>(compute_loc, zoom_pot, res, None, &view_center).is_some()
}

/// Apply admitted generator fields and invalidate undelivered seats on generation bump.
pub fn apply_generator_admission<T: Mandelbrotable + From<f32>>(
    ctx: &mut WorkContext<T>,
    admission: GeneratorAdmission<T>,
    view_center: (IntExp, IntExp),
    generation: u64,
) {
    let (c_generator, coord_anchor, coords_are_relative) = match admission {
        GeneratorAdmission::Absolute(generator) => (generator, view_center, false),
        GeneratorAdmission::Relative { generator, anchor } => (generator, anchor, true),
    };
    if ctx.generator_generation != generation {
        for p in &mut ctx.points {
            if !p.delivered {
                p.initialized = false;
                p.delta = None;
            }
        }
    }
    ctx.c_generator = c_generator;
    ctx.coord_anchor = coord_anchor;
    ctx.coords_are_relative = coords_are_relative;
    let (_, space) = c_generator.origin_and_space();
    ctx.pitch_epsilon = space.abs() * T::from(1.0 / 256.0);
    ctx.generator_generation = generation;
}

/// Rebuild `c_generator` relative to a published reference anchor.
// r[impl cz.depth.c-generator-fails-closed+1]
pub fn rebuild_generator_for_reference<T: Mandelbrotable + From<f32>>(
    ctx: &mut WorkContext<T>,
    compute_loc: &(IntExp, IntExp),
    zoom_pot: i64,
    res: (u32, u32),
    published: &PublishedReference,
) -> bool {
    let view_center = view_center_compute(compute_loc, zoom_pot as i32, res);
    let Some(admission) = admit_generator::<T>(
        compute_loc,
        zoom_pot,
        res,
        Some(&published.c),
        &view_center,
    ) else {
        return false;
    };
    apply_generator_admission(ctx, admission, view_center, published.generation);
    true
}

/// Plane C = IntExp anchor + little c (f64 host narrow).
// r[impl cz.depth.floatexp-host-coords+1]
#[inline]
pub fn c_from_delta_c_f64(delta_c: (f64, f64), anchor: &(IntExp, IntExp)) -> (f64, f64) {
    use crate::assemblies::headgroup::window::coords::f64_to_intexp;
    let re = anchor.0.clone() + f64_to_intexp(delta_c.0);
    let im = anchor.1.clone() + f64_to_intexp(delta_c.1);
    (f64::from(re), f64::from(im))
}

/// Exact plane C in FloatExp = IntExp anchor + f64 little c (per-seat precision).
#[inline]
pub fn c_floatexp_from_delta_c(delta_c: (f64, f64), anchor: &(IntExp, IntExp)) -> ComplexFloatExp {
    use crate::assemblies::headgroup::window::coords::f64_to_intexp;
    ComplexFloatExp::new(
        FloatExp::from(anchor.0.clone()) + FloatExp::from(f64_to_intexp(delta_c.0)),
        FloatExp::from(anchor.1.clone()) + FloatExp::from(f64_to_intexp(delta_c.1)),
    )
}

/// Materialize plane C for a seat sample (relative → anchor+delta_c via IntExp).
#[inline]
pub fn c_for_seat_f64(ctx: &WorkContext<f64>, delta_c: (f64, f64)) -> (f64, f64) {
    if ctx.coords_are_relative {
        c_from_delta_c_f64(delta_c, &ctx.coord_anchor)
    } else {
        delta_c
    }
}

/// Legacy alias — prefer `c_from_delta_c_f64`.
#[inline]
pub fn abs_c_f64(delta_c: (f64, f64), anchor: &(IntExp, IntExp)) -> (f64, f64) {
    c_from_delta_c_f64(delta_c, anchor)
}

/// Compute-space center of a viewport (half-res seat), exact IntExp pitch.
pub fn view_center_compute(
    compute_loc: &(IntExp, IntExp),
    zoom_pot: i32,
    res: (u32, u32),
) -> (IntExp, IntExp) {
    let exponent = zoom_pot.saturating_add(crate::constants::PIXELS_PER_UNIT_POT);
    let pitch = IntExp::from(1).shift(exponent.saturating_neg());
    (
        compute_loc.0.clone() + pitch.clone() * IntExp::from((res.0 / 2) as i32),
        compute_loc.1.clone() - pitch * IntExp::from((res.1 / 2) as i32),
    )
}

/// Synchronous view-center reference for relative shells before the async worker publishes.
/// Escaped is allowed: without a reference the f64 generator cannot stay relative to a
/// usable orbit, and zero-orbit c collapses per-seat pitch past ~2^50.
/// Prefer the longest orbit among a coarse seat sample (exterior filaments escape; longer
/// pre-escape iterates still give seats a usable Z_ref).
fn bootstrap_relative_reference<T: Mandelbrotable>(
    zoom_pot: i64,
    anchor: &(IntExp, IntExp),
    generator: &CGenerator<T>,
    res: (u32, u32),
) -> PublishedReference {
    use crate::assemblies::headgroup::window::coords::f64_to_intexp;
    let bits = bits_for_zoom(zoom_pot, PIXELS_PER_UNIT_POT).max(128);
    let mut best = {
        let orbit = ReferenceOrbit::compute(anchor, bits, 4096);
        PublishedReference {
            orbit,
            c: (anchor.0.clone(), anchor.1.clone()),
            generation: 0,
        }
    };
    let step_x = (res.0 / 8).max(1);
    let step_y = (res.1 / 8).max(1);
    let mut y = 0u32;
    while y < res.1 {
        let mut x = 0u32;
        while x < res.0 {
            let lc = generator.get_c((x, y));
            let c = (
                anchor.0.clone() + f64_to_intexp(lc.0.to_f64()),
                anchor.1.clone() + f64_to_intexp(lc.1.to_f64()),
            );
            let orbit = ReferenceOrbit::compute(&c, bits, 4096);
            if orbit.iterates.len() > best.orbit.iterates.len() {
                best = PublishedReference {
                    orbit,
                    c,
                    generation: 0,
                };
            }
            x = x.saturating_add(step_x);
            if x == 0 {
                break;
            }
        }
        y = y.saturating_add(step_y);
        if y == 0 {
            break;
        }
    }
    best
}

/// Alias for tests / depth fixtures — always-relative stencil build.
pub fn from_stencil_relative<T: Mandelbrotable + From<f32> + 'static>(
    frame_info: (ObjectivePosAndZoom, (u32, u32)),
    previous: Option<(WorkContext<T>, ObjectivePosAndZoom)>,
) -> Option<WorkContext<T>> {
    from_stencil(frame_info, previous)
}

/// Build an O(1)-coordinate shell from a stencil. Reuses the previous context's
/// point/mixmap buffers when present so steady-zoom pivots avoid large reallocs.
///
/// `previous` is `(old_context, old_objective)`.
// r[impl cz.craft.stencil-only-replace+2]
pub fn from_stencil<T: Mandelbrotable + From<f32> + 'static>(
    frame_info: (ObjectivePosAndZoom, (u32, u32)),
    previous: Option<(WorkContext<T>, ObjectivePosAndZoom)>,
) -> Option<WorkContext<T>> {
    let carried_library = previous
        .as_ref()
        .map(|(old, _)| old.reference_library.clone())
        .unwrap_or_default();
    // Greedy keep: off-screen references stay useful across zoom.
    let carried_reference = previous
        .as_ref()
        .and_then(|(old, _)| old.latest_reference.clone());
    let (carried_hud_points, carried_hud_started, carried_trial_cooldown) = previous
        .as_ref()
        .map(|(old, _)| {
            (
                old.hud_points_window,
                old.hud_window_started,
                old.pert_trial_cooldown,
            )
        })
        .unwrap_or((0, Instant::now(), 0));

    let (obj, res) = frame_info;
    let compute_loc = (obj.pos.0.clone(), IntExp::ZERO - obj.pos.1.clone());
    let view_center = view_center_compute(&compute_loc, obj.zoom_pot, res);
    let relative_anchor = carried_reference.as_ref().map(|r| &r.c);
    let generator_generation = carried_reference
        .as_ref()
        .map(|r| r.generation)
        .unwrap_or(0);
    let admission = admit_generator::<T>(
        &compute_loc,
        obj.zoom_pot as i64,
        res,
        relative_anchor,
        &view_center,
    )?;
    let coords_are_relative = admission.is_relative();
    let c_generator = *admission.generator();
    let coord_anchor = match admission {
        GeneratorAdmission::Absolute(_) => view_center,
        GeneratorAdmission::Relative { anchor, .. } => anchor,
    };
    let (_, space) = c_generator.origin_and_space();
    let seat_pitch_epsilon = space.abs() * T::from(1.0 / 256.0);
    let use_floatexp_host = std::any::TypeId::of::<T>()
        == std::any::TypeId::of::<crate::floatexp::FloatExp>();
    let view_gear = if use_floatexp_host {
        ComputeGear::FloatExp
    } else {
        // Live f64: naive/direct is F64, but deep / relative shells need the
        // compute-gear floor so completed frames do not snap HUD back to F64
        // after refresh_active_gear (issue #5).
        // r[impl cz.depth.gear-hud+2]
        // r[impl cz.depth.compute-gear+1]
        let pitch = seat_pitch_epsilon.to_f64() * 256.0;
        if coords_are_relative
            || (pitch > 0.0 && pitch < crate::delta_gear::F64_PERTURB_USEFUL_FLOOR)
        {
            ComputeGear::ScaledF64
        } else {
            ComputeGear::F64
        }
    };

    // r[impl cz.craft.pan-zoom-slot0+1]
    // Zoom takes precedence over pan when both change; neither defaults to attention.
    let motion = match previous.as_ref() {
        None => Motion::Neither,
        Some((_, old)) => {
            if obj.zoom_pot != old.zoom_pot {
                Motion::Zoomed
            } else if obj.pos.0 != old.pos.0 || obj.pos.1 != old.pos.1 {
                Motion::Panned
            } else {
                Motion::Neither
            }
        }
    };

    let new_len = (res.0 * res.1) as usize;
    let (mut points, mut random_map, old_res, mut completed_points) = match previous {
        Some((old, _)) => {
            let WorkContext {
                points,
                random_map,
                res: old_res,
                mut completed_points,
                ..
            } = old;
            completed_points.len = 0;
            (points, random_map, old_res, completed_points)
        }
        None => (
            Vec::new(),
            Vec::new(),
            (0, 0),
            // Cap at least the screen so a shallow GPU flood is not BufferFull-throttled
            // mid-shift (old fixed 100k capped home fill well below one frame).
            Stec::with_capacity(new_len.max(100_000), (CompletedPoint::Dummy {}, 0)),
        ),
    };

    points.clear();
    points.resize_with(new_len, placeholder_point);

    if old_res != res || random_map.len() != new_len {
        random_map = get_random_mixmap(new_len);
    }

    let mut edges = Vec::new();
    for i in 0..(res.0 - 1) as i32 {
        edges.push((i, 0));
    }
    for i in 0..(res.1 - 1) as i32 {
        edges.push(((res.0 - 1) as i32, i));
    }
    for i in 0..(res.0) as i32 {
        edges.push((i, (res.1 - 1) as i32));
    }
    for i in 1..(res.1 - 1) as i32 {
        edges.push((0, i));
    }
    {
        let mut rng = rand::rng();
        edges.shuffle(&mut rng);
    }

    let center = ((res.0 / 2) as i32, (res.1 / 2) as i32);

    let mut ctx = WorkContext {
        points,
        completed_points,
        index: 0,
        random_index: 0,
        time_created: Instant::now(),
        time_workshift_started: Instant::now(),
        percent_completed: 0.0,
        random_map,
        workshifts: 0,
        total_iterations: 0,
        spent_tokens_today: 0,
        total_iterations_today: 0,
        total_points_today: 0,
        total_bouts_today: 0,
        last_update: 0,
        res,
        scredge_poses: VecDeque::from(edges),
        edge_queue: VecDeque::new(),
        out_queue: VecDeque::new(),
        in_queue: VecDeque::new(),
        motion,
        attention: None,
        attention_anchor: center,
        attention_index: 0,
        attention_current: None,
        c_generator,
        pitch_epsilon: seat_pitch_epsilon,
        coord_anchor,
        view_gear,
        active_gear: view_gear,
        coords_are_relative,
        latest_reference: carried_reference.clone(),
        hud_points_window: carried_hud_points,
        hud_window_started: carried_hud_started,
        reference_floor_active: false,
        pert_trial_shifts_left: 0,
        pert_trial_cooldown: carried_trial_cooldown,
        generator_generation,
        last_used_naive_gpu: false,
        manual_gear: None,
        pps_locked_kernel: None,
        pps_lock_started: Instant::now(),
        pps_probe_queue: Vec::new(),
        pps_probe_shifts_left: 0,
        pps_probe_points: 0,
        pps_probe_started: Instant::now(),
        pps_probe_samples: Vec::new(),
        reference_library: {
            let mut lib = carried_library;
            if let Some(ref r) = carried_reference {
                if !lib.iter().any(|x| x.generation == r.generation) {
                    lib.push(r.clone());
                }
            }
            lib
        },
    };
    if ctx.coords_are_relative && ctx.latest_reference.is_none() {
        let bootstrap = bootstrap_relative_reference(
            obj.zoom_pot as i64,
            &ctx.coord_anchor,
            &ctx.c_generator,
            res,
        );
        ctx.latest_reference = Some(Arc::new(bootstrap));
        let published = ctx.latest_reference.as_ref().unwrap().clone();
        let _ = rebuild_generator_for_reference(
            &mut ctx,
            &compute_loc,
            obj.zoom_pot as i64,
            res,
            published.as_ref(),
        );
    }
    Some(ctx)
}

/// Absolute plane coordinate = IntExp anchor + relative seat sample.
// r[impl cz.depth.floatexp-host-coords+1]
#[inline]
pub fn absolute_c(
    relative: (FloatExp, FloatExp),
    anchor: &(IntExp, IntExp),
) -> (FloatExp, FloatExp) {
    (
        FloatExp::from(anchor.0.clone()) + relative.0,
        FloatExp::from(anchor.1.clone()) + relative.1,
    )
}

#[inline]
fn gear_rank(gear: ComputeGear) -> u8 {
    match gear {
        ComputeGear::F32 => 0,
        ComputeGear::F64 => 1,
        ComputeGear::ScaledF64 => 2,
        ComputeGear::FloatExp => 3,
        ComputeGear::Mixed => 4,
    }
}

/// Refresh HUD aggregate gear from seats touched this shift (O(1) per seat).
/// Full-frame scans are forbidden here — they made home fill O(n²).
/// Never demote below `view_gear`: zero-orbit / idle F64 seats must not hide a
/// deep ScaledF64 view requirement (headed "gear:F64" at the precision wall).
// r[impl cz.depth.gear-hud+2]
#[inline]
pub fn note_seat_gear<T: Mandelbrotable>(ctx: &mut WorkContext<T>, seat_gear: ComputeGear) {
    if seat_gear == ComputeGear::Mixed {
        return;
    }
    if gear_rank(seat_gear) < gear_rank(ctx.view_gear) {
        return;
    }
    if seat_gear == ctx.view_gear {
        return;
    }
    if ctx.active_gear == ctx.view_gear {
        ctx.active_gear = seat_gear;
    } else if ctx.active_gear != seat_gear {
        ctx.active_gear = ComputeGear::Mixed;
    }
}

/// Legacy name: reset to view gear then rely on `note_seat_gear` during the shift.
pub fn refresh_active_gear<T: Mandelbrotable>(ctx: &mut WorkContext<T>) {
    ctx.active_gear = ctx.view_gear;
}

/// Materialize seat coordinates from the generator on first start.
// r[impl cz.craft.stencil-only-replace+2]
#[inline]
pub fn ensure_started<T: Mandelbrotable>(ctx: &mut WorkContext<T>, pos: (i32, i32)) {
    let index = index_from_pos(&pos, ctx.res.0);
    let point = &mut ctx.points[index];
    if !point.initialized {
        let delta_c = ctx.c_generator.get_c((pos.0 as u32, pos.1 as u32));
        point.delta_c = delta_c;
        point.c = delta_c;
        point.z = delta_c;
        point.dc = (T::ONE, T::ZERO);
        point.initialized = true;
    }
}

/// Update live attention. `None` means the pointer is off-screen (or unset):
/// the spiral keeps / restores the screen-center anchor.
// r[impl cz.craft.attention-spiral+1]
pub fn set_attention<T: Mandelbrotable>(ctx: &mut WorkContext<T>, attention: Option<(i32, i32)>) {
    ctx.attention = attention;
    let anchor = match attention {
        Some(pos) => pos,
        None => ((ctx.res.0 / 2) as i32, (ctx.res.1 / 2) as i32),
    };
    if anchor != ctx.attention_anchor {
        ctx.attention_anchor = anchor;
        ctx.attention_index = 0;
        ctx.attention_current = None;
    }
}

/// Integer floor sqrt for the square-ring index.
#[inline]
fn isqrt_u64(n: u64) -> u64 {
    if n <= 1 {
        return n;
    }
    // Newton's method; avoid (n+1) overflow at u64::MAX.
    let mut x = 1u64 << ((64 - n.leading_zeros() + 1) / 2);
    loop {
        let y = (x + n / x) / 2;
        if y >= x {
            return x;
        }
        x = y;
    }
}

/// Offset from spiral index `k` on a square ring (Ulam-style).
/// Ring 0 is the origin; ring r has 8r seats.
#[inline]
pub fn square_ring_offset(k: u64) -> (i32, i32) {
    if k == 0 {
        return (0, 0);
    }
    let s = isqrt_u64(k);
    let r = ((s + 1) / 2) as i32;
    let r_u = r as u64;
    let start = (2 * r_u - 1) * (2 * r_u - 1);
    let t = (k - start) as i32;
    let side = 2 * r;
    if t < side {
        (r, -r + 1 + t)
    } else if t < 2 * side {
        (r - 1 - (t - side), r)
    } else if t < 3 * side {
        (-r, r - 1 - (t - 2 * side))
    } else {
        (-r + 1 + (t - 3 * side), -r)
    }
}

const ATTENTION_SCAN_CAP: u32 = 64;

/// Advance the attention spiral to the next in-bounds, undelivered seat.
/// Returns `None` when the spiral is exhausted or the scan budget is spent.
// r[impl cz.craft.attention-spiral+1]
pub fn next_attention_spiral_pos<T: Mandelbrotable>(
    ctx: &mut WorkContext<T>,
) -> Option<(i32, i32)> {
    let max_ring = max(ctx.res.0, ctx.res.1) as u64;
    // Indices past (2*max_ring+1)^2 lie outside every on-screen ring.
    let max_index = (2 * max_ring + 1).saturating_mul(2 * max_ring + 1);
    let (ax, ay) = ctx.attention_anchor;
    for _ in 0..ATTENTION_SCAN_CAP {
        let k = ctx.attention_index;
        if k >= max_index {
            return None;
        }
        let (dx, dy) = square_ring_offset(k);
        ctx.attention_index = k + 1;
        let pos = (ax + dx, ay + dy);
        if pos.0 < 0
            || pos.1 < 0
            || pos.0 >= ctx.res.0 as i32
            || pos.1 >= ctx.res.1 as i32
        {
            continue;
        }
        let index = index_from_pos(&pos, ctx.res.0);
        if !ctx.points[index].delivered {
            return Some(pos);
        }
    }
    None
}

pub(crate) fn queue_fallback_pos_pub<T: Mandelbrotable>(
    context: &WorkContext<T>,
    prefer_scredge: bool,
) -> Option<((i32, i32), Step)> {
    queue_fallback_pos(context, prefer_scredge)
}

fn queue_fallback_pos<T: Mandelbrotable>(
    context: &WorkContext<T>,
    prefer_scredge: bool,
) -> Option<((i32, i32), Step)> {
    if prefer_scredge {
        if let Some(pos) = context.scredge_poses.front() {
            return Some((*pos, Step::Scredge));
        }
    }
    if let Some((pos, _)) = context.edge_queue.front() {
        return Some((*pos, Step::Edge));
    }
    if let Some((pos, _)) = context.out_queue.front() {
        return Some((*pos, Step::Out));
    }
    if !prefer_scredge {
        if let Some(pos) = context.scredge_poses.front() {
            return Some((*pos, Step::Scredge));
        }
    }
    if let Some((pos, _)) = context.in_queue.front() {
        return Some((*pos, Step::In));
    }
    None
}

/// The swappable numerical implementation run by the golden scheduler.
///
/// Queue choice, attention, backpressure, and wall-clock policy remain in
/// `workshift_with_kernel`; a kernel may only start one seat, run one bounded
/// bout, and turn a finished seat into its answer.
// r[impl cz.craft.kernel-seam+1]
pub trait SeatKernel<T>
where
    T: Mandelbrotable + std::fmt::Debug + Finite + Gt + Abs + From<f32> + Into<f64>,
{
    fn start_seat(&self, context: &mut WorkContext<T>, pos: (i32, i32));
    fn iterate_bout(
        &self,
        point: &mut Point<T>,
        reference: Option<&ReferenceOrbit>,
        r_squared: T,
        epsilon: T,
        cap: BoutCap,
    );
    fn completion(&self, point: &mut Point<T>) -> CompletedPoint<T>;
}

/// Production naive Mandelbrot iteration (`mode:naive`).
// r[impl cz.perf.pps-selected-kernel+1]
#[derive(Clone, Copy, Debug, Default)]
pub struct DirectKernel;

impl<T> SeatKernel<T> for DirectKernel
where
    T: Mandelbrotable + std::fmt::Debug + Finite + Gt + Abs + From<f32> + Into<f64>,
{
    #[inline]
    fn start_seat(&self, context: &mut WorkContext<T>, pos: (i32, i32)) {
        ensure_started(context, pos);
        if context.coords_are_relative {
            let index = index_from_pos(&pos, context.res.0);
            let point = &mut context.points[index];
            let delta_c = point.delta_c;
            let c = c_from_delta_c_f64(
                (delta_c.0.into(), delta_c.1.into()),
                &context.coord_anchor,
            );
            point.c = (
                T::from(crate::assemblies::headgroup::window::coords::f64_to_intexp(c.0)),
                T::from(crate::assemblies::headgroup::window::coords::f64_to_intexp(c.1)),
            );
            point.z = point.c;
        }
    }

    #[inline]
    fn iterate_bout(
        &self,
        point: &mut Point<T>,
        _reference: Option<&ReferenceOrbit>,
        r_squared: T,
        epsilon: T,
        cap: BoutCap,
    ) {
        iterate_max_n_times(point, r_squared, epsilon, cap);
    }

    #[inline]
    fn completion(&self, point: &mut Point<T>) -> CompletedPoint<T> {
        direct_completion(point)
    }
}

/// Shared period / escape completion used by both kernels.
pub fn direct_completion<T>(point: &mut Point<T>) -> CompletedPoint<T>
where
    T: Mandelbrotable + Into<f64> + Copy,
{
    direct_completion_with_c(point, point.c)
}

pub fn direct_completion_with_c<T>(
    point: &mut Point<T>,
    c: (T, T),
) -> CompletedPoint<T>
where
    T: Mandelbrotable + Into<f64> + Copy,
{
    if point.repeats {
        let c64 = (c.0.into(), c.1.into());
        let (partials, tail) = period_partials(c64, point.iterations);
        point.period = partials
            .into_iter()
            .find_map(|p| verified_period_from(c64, p, tail))
            .unwrap_or(0);
        CompletedPoint::Repeats {
            period: point.period,
            smallness: point.smallness_squared,
            small_time: point.small_time,
        }
    } else {
        CompletedPoint::Escapes {
            escape_time: point.iterations,
            escape_location: (point.z.0, point.z.1),
            escape_derivative: point.dc,
            start_location: (c.0, c.1),
            smallness: point.smallness_squared,
            small_time: point.small_time,
        }
    }
}

pub fn workshift(
    day_token_allowance: u32,
    iteration_token_cost: u32,
    point_token_cost: u32,
    bout_token_cost: u32,
    context: &mut WorkContext<f64>,
    gpu: Option<&mut super::naive_gpu::NaiveGpuContext>,
) {
    if context.pert_trial_cooldown > 0 {
        context.pert_trial_cooldown -= 1;
    }
    context.last_used_naive_gpu = false;
    let gpu_available = gpu.is_some();
    context.ensure_pps_probe(gpu_available);
    // Soft-trial only after the PPS race locks (do not fight the probe).
    let policy_before = if context.pps_locked_kernel.is_some() || context.manual_gear.is_some()
    {
        context.update_reference_floor_policy()
    } else {
        "probing"
    };
    run_workshift_kernel(
        day_token_allowance,
        iteration_token_cost,
        point_token_cost,
        bout_token_cost,
        context,
        gpu,
    );
    let points_delta = context.total_points_today;
    context.finish_pps_probe_shift(points_delta);
    let trial_tick = context.tick_pert_trial();
    if (policy_before != "no_ref" && policy_before != "probing") || trial_tick.is_some() {
        let _ = context.update_reference_floor_policy();
    }
}

fn run_workshift_kernel(
    day_token_allowance: u32,
    iteration_token_cost: u32,
    point_token_cost: u32,
    bout_token_cost: u32,
    context: &mut WorkContext<f64>,
    gpu: Option<&mut super::naive_gpu::NaiveGpuContext>,
) {
    use crate::assemblies::structs::KernelMode;
    let gpu_available = gpu.is_some();
    let mode = context.dispatch_kernel(gpu_available);
    match mode {
        KernelMode::Pert => {
            workshift_with_kernel(
                day_token_allowance,
                iteration_token_cost,
                point_token_cost,
                bout_token_cost,
                context,
                &super::perturb_kernel::PerturbationKernel,
            );
        }
        KernelMode::NaiveGpu => {
            if let Some(gpu) = gpu {
                super::naive_gpu::workshift_naive_gpu(
                    day_token_allowance,
                    iteration_token_cost,
                    point_token_cost,
                    bout_token_cost,
                    context,
                    gpu,
                );
            } else {
                workshift_with_kernel(
                    day_token_allowance,
                    iteration_token_cost,
                    point_token_cost,
                    bout_token_cost,
                    context,
                    &DirectKernel,
                );
            }
        }
        KernelMode::Naive => {
            workshift_with_kernel(
                day_token_allowance,
                iteration_token_cost,
                point_token_cost,
                bout_token_cost,
                context,
                &DirectKernel,
            );
        }
    }
}

pub fn workshift_with_kernel<T, K>(
    day_token_allowance: u32,
    iteration_token_cost: u32,
    point_token_cost: u32,
    bout_token_cost: u32,
    context: &mut WorkContext<T>,
    kernel: &K,
)
where
    T: Mandelbrotable + Sub<Output=T> + std::fmt::Debug + Add<Output=T> + Mul<Output=T> + Into<f64> + PartialOrd + Finite + Gt + Abs + From<f32> + Copy,
    K: SeatKernel<T>,
{

    context.time_workshift_started = Instant::now();

    context.update_reference_floor_policy();


    context.total_bouts_today = 0;
    context.total_iterations_today = 0;
    context.total_points_today = 0;
    context.spent_tokens_today = 0;
    // r[impl cz.depth.gear-hud+2]
    refresh_active_gear(context);



    // r[impl cz.craft.epsilon-pixel-pitch+1]
    let episilon = context.pitch_epsilon;


    let total_points = context.points.len();
    context.random_index = context.random_map[min(context.index, total_points-1)];


    // r[impl cz.craft.wall-clock-law+1]
    while context.time_workshift_started.elapsed().as_millis()<10{//while context.index < total_points && context.spent_tokens_today + bout_token_cost + 1000 * iteration_token_cost * point_token_cost < day_token_allowance { // workbout loop


        // r[impl cz.craft.attention-spiral+1]
        // r[impl cz.craft.pan-zoom-slot0+1]
        // Slot 0's leading phase is the only motion-dependent choice:
        //   Zoomed / Neither → Attention
        //   Panned           → Scredge
        // Everything else in the rotation is unchanged.
        let (pos, step) = match context.workshifts%5 {
            0 => {
                // Pan only owns the first shift of a fresh shell. After that
                // (or when the user has stopped and no new Replace arrives)
                // fall back to attention like Neither.
                if context.motion == Motion::Panned && context.workshifts == 0 {
                    if let Some(p) = queue_fallback_pos(context, true) {
                        p
                    } else if let Some(pos) = context.attention_current {
                        (pos, Step::Attention)
                    } else if let Some(pos) = next_attention_spiral_pos(context) {
                        context.attention_current = Some(pos);
                        (pos, Step::Attention)
                    } else {
                        context.index = total_points-1; break;
                    }
                } else if let Some(pos) = context.attention_current {
                    (pos, Step::Attention)
                } else if let Some(pos) = next_attention_spiral_pos(context) {
                    context.attention_current = Some(pos);
                    (pos, Step::Attention)
                } else if let Some(p) = queue_fallback_pos(context, context.workshifts == 0) {
                    // r[impl cz.craft.scredge-first-shift0+1]
                    p
                } else {
                    context.index = total_points-1; break;
                }
            }
            1 => {
                if let Some(p) = queue_fallback_pos(context, false) {
                    // Prefer edge: queue_fallback already does edge→out→scredge→in
                    p
                } else {
                    context.index = total_points-1; break;
                }
            }
            2 => {
                // Out first
                if context.out_queue.len()>0{
                    (context.out_queue[0].0, Step::Out)
                } else if let Some(p) = queue_fallback_pos(context, false) {
                    p
                } else {
                    context.index = total_points-1; break;
                }
            }
            3 => {
                if let Some(p) = queue_fallback_pos(context, false) {
                    p
                } else {
                    context.index = total_points-1; break;
                }
            }
            4 => {
                // Scredge first this slot
                if let Some(p) = queue_fallback_pos(context, true) {
                    p
                } else {
                    context.index = total_points-1; break;
                }
            }
            _ => {break}
        };

        let index = index_from_pos(&pos, context.res.0);

        if context.points[index].delivered {
            match step {
                Step::Out => {
                    let _ =  context.out_queue.pop_front();
                }
                Step::Scredge => {
                    let _ = context.scredge_poses.pop_front();
                }
                Step::In => {
                    let _ =  context.in_queue.pop_front();
                }
                Step::Edge => {
                    let _ =  context.edge_queue.pop_front();
                }
                Step::Attention => {
                    // Held seat got delivered elsewhere; release it so the
                    // next bout advances the spiral instead of spinning.
                    context.attention_current = None;
                }
            }
            continue;
        }

        // Capture before start_seat: a same-bout glitch restart can drop
        // iterations — never panic the IPS counter on that non-monotonic path.
        let old_iterations = context.points[index].iterations;

        // r[impl cz.craft.stencil-only-replace+2]
        kernel.start_seat(context, pos);

        // Disjoint fields: take the reference so we can mutably borrow the seat.
        // Prefer the library member matching this seat's bound generation so
        // off-screen / multi-ref picks stay coherent through iterate_bout.
        let held_reference = context.latest_reference.take();
        let bound_gen = context.points[index]
            .delta
            .as_ref()
            .map(|d| d.generation)
            .filter(|_| !context.points[index].direct_only);
        let seat_orbit_arc = bound_gen.and_then(|g| {
            context
                .reference_library
                .iter()
                .find(|r| r.generation == g)
                .cloned()
                .or_else(|| {
                    held_reference
                        .clone()
                        .filter(|r| r.generation == g)
                })
        });
        let use_published_orbit = !context.points[index].direct_only
            && (seat_orbit_arc.is_some()
                || context.reference_floor_active
                || (context.coords_are_relative && held_reference.is_some()));
        let orbit = if use_published_orbit {
            seat_orbit_arc
                .as_ref()
                .map(|r| &r.orbit)
                .or_else(|| held_reference.as_ref().map(|r| &r.orbit))
        } else {
            None
        };

        // r[impl cz.craft.attention-spiral+1]
        // Every bout is bounded — the worker may never make an unbounded call.
        // Attention tenacity is carried by `attention_current` across bouts,
        // not by an uncapped iteration count inside a single bout.
        kernel.iterate_bout(
            &mut context.points[index],
            orbit,
            4.0f32.into(),
            episilon,
            BoutCap::STANDARD,
        );
        context.latest_reference = held_reference;
        if let Some(d) = context.points[index].delta.as_ref() {
            note_seat_gear(context, d.gear);
        }



        context.total_iterations_today += context.points[index]
            .iterations
            .saturating_sub(old_iterations);


        if context.points[index].repeats || context.points[index].escapes {

            //context.already_done.push(context.index);
            //context.already_done_hashset.insert(context.index);
            context.total_iterations += context.points[index].iterations;



            match step {
                Step::Out => {
                    let _ =  context.out_queue.pop_front();
                }
                Step::Scredge => {
                    let _ = context.scredge_poses.pop_front();
                }
                Step::In => {
                    let _ =  context.in_queue.pop_front();
                }
                Step::Edge => {
                    let _ =  context.edge_queue.pop_front();
                }
                Step::Attention => {
                    // Held seat finished: release it so the next attention
                    // bout advances the spiral to the next undelivered seat.
                    context.attention_current = None;
                }
            }

            // Candidate periods are verified inside the kernel; scheduler
            // side effects (neighbor discovery and queue policy) stay here.
            let completed_point = kernel.completion(&mut context.points[index]);
            if context.points[index].repeats {
                queue_incomplete_neighbors_in(&pos, context.res, &context.points, &mut context.in_queue);
            } else {
                queue_incomplete_neighbors(&pos, context.res, &context.points, &mut context.out_queue);
            }

            if let Some(e) = point_is_edge(&pos, context.res, &context.points) {
                //context.edge_queue.clear();
                queue_incomplete_neighbors_of_edge(&e.0, &e.1, context.res, &context.points, &mut context.edge_queue);
            }

            // r[impl cz.craft.provisional-not-delivered+1]
            // r[impl cz.craft.undeliver-on-full+1]
            match context.push_delivery(Delivery::Final(completed_point), index) {
                PushOutcome::Published => {}
                PushOutcome::BufferFull => { break; }
            }


            context.total_points_today += 1;
            context.record_hud_completion_batch(1);
        } else {
            match step {
                // r[impl cz.craft.out-rotates-in-stays+1]
                Step::Out => {
                    let pos = context.out_queue.pop_front().unwrap();
                    context.out_queue.push_back(pos);
                    continue;
                }
                /*Step::In => {
                    let pos = context.in_queue.pop_front().unwrap();
                    context.in_queue.push_back(pos);
                    continue;
                }*/
                Step::Scredge => {
                    //let pos = context.scredge_poses.pop_front().unwrap();
                    //context.scredge_poses.push_back(pos);
                    // r[impl cz.craft.provisional-not-delivered+1]
                    // Provisional: publishes data but cannot mark the seat delivered.
                    let provisional = CompletedPoint::Repeats{
                        period: 0,
                        smallness: context.points[index].smallness_squared,
                        small_time: context.points[index].small_time,
                    };
                    match context.push_delivery(Delivery::Provisional(provisional), index) {
                        PushOutcome::Published => { continue; }
                        PushOutcome::BufferFull => { break; }
                    }
                }
                _ => {}
            }
        }

        context.total_bouts_today += 1;
        context.spent_tokens_today = context.total_bouts_today * bout_token_cost + context.total_points_today * point_token_cost + context.total_iterations_today * point_token_cost;
    }

    context.workshifts += 1;
    // r[impl cz.craft.load-proportional-ignorance+1]
    // Idle metric is delivered fraction, not the empty-queue break index.
    let delivered = context.points.iter().filter(|p| p.delivered).count();
    context.percent_completed = delivered as f64 / (total_points as f64) * 100.0;
}

/// Hard ceiling for any single iteration bout. The worker must never make an
/// unbounded call; the 10 ms wall-clock check at the top of the bout loop is
/// only valid if no call inside the loop can run away. This type makes the cap
/// a construction-time fact rather than a convention.
pub const MAX_BOUT: u32 = 1000;

/// Bounded iteration cap. The only constructor clamps to `MAX_BOUT`, so an
/// unbounded (or merely huge) count cannot be expressed at a call site.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BoutCap(u32);

impl BoutCap {
    /// Values above `MAX_BOUT` clamp; they never exceed it.
    pub const fn new(n: u32) -> Self {
        if n > MAX_BOUT { BoutCap(MAX_BOUT) } else { BoutCap(n) }
    }
    /// The standard full bout.
    pub const STANDARD: BoutCap = BoutCap(MAX_BOUT);
    #[inline]
    pub const fn get(self) -> u32 { self.0 }
}

#[inline]
pub fn iterate_max_n_times<T:Sub<Output=T> + Add<Output=T> + Mul<Output=T> + Into<f64>+ PartialOrd + Gt +From<f32>+ Copy> (point: &mut Point<T>, r_squared:T, epsilon:T, cap: BoutCap) {
    for _ in 0..cap.get() {
        update_point_results(point);
        point.escapes = bailout_point(point, r_squared);// || (!point.real_squared.is_finite()) || (!point.imag_squared.is_finite());
        if !(point.escapes || point.repeats) {
            iterate(point);
        } else {
            break;
        }
        point.repeats = loop_check_point(point, epsilon);
        update_loop_check_points(point);
    }
}


pub trait Finite {
    fn is_finite(self) -> bool;
}
impl Finite for f32 {
    fn is_finite(self) -> bool {
        self.is_finite()
    }
}
impl Finite for f64 {
    fn is_finite(self) -> bool {
        self.is_finite()
    }
}
impl Finite for FloatExp {
    fn is_finite(self) -> bool {
        self.mantissa.is_finite()
    }
}
impl Gt for FloatExp {
    fn gt(self, a: Self) -> bool {
        self > a
    }
}
impl Abs for FloatExp {
    fn abs(self) -> Self {
        self.abs()
    }
}


use std::ops::*;

#[inline(always)]
pub fn iterate<T:Sub<Output=T> + Add<Output=T> + Mul<Output=T> + From<f32> + Copy> (point: &mut Point<T>) {
    iterate_with_c(point, point.c);
}

#[inline(always)]
pub fn iterate_with_c<T:Sub<Output=T> + Add<Output=T> + Mul<Output=T> + From<f32> + Copy> (
    point: &mut Point<T>,
    c: (T, T),
) {
    // r[impl cz.craft.screen-space-derivative-edges+1]
    // d_z/d_c recurrence: (d z)/(d c)_{n+1} = 2 z_n (d z)/(d c)_n + 1.
    let d_z_d_c = (
        T::from(2.0) * (point.z.0 * point.dc.0 - point.z.1 * point.dc.1) + T::from(1.0),
        T::from(2.0) * (point.z.0 * point.dc.1 + point.z.1 * point.dc.0),
    );
    // move z
    point.z = (
        point.real_squared - point.imag_squared + c.0
        , T::from(2.0f32.into()) * point.real_imag + c.1
    );
    point.dc = d_z_d_c;
    point.iterations+=1;
}

use std::cmp::*;
#[inline(always)]
pub fn bailout_point<T:Sub<Output=T> + Add<Output=T> + Mul<Output=T> + Gt + PartialOrd + Copy> (point: & Point<T>, r_squared:T) -> bool {
    // checks

    point.real_squared + point.imag_squared > r_squared
}

#[inline(always)]
fn points_near<T:Sub<Output=T> + Add<Output=T> + Mul<Output=T> + PartialOrd + Copy> (z1: (T, T), z2: (T, T), e: T) -> bool {
    z1.0 >= (z2.0 - e) && z1.0 <= (z2.0 + e)
    && z1.1 >= (z2.1 - e) && z1.1 <= (z2.1 + e)
}

#[inline(always)]
fn loop_check_point<T:Sub<Output=T> + Add<Output=T> + PartialOrd + Mul<Output=T> + Copy> (point: &mut  Point<T>, epsilon:T) -> bool {
    let near = points_near(point.z, point.loop_detection_point.0, epsilon);

    if near {point.period = point.iterations-point.loop_detection_point.1}
    near
}

#[inline(always)]
fn update_loop_check_points<T:Sub<Output=T> + Add<Output=T> + Mul<Output=T> + Copy> (point: &mut Point<T>) {

    if point.iterations >= point.loop_detection_point.1 << 1 {
        point.loop_detection_point = (point.z, point.iterations);
    }

}

fn iterate_complex(z: (f64, f64), c: (f64, f64)) -> (f64, f64) {
    (z.0 * z.0 - z.1 * z.1 + c.0, 2.0 * z.0 * z.1 + c.1)
}

// Record-minimum steps of the critical orbit (atom-domain partials), ascending,
// plus the tail iterate after `max` steps. Candidates must be tried in this
// order: the first that verifies is the true period. Using only the LAST record
// (e.g. small_time) is wrong — an interior orbit keeps setting minima as it
// converges, so the last record is the convergence time, and Newton then
// verifies a multiple of the true period. The tail iterate rides the attracting
// cycle and is the best Newton start; F^p(0,c) (the published guess) is far
// from the attractor exactly where necks make convergence slow.
pub fn period_partials(c: (f64, f64), max: u32) -> (Vec<u32>, (f64, f64)) {
    let mut z = c; // z_1: this codebase's orbit convention starts at c
    let mut best = f64::MAX;
    let mut out = Vec::new();
    for n in 1..=max {
        let r = z.0 * z.0 + z.1 * z.1;
        if r < best {
            best = r;
            out.push(n);
        }
        z = iterate_complex(z, c);
    }
    (out, z)
}

// r[impl cz.craft.period-derivative-test+1]
pub fn verified_period(c: (f64, f64), period: u32) -> Option<u32> {
    // F^p(0,c) is the published Newton starting point.
    let mut w = (0.0, 0.0);
    for _ in 0..period {
        w = iterate_complex(w, c);
    }
    verified_period_from(c, period, w)
}

// r[impl cz.craft.period-derivative-test+1]
pub fn verified_period_from(
    c: (f64, f64),
    period: u32,
    start: (f64, f64),
) -> Option<u32> {
    if period == 0 {
        return None;
    }

    let mut w = start;

    // Solve F^p(w,c)=w.  Stop only when another Newton step is exactly
    // unrepresentable in f64; the mathematical acceptance test below has no epsilon.
    // Budget is generous because necks between hyperbolic components are
    // parabolic (multiplier on the unit circle): convergence is linear there,
    // with quadratic behavior only resuming within ~δ of the attractor.
    let mut converged = false;
    let mut previous = None;
    for _ in 0..128 {
        let mut z = w;
        let mut dz = (1.0, 0.0);
        for _ in 0..period {
            dz = (
                2.0 * (z.0 * dz.0 - z.1 * dz.1),
                2.0 * (z.0 * dz.1 + z.1 * dz.0),
            );
            z = iterate_complex(z, c);
        }

        let numerator = (z.0 - w.0, z.1 - w.1);
        let denominator = (dz.0 - 1.0, dz.1);
        let denominator_norm = denominator.0 * denominator.0
            + denominator.1 * denominator.1;
        if denominator_norm == 0.0 || !denominator_norm.is_finite() {
            return None;
        }
        let quotient = (
            (numerator.0 * denominator.0 + numerator.1 * denominator.1)
                / denominator_norm,
            (numerator.1 * denominator.0 - numerator.0 * denominator.1)
                / denominator_norm,
        );
        let next = (w.0 - quotient.0, w.1 - quotient.1);
        if !next.0.is_finite() || !next.1.is_finite() {
            return None;
        }
        let correction_norm = quotient.0 * quotient.0 + quotient.1 * quotient.1;
        let scale = (w.0 * w.0 + w.1 * w.1).max(1.0);
        if next == w
            || previous == Some(next)
            || correction_norm <= f64::EPSILON * f64::EPSILON * scale
        {
            converged = true;
            w = next;
            break;
        }
        previous = Some(w);
        w = next;
    }
    if !converged {
        return None;
    }

    // Newton can land on a divisor of the candidate period (fixed points also
    // satisfy F^p(w)=w). Reduce to the true minimal period of the converged
    // attractor before the multiplier test; the cycle points are O(1) apart,
    // so the closeness bound here is only absorbing f64 roundoff, not deciding
    // membership (that is the exact unit-circle multiplier test below).
    let mut z = w;
    let mut minimal_period = period;
    for d in 1..period {
        z = iterate_complex(z, c);
        if period % d == 0 {
            let diff = (z.0 - w.0, z.1 - w.1);
            let norm = diff.0 * diff.0 + diff.1 * diff.1;
            let scale = (w.0 * w.0 + w.1 * w.1).max(1.0);
            if norm <= 1e-24 * scale {
                minimal_period = d;
                break;
            }
        }
    }

    // The cycle multiplier is exact in the mathematical model: attracting iff |b| <= 1.
    let mut z = w;
    let mut multiplier = (1.0, 0.0);
    for _ in 0..minimal_period {
        multiplier = (
            2.0 * (z.0 * multiplier.0 - z.1 * multiplier.1),
            2.0 * (z.0 * multiplier.1 + z.1 * multiplier.0),
        );
        z = iterate_complex(z, c);
    }
    let multiplier_norm =
        multiplier.0 * multiplier.0 + multiplier.1 * multiplier.1;
    (multiplier_norm <= 1.0).then_some(minimal_period)
}

#[inline]
// r[impl cz.craft.cached-products+1]
pub fn update_point_results<T:Sub<Output=T> + Add<Output=T> + Into<f64> + Gt + Mul<Output=T> + Copy>(point: &mut Point<T>) {
    // update values
    point.real_squared = point.z.0 * point.z.0;
    point.imag_squared = point.z.1 * point.z.1;
    point.real_imag = point.z.0 * point.z.1;
    let rad = point.real_squared + point.imag_squared;
    if rad.into() < point.smallness_squared.into() {point.smallness_squared =rad;point.small_time=point.iterations}

}



// r[impl cz.craft.cost-metadata+1]
pub fn queue_incomplete_neighbors<T:Sub<Output=T> + Add<Output=T> + Mul<Output=T> + Copy>(pos:&(i32, i32), res: (u32, u32), points: &Vec<Point<T>>, queue: &mut VecDeque<((i32, i32), u32)>) {

    let difficulty = points[index_from_pos(pos, res.0)].iterations;

    let wid = res.0;

    let neighbors: [(i32, i32);4] = [
        (pos.0+1, pos.1)
        , (pos.0-1, pos.1)
        , (pos.0, pos.1+1)
        , (pos.0, pos.1-1)
    ];
    for n in neighbors {

        if (
            n.0 >= 0 && n.0 <= res.0 as i32 - 1
            && n.1 >= 0 && n.1 <= res.1 as i32 - 1
            ) {
            let index = index_from_pos(&n, wid);
            if !points[index].delivered {
                queue.push_back((n, difficulty));
            }
        }
    }
}

pub fn queue_incomplete_neighbors_in<T:Sub<Output=T> + Add<Output=T> + Mul<Output=T> + Copy>(pos:&(i32, i32), res: (u32, u32), points: &Vec<Point<T>>, queue: &mut VecDeque<((i32, i32), u32)>) {

    let period = points[index_from_pos(&pos, res.0)].period;

    let wid = res.0;

    let neighbors: [(i32, i32);4] = [
        (pos.0+1, pos.1)
        , (pos.0-1, pos.1)
        , (pos.0, pos.1+1)
        , (pos.0, pos.1-1)
    ];
    for n in neighbors {

        if (
            n.0 >= 0 && n.0 <= res.0 as i32 - 1
                && n.1 >= 0 && n.1 <= res.1 as i32 - 1
        ) {
            let index = index_from_pos(&n, wid);
            if !points[index].delivered {
                queue.push_back((n, period));
            }
        }
    }
}

pub fn point_is_edge<T:Sub<Output=T> + Add<Output=T> + Mul<Output=T> + Copy> (pos:&(i32, i32), res: (u32, u32), points: &Vec<Point<T>>) -> Option<((i32, i32), (i32, i32))> {
    let neighbors: [(i32, i32);4] = [
        (pos.0+1, pos.1)
        , (pos.0-1, pos.1)
        , (pos.0, pos.1+1)
        , (pos.0, pos.1-1)
    ];

    let index = index_from_pos(&pos, res.0);
    for n in neighbors {

        if (
            n.0 >= 0 && n.0 <= res.0 as i32 - 1
                && n.1 >= 0 && n.1 <= res.1 as i32 - 1
        ) {
            let nindex = index_from_pos(&n, res.0);
            if (points[index].escapes || points[index].repeats)
                && (points[nindex].escapes || points[nindex].repeats)
            {
                if points[index].escapes != points[nindex].escapes || points[index].repeats != points[nindex].repeats {
                    return Some((*pos, n));
                } else if points[index].repeats == true {
                    // period 0 = verified-unknown; unknown must not light a filament
                    if points[index].period != 0 && points[nindex].period != 0
                        && points[index].period != points[nindex].period {
                        return Some((*pos, n));
                    }
                }
            }
        }
    }
    None
}

pub fn queue_incomplete_neighbors_of_edge<T:Sub<Output=T> + Add<Output=T> + Mul<Output=T> + Copy>(pos1:&(i32, i32), pos2:&(i32, i32), res: (u32, u32), points: &Vec<Point<T>>, queue: &mut VecDeque<((i32, i32), u32)>) {

    let difficulty = points[index_from_pos(pos1, res.0)].iterations;

    let wid = res.0;

    let neighbors: [(i32, i32);8] = if (pos1.0 - pos2.0).abs()==1 { // horizontal
        if pos1.0>pos2.0 { // pos1 more right
            [
                (pos1.0, pos1.1+1)
                , (pos2.0, pos2.1+1)
                , (pos2.0, pos2.1-1)
                , (pos1.0, pos1.1-1)
                , (pos1.0+1, pos1.1+1)
                , (pos2.0-1, pos2.1+1)
                , (pos2.0-1, pos2.1-1)
                , (pos1.0+1, pos1.1-1)
                //, (pos1.0+1, pos1.1)
                //, (pos2.0-1, pos2.1)
            ]
        } else { // pos2 more right
            [
                (pos2.0, pos2.1+1)
                , (pos1.0, pos1.1+1)
                , (pos1.0, pos1.1-1)
                , (pos2.0, pos2.1-1)
                , (pos2.0+1, pos2.1+1)
                , (pos1.0-1, pos1.1+1)
                , (pos1.0-1, pos1.1-1)
                , (pos2.0+1, pos2.1-1)
                //, (pos2.0+1, pos2.1)
                //, (pos1.0-1, pos1.1)
            ]
        }
    } else { // vertical
        if pos1.0>pos2.0 { // pos1 higher
            [
                (pos1.0+1, pos1.1)
                , (pos2.0+1, pos2.1)
                , (pos1.0-1, pos1.1)
                , (pos2.0-1, pos2.1)
                , (pos1.0+1, pos1.1+1)
                , (pos2.0+1, pos2.1-1)
                , (pos2.0-1, pos2.1-1)
                , (pos1.0-1, pos1.1+1)
                //, (pos1.0, pos1.1+1)
                //, (pos2.0, pos2.1-1)
            ]
        } else { // pos2 higher
            [
                (pos1.0+1, pos1.1)
                , (pos2.0+1, pos2.1)
                , (pos2.0-1, pos2.1)
                , (pos1.0-1, pos1.1)
                , (pos2.0+1, pos2.1+1)
                , (pos1.0+1, pos1.1-1)
                , (pos1.0-1, pos1.1-1)
                , (pos2.0-1, pos2.1+1)
                //, (pos2.0, pos2.1+1)
                //, (pos1.0, pos1.1-1)
            ]
        }
    };

    /*let neighbors: [(i32, i32);4] = [
        (pos.0+1, pos.1)
        , (pos.0-1, pos.1)
        , (pos.0, pos.1+1)
        , (pos.0, pos.1-1)
    ];*/
    for n in neighbors {

        if (
            n.0 >= 0 && n.0 <= res.0 as i32 - 1
                && n.1 >= 0 && n.1 <= res.1 as i32 - 1
        ) {
            let index = index_from_pos(&n, wid);
            if !points[index].delivered {
                // r[impl cz.craft.edge-push-front+1]
                queue.push_front((n, difficulty));
            }
        }
    }
}