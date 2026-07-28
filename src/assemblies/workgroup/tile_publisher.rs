//! Tile publisher: calibrated → Answer with proximate bias / NORES.
//! Design: docs/design/tile_publisher.md
// r[impl cz.int.publisher-nores-bias+1]
// r[impl cz.int.publish-cadence+1]

use std::time::{Duration, Instant};

use crate::assemblies::structs::*;
use crate::assemblies::workgroup::structs::*;
use crate::constants::NORES_ANSWER;
use crate::constants::TILE_EDGE_LENGTH;
use crate::range::Range;
use steady_state::*;

/// Convert one calibrated seat using optional proximate bias.
/// Finished Outside/Inside collapse honestly (bias when provided for continuity).
/// Agnostic with proximate → biased guess; without proximate → NORES (never invent Inside).
///
/// Progressive publish note: flood/hint paths must not finish Agnostic seats into the
/// headgroup as sole content — see TileSession (Agnostic hints go to GPU work).
pub fn publish_seat(calibrated: CalibratedAnswer, proximate: Option<Answer>) -> Answer {
    match &calibrated.result {
        CalibratedMandelbrotResult::Agnostic { .. } => match proximate {
            Some(bias) => calibrated.guess_biased(bias),
            None => NORES_ANSWER,
        },
        CalibratedMandelbrotResult::Outside { .. }
        | CalibratedMandelbrotResult::Inside { .. } => match proximate {
            Some(bias) => calibrated.guess_biased(bias),
            None => collapse_exact(calibrated),
        },
    }
}

/// Publish a full tile: each present calibrated seat uses matching proximate if any.
/// Prefers the GPU publisher shader when a device is available (auth tile_publisher.md).
pub fn publish_tile(
    calibrated: &Tile<CalibratedAnswer>,
    proximate: Option<&Tile<Answer>>,
) -> Tile<Answer> {
    if let Some(gpu_out) = try_publish_tile_gpu(calibrated, proximate) {
        return gpu_out;
    }
    publish_tile_cpu(calibrated, proximate)
}

fn publish_tile_cpu(
    calibrated: &Tile<CalibratedAnswer>,
    proximate: Option<&Tile<Answer>>,
) -> Tile<Answer> {
    let mut out = Tile::new(calibrated.origin_seat, calibrated.magnification_pot);
    for y in 0..TILE_EDGE_LENGTH {
        for x in 0..TILE_EDGE_LENGTH {
            let local = (x, y);
            let Some(cal) = calibrated.get(local) else {
                continue;
            };
            let bias = proximate.and_then(|p| p.get(local));
            out.set(local, publish_seat(cal, bias));
        }
    }
    out
}

fn try_publish_tile_gpu(
    calibrated: &Tile<CalibratedAnswer>,
    proximate: Option<&Tile<Answer>>,
) -> Option<Tile<Answer>> {
    use bytemuck::Zeroable;
    use crate::assemblies::structs::gpu_tile::GPUCalibratedAnswer;
    use crate::assemblies::workgroup::publisher_shader::{GpuPackedAnswer, PublisherGpu};

    let gpu = PublisherGpu::shared()?;
    let n = TILE_EDGE_LENGTH * TILE_EDGE_LENGTH;
    let mut cal = vec![GPUCalibratedAnswer::EMPTY; n];
    let mut bias = vec![GpuPackedAnswer::zeroed(); n];
    let mut valid = vec![0u32; n];
    for y in 0..TILE_EDGE_LENGTH {
        for x in 0..TILE_EDGE_LENGTH {
            let idx = y * TILE_EDGE_LENGTH + x;
            if let Some(c) = calibrated.get((x, y)) {
                cal[idx] = GPUCalibratedAnswer::from_calibrated(c);
            }
            if let Some(b) = proximate.and_then(|p| p.get((x, y))) {
                bias[idx] = answer_to_packed(b);
                valid[idx] = 1;
            }
        }
    }
    let packed = gpu.publish_tile(&cal, &bias, &valid)?;
    let mut out = Tile::new(calibrated.origin_seat, calibrated.magnification_pot);
    for y in 0..TILE_EDGE_LENGTH {
        for x in 0..TILE_EDGE_LENGTH {
            let idx = y * TILE_EDGE_LENGTH + x;
            // Only emit seats that had calibrated input (matches CPU publisher).
            if calibrated.get((x, y)).is_none() {
                continue;
            }
            out.set((x, y), packed_to_answer(packed[idx]));
        }
    }
    Some(out)
}

fn answer_to_packed(answer: Answer) -> crate::assemblies::workgroup::publisher_shader::GpuPackedAnswer {
    use crate::assemblies::workgroup::publisher_shader::GpuPackedAnswer;
    match answer.result {
        MandelbrotResult::Outside {
            escape_time_r2,
            escape_z,
        } => GpuPackedAnswer {
            kind: 1,
            escape_or_period: escape_time_r2.min(u32::MAX as u64) as u32,
            min_mag_time: answer.min_magnitude_time.min(u32::MAX as u64) as u32,
            min_mag: answer.min_magnitude as f32,
            zx: escape_z.0 as f32,
            zy: escape_z.1 as f32,
            _pad0: 0,
            _pad1: 0,
        },
        MandelbrotResult::Inside { period } => GpuPackedAnswer {
            kind: 2,
            escape_or_period: period.min(u32::MAX as u64) as u32,
            min_mag_time: answer.min_magnitude_time.min(u32::MAX as u64) as u32,
            min_mag: answer.min_magnitude as f32,
            zx: 0.0,
            zy: 0.0,
            _pad0: 0,
            _pad1: 0,
        },
    }
}

fn packed_to_answer(
    packed: crate::assemblies::workgroup::publisher_shader::GpuPackedAnswer,
) -> Answer {
    let min_mag = if packed.min_mag >= 1e29 {
        f64::INFINITY
    } else {
        packed.min_mag as f64
    };
    let zx = if packed.zx <= -1e29 {
        f32::NEG_INFINITY
    } else {
        packed.zx
    };
    let zy = if packed.zy >= 1e29 {
        f32::INFINITY
    } else {
        packed.zy
    };
    if packed.kind == 2 {
        Answer {
            result: MandelbrotResult::Inside {
                period: packed.escape_or_period as u64,
            },
            min_magnitude_time: packed.min_mag_time as u64,
            min_magnitude: min_mag,
        }
    } else {
        Answer {
            result: MandelbrotResult::Outside {
                escape_time_r2: packed.escape_or_period as u64,
                escape_z: (zx as f32, zy as f32),
            },
            min_magnitude_time: packed.min_mag_time as u64,
            min_magnitude: min_mag,
        }
    }
}

fn collapse_exact(answer: CalibratedAnswer) -> Answer {
    match answer.result {
        CalibratedMandelbrotResult::Outside {
            escape_time_r2,
            escape_z,
        } => Answer {
            result: MandelbrotResult::Outside {
                escape_time_r2: escape_time_r2.lower_bound,
                escape_z: (escape_z.0.lower_bound, escape_z.1.lower_bound),
            },
            min_magnitude_time: answer.min_magnitude_time.lower_bound,
            min_magnitude: answer.min_magnitude.lower_bound,
        },
        CalibratedMandelbrotResult::Inside { period } => Answer {
            result: MandelbrotResult::Inside {
                period: period.lower_bound,
            },
            min_magnitude_time: answer.min_magnitude_time.lower_bound,
            min_magnitude: answer.min_magnitude.lower_bound,
        },
        CalibratedMandelbrotResult::Agnostic { period, .. } => {
            // Should not reach here; Agnostic handled by publish_seat.
            Answer {
                result: MandelbrotResult::Inside {
                    period: period.lower_bound,
                },
                min_magnitude_time: answer.min_magnitude_time.lower_bound,
                min_magnitude: answer.min_magnitude.lower_bound,
            }
        }
    }
}

/// Publish cadence: flat **1000/s** ceiling while incomplete; idle when complete.
/// No minimum floor (developer: flat 1000).
pub const PUBLISH_MAX_HZ: f64 = 1000.0;

/// Memory limit bump request from workgroup publisher → headgroup (raises slider floor).
// r[impl cz.int.memory-bump+1]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MemoryBump {
    pub needed_bytes: usize,
}

/// Thin live-path owner of publish cadence (+ optional bump send helper).
/// Full publisher actor extract is follow-up; screen_worker hosts this for now.
#[derive(Debug)]
pub struct LivePublisher {
    pub cadence: PublishCadence,
    pub memory_limit_bytes: usize,
}

impl LivePublisher {
    pub fn new(incomplete: bool) -> Self {
        LivePublisher {
            cadence: PublishCadence::new(incomplete),
            memory_limit_bytes: 1_000_000_000,
        }
    }

    /// Gate for the live drain/flush path: under max 1000 Hz and work is ready.
    pub fn should_publish(&mut self, now: Instant, has_work: bool) -> bool {
        self.cadence.should_publish(now, has_work)
    }

    pub fn record_publish(&mut self, now: Instant) {
        self.cadence.record_publish(now);
    }

    pub fn set_incomplete(&mut self, incomplete: bool) {
        self.cadence.set_incomplete(incomplete);
    }
}

#[derive(Debug)]
pub struct PublishCadence {
    incomplete: bool,
    window_start: Instant,
    publishes_in_window: u32,
    last_publish: Option<Instant>,
}

impl Default for PublishCadence {
    fn default() -> Self {
        Self::new(true)
    }
}

impl PublishCadence {
    pub fn new(incomplete: bool) -> Self {
        PublishCadence {
            incomplete,
            window_start: Instant::now(),
            publishes_in_window: 0,
            last_publish: None,
        }
    }

    pub fn new_at(incomplete: bool, now: Instant) -> Self {
        PublishCadence {
            incomplete,
            window_start: now,
            publishes_in_window: 0,
            last_publish: None,
        }
    }

    pub fn set_incomplete(&mut self, incomplete: bool) {
        self.incomplete = incomplete;
        if !incomplete {
            self.publishes_in_window = 0;
        }
    }

    pub fn incomplete(&self) -> bool {
        self.incomplete
    }

    fn roll_window(&mut self, now: Instant) {
        if now.duration_since(self.window_start) > Duration::from_secs(1) {
            self.window_start = now;
            self.publishes_in_window = 0;
        }
    }

    /// Whether a publish is allowed now under the max-1000 Hz cap while incomplete.
    pub fn allow_publish(&mut self, now: Instant) -> bool {
        if !self.incomplete {
            return false;
        }
        self.roll_window(now);
        if self.publishes_in_window as f64 >= PUBLISH_MAX_HZ {
            return false;
        }
        if let Some(last) = self.last_publish {
            let min_gap = Duration::from_secs_f64(1.0 / PUBLISH_MAX_HZ);
            if now.duration_since(last) < min_gap {
                return false;
            }
        }
        true
    }

    /// Live-path gate: respect max Hz; publish only when work is ready (flat 1000, no min floor).
    pub fn should_publish(&mut self, now: Instant, has_work: bool) -> bool {
        has_work && self.allow_publish(now)
    }

    pub fn record_publish(&mut self, now: Instant) {
        self.last_publish = Some(now);
        self.publishes_in_window = self.publishes_in_window.saturating_add(1);
    }

    pub fn max_publishes_per_second() -> u32 {
        PUBLISH_MAX_HZ as u32
    }
}

/// Helper for tests: exact Outside calibrated seat.
pub fn exact_outside(escape_time: u64) -> CalibratedAnswer {
    CalibratedAnswer {
        result: CalibratedMandelbrotResult::Outside {
            escape_time_r2: Range {
                lower_bound: escape_time,
                upper_bound: escape_time,
            },
            escape_z: (
                Range {
                    lower_bound: 2.0,
                    upper_bound: 2.0,
                },
                Range {
                    lower_bound: 0.0,
                    upper_bound: 0.0,
                },
            ),
        },
        min_magnitude_time: Range {
            lower_bound: 0,
            upper_bound: 0,
        },
        min_magnitude: Range {
            lower_bound: 4.0,
            upper_bound: 4.0,
        },
        highlights: CalibratedHighlights {
            in_filament: Range {
                lower_bound: false,
                upper_bound: false,
            },
            out_filament: Range {
                lower_bound: false,
                upper_bound: false,
            },
            small_time_edge: Range {
                lower_bound: false,
                upper_bound: false,
            },
            node: Range {
                lower_bound: false,
                upper_bound: false,
            },
        },
    }
}

pub fn agnostic_wide() -> CalibratedAnswer {
    CalibratedAnswer {
        result: CalibratedMandelbrotResult::Agnostic {
            period: Range {
                lower_bound: 0,
                upper_bound: 100,
            },
            escape_time_r2: Range {
                lower_bound: 1,
                upper_bound: 1_000_000,
            },
            escape_z: (
                Range {
                    lower_bound: -100.0,
                    upper_bound: 100.0,
                },
                Range {
                    lower_bound: -100.0,
                    upper_bound: 100.0,
                },
            ),
        },
        min_magnitude_time: Range {
            lower_bound: 0,
            upper_bound: 1000,
        },
        min_magnitude: Range {
            lower_bound: 0.0,
            upper_bound: 4.0,
        },
        highlights: CalibratedHighlights {
            in_filament: Range {
                lower_bound: false,
                upper_bound: true,
            },
            out_filament: Range {
                lower_bound: false,
                upper_bound: true,
            },
            small_time_edge: Range {
                lower_bound: false,
                upper_bound: true,
            },
            node: Range {
                lower_bound: false,
                upper_bound: true,
            },
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // r[verify cz.int.publisher-nores-bias+1]
    #[test]
    fn agnostic_without_proximate_is_nores() {
        let out = publish_seat(agnostic_wide(), None);
        match out.result {
            MandelbrotResult::Outside { escape_time_r2, .. } => {
                assert_eq!(escape_time_r2, 1);
            }
            MandelbrotResult::Inside { .. } => panic!("must not invent Inside from empty proximate"),
        }
        assert!(out.min_magnitude.is_infinite());
    }

    // r[verify cz.int.publisher-nores-bias+1]
    #[test]
    fn agnostic_keeps_proximate_bias_when_in_bounds() {
        let bias = Answer {
            result: MandelbrotResult::Outside {
                escape_time_r2: 50,
                escape_z: (1.0, 0.0),
            },
            min_magnitude_time: 10,
            min_magnitude: 1.0,
        };
        let out = publish_seat(agnostic_wide(), Some(bias));
        match out.result {
            MandelbrotResult::Outside { escape_time_r2, .. } => {
                assert_eq!(escape_time_r2, 50);
            }
            MandelbrotResult::Inside { .. } => panic!("expected Outside bias"),
        }
    }

    // r[verify cz.int.publisher-nores-bias+1]
    #[test]
    fn disproven_proximate_clamps_to_nearest_bound() {
        let cal = CalibratedAnswer {
            result: CalibratedMandelbrotResult::Outside {
                escape_time_r2: Range {
                    lower_bound: 10,
                    upper_bound: 20,
                },
                escape_z: (
                    Range {
                        lower_bound: 2.0,
                        upper_bound: 2.0,
                    },
                    Range {
                        lower_bound: 0.0,
                        upper_bound: 0.0,
                    },
                ),
            },
            min_magnitude_time: Range {
                lower_bound: 0,
                upper_bound: 0,
            },
            min_magnitude: Range {
                lower_bound: 4.0,
                upper_bound: 4.0,
            },
            highlights: exact_outside(1).highlights,
        };
        let bias = Answer {
            result: MandelbrotResult::Outside {
                escape_time_r2: 100,
                escape_z: (2.0, 0.0),
            },
            min_magnitude_time: 0,
            min_magnitude: 4.0,
        };
        let out = publish_seat(cal, Some(bias));
        match out.result {
            MandelbrotResult::Outside { escape_time_r2, .. } => {
                assert_eq!(escape_time_r2, 20, "disproven high bias clamps to upper");
            }
            MandelbrotResult::Inside { .. } => panic!("expected Outside"),
        }
    }

    // r[verify cz.int.publish-cadence+1]
    #[test]
    fn cadence_idle_when_complete() {
        let mut c = PublishCadence::new(false);
        assert!(!c.allow_publish(Instant::now()));
    }

    // r[verify cz.int.publish-cadence+1]
    #[test]
    fn cadence_allows_while_incomplete() {
        let mut c = PublishCadence::new(true);
        let t0 = Instant::now();
        assert!(c.allow_publish(t0));
        c.record_publish(t0);
    }

    // r[verify cz.int.publish-cadence+1]
    #[test]
    fn cadence_caps_at_max_hz() {
        let t0 = Instant::now();
        let mut c = PublishCadence::new_at(true, t0);
        let gap = Duration::from_millis(1);
        let max = PublishCadence::max_publishes_per_second();
        for i in 0..max {
            let t = t0 + gap * i;
            assert!(c.allow_publish(t), "i={i}");
            c.record_publish(t);
        }
        assert!(!c.allow_publish(t0 + gap * max));
    }

    // r[verify cz.int.publish-cadence+1]
    // Flat 1000: idle without work must not force publish.
    #[test]
    fn cadence_does_not_force_without_work() {
        let t0 = Instant::now();
        let mut c = PublishCadence::new_at(true, t0);
        assert!(
            !c.should_publish(t0, false),
            "no work → no publish even when incomplete"
        );
        assert!(
            c.should_publish(t0, true),
            "work ready under max"
        );
        c.record_publish(t0);
        assert!(
            !c.should_publish(t0 + Duration::from_millis(1), false),
            "still no work → still no publish"
        );
        assert!(
            c.should_publish(t0 + Duration::from_millis(1), true),
            "work ready under max after gap"
        );
        let long_idle = t0 + Duration::from_secs(2);
        assert!(
            !c.should_publish(long_idle, false),
            "long idle without work must not force publish (flat 1000)"
        );
    }

    // r[verify cz.int.publish-cadence+1]
    #[test]
    fn cadence_max_hz_constant() {
        assert_eq!(PublishCadence::max_publishes_per_second(), 1000);
    }
}

/// Live publisher actor: sits between the uploader and the headgroup.
///
/// Receives gpu-native (or CPU-fallback) tile handles, applies publisher policy
/// (cadence bookkeeping; collapse stays in place until the compute shader lands),
/// and owns the memory-bump channel to the window (tile_publisher.md).
pub struct PublisherActorState {
    unsent: Option<crate::assemblies::structs::GpuTileHandle>
    , unsent_bump: Option<MemoryBump>
    , live: LivePublisher
}

pub async fn run_actor(
    actor: steady_state::SteadyActorShadow
    , tiles_in: steady_state::SteadyRx<crate::assemblies::structs::GpuTileHandle>
    , tiles_out: steady_state::SteadyTx<crate::assemblies::structs::GpuTileHandle>
    , bump_in: steady_state::SteadyRx<MemoryBump>
    , bump_out: steady_state::SteadyTx<MemoryBump>
    , state: steady_state::SteadyState<PublisherActorState>
) -> Result<(), Box<dyn std::error::Error>> {
    use steady_state::*;
    internal_actor(
        actor.into_spotlight([&tiles_in, &bump_in], [&tiles_out, &bump_out])
        , tiles_in
        , tiles_out
        , bump_in
        , bump_out
        , state
    ).await
}

async fn internal_actor<A: steady_state::SteadyActor>(
    mut actor: A
    , tiles_in: steady_state::SteadyRx<crate::assemblies::structs::GpuTileHandle>
    , tiles_out: steady_state::SteadyTx<crate::assemblies::structs::GpuTileHandle>
    , bump_in: steady_state::SteadyRx<MemoryBump>
    , bump_out: steady_state::SteadyTx<MemoryBump>
    , state: steady_state::SteadyState<PublisherActorState>
) -> Result<(), Box<dyn std::error::Error>> {
    use steady_state::*;
    use crate::assemblies::structs::GpuTileHandle;
    let mut tiles_in = tiles_in.lock().await;
    let mut tiles_out = tiles_out.lock().await;
    let mut bump_in = bump_in.lock().await;
    let mut bump_out = bump_out.lock().await;
    let mut state = state.lock(|| PublisherActorState {
        unsent: None
        , unsent_bump: None
        , live: LivePublisher::new(true)
    }).await;

    let max_sleep = Duration::from_millis(2);

    while actor.is_running(|| i!(tiles_out.mark_closed() && bump_out.mark_closed())) {
        if actor.avail_units(&mut tiles_in) == 0
            && actor.avail_units(&mut bump_in) == 0
            && state.unsent.is_none()
            && state.unsent_bump.is_none()
        {
            await_for_any!(
                actor.wait_periodic(max_sleep)
                , actor.wait_avail(&mut tiles_in, 1)
                , actor.wait_avail(&mut bump_in, 1)
            );
        }

        if let Some(bump) = state.unsent_bump.take() {
            match actor.try_send(&mut bump_out, bump) {
                SendOutcome::Success => {}
                SendOutcome::Blocked(b)
                | SendOutcome::Timeout(b)
                | SendOutcome::Closed(b) => {
                    state.unsent_bump = Some(b);
                }
            }
        }

        while state.unsent_bump.is_none() && actor.avail_units(&mut bump_in) > 0 {
            let Some(bump) = actor.try_take(&mut bump_in) else { break };
            // Publisher owns the bump channel to the headgroup: raise our own
            // floor too, then forward so the window slider can follow.
            state.live.memory_limit_bytes = state.live.memory_limit_bytes.max(bump.needed_bytes);
            match actor.try_send(&mut bump_out, bump) {
                SendOutcome::Success => {}
                SendOutcome::Blocked(b)
                | SendOutcome::Timeout(b)
                | SendOutcome::Closed(b) => {
                    state.unsent_bump = Some(b);
                    break;
                }
            }
        }

        if let Some(tile) = state.unsent.take() {
            match actor.try_send(&mut tiles_out, tile) {
                SendOutcome::Success => {
                    state.live.record_publish(Instant::now());
                }
                SendOutcome::Blocked(t)
                | SendOutcome::Timeout(t)
                | SendOutcome::Closed(t) => {
                    state.unsent = Some(t);
                    continue;
                }
            }
        }

        while state.unsent.is_none() && actor.avail_units(&mut tiles_in) > 0 {
            let Some(handle) = actor.try_take(&mut tiles_in) else { break };
            // Handles arriving here are already collapsed to Answer for the
            // headgroup; the publisher shader will take over this step later.
            let _: &GpuTileHandle = &handle;
            match actor.try_send(&mut tiles_out, handle) {
                SendOutcome::Success => {
                    state.live.record_publish(Instant::now());
                }
                SendOutcome::Blocked(t)
                | SendOutcome::Timeout(t)
                | SendOutcome::Closed(t) => {
                    state.unsent = Some(t);
                    break;
                }
            }
        }
    }

    info!("Tile publisher shutting down.");
    Ok(())
}
