//! Perturbation delta kernel — the sole production numerical path (f64 seats).
//!
//! Inner recurrence uses the compute gear ladder (f64 → scaled-f64 → FloatExp).
//! Depth tests use `perturb_floatexp` (`SeatKernel<FloatExp>`).

pub mod floatexp_host {
    pub use crate::assemblies::workgroup::screen_worker::perturb_floatexp::{
        FloatExpPerturbationKernel, PerturbationKernel,
    };
}

use std::sync::OnceLock;

use crate::delta_gear::{
    f64_step, floatexp_from_f64_pair, gear_for_delta, narrow_z_ref, scaled_f64_step,
    scaled_scale_from_dz, ComputeGear,
};
use crate::floatexp::{ComplexFloatExp, FloatExp};
use crate::reference::ReferenceOrbit;

use super::workshift::{
    direct_completion, ensure_started, update_point_results, bailout_point, iterate, BoutCap,
    CompletedPoint, DeltaState, Point, SeatKernel, WorkContext,
};

/// The only production kernel. Always runs delta iteration.
// r[impl cz.depth.delta-kernel+1]
// r[impl cz.perf.one-kernel-path+1]
// r[impl cz.ref.zero-orbit-same-path+1]
// r[impl cz.depth.compute-gear+1]
#[derive(Clone, Copy, Debug, Default)]
pub struct PerturbationKernel;

fn zero_orbit() -> &'static ReferenceOrbit {
    static ZERO: OnceLock<ReferenceOrbit> = OnceLock::new();
    ZERO.get_or_init(ReferenceOrbit::zero_orbit)
}

fn abs_plane_f64(c: (f64, f64), anchor: &(crate::utils::IntExp, crate::utils::IntExp)) -> (f64, f64) {
    (
        f64::from(anchor.0.clone()) + c.0,
        f64::from(anchor.1.clone()) + c.1,
    )
}

fn to_delta_c_f64(c: (f64, f64)) -> ComplexFloatExp {
    ComplexFloatExp::new(FloatExp::from(c.0), FloatExp::from(c.1))
}

fn reference_c_floatexp(orbit: &ReferenceOrbit) -> ComplexFloatExp {
    ComplexFloatExp::new(
        FloatExp::from_rug(&orbit.c.0),
        FloatExp::from_rug(&orbit.c.1),
    )
}

fn active_orbit<'a>(
    point: &Point<f64>,
    published: Option<&'a ReferenceOrbit>,
) -> &'a ReferenceOrbit {
    if point.direct_only {
        zero_orbit()
    } else if let Some(r) = published {
        if r.escaped {
            zero_orbit()
        } else {
            r
        }
    } else {
        zero_orbit()
    }
}

fn active_generation(
    point: &Point<f64>,
    published: Option<&crate::assemblies::workgroup::reference_worker::PublishedReference>,
) -> u64 {
    if point.direct_only {
        0
    } else {
        published.map(|r| r.generation).unwrap_or(0)
    }
}

fn published_generation(
    published: Option<&crate::assemblies::workgroup::reference_worker::PublishedReference>,
) -> u64 {
    published.map(|r| r.generation).unwrap_or(0)
}

fn maybe_clear_zero_bind(
    point: &mut Point<f64>,
    published: Option<&crate::assemblies::workgroup::reference_worker::PublishedReference>,
) {
    let gen = published_generation(published);
    if point.direct_only && gen != point.bound_zero_generation {
        point.direct_only = false;
        point.delta = None;
        point.initialized = false;
    }
}

fn reset_for_glitch(point: &mut Point<f64>, against_generation: u64) {
    point.direct_only = true;
    point.bound_zero_generation = against_generation;
    point.delta = None;
    point.initialized = false;
    point.iterations = 0;
    point.escapes = false;
    point.repeats = false;
    point.delivered = false;
}

fn rebind_to_zero_continuing(
    point: &mut Point<f64>,
    delta: &mut DeltaState,
    against_generation: u64,
) {
    point.direct_only = true;
    point.bound_zero_generation = against_generation;
    delta.dc = delta.abs_c;
    delta.dz = floatexp_from_f64_pair(point.z);
    delta.generation = 0;
    delta.gear = gear_for_delta(delta.dc, delta.dz);
    delta.scale = scaled_scale_from_dz(delta.dz);
}

#[inline(always)]
fn sync_point_from_delta_fe(
    point: &mut Point<f64>,
    z_ref: ComplexFloatExp,
    dz: ComplexFloatExp,
    dd: ComplexFloatExp,
) {
    let z = z_ref + dz;
    point.z = (z.re.to_f64(), z.im.to_f64());
    point.dc = (dd.re.to_f64(), dd.im.to_f64());
    update_point_results(point);
}

#[inline(always)]
fn sync_point_from_f64_locals(
    point: &mut Point<f64>,
    z_ref: (f64, f64),
    dz: (f64, f64),
    dd: (f64, f64),
) {
    point.z = (z_ref.0 + dz.0, z_ref.1 + dz.1);
    point.dc = dd;
    update_point_results(point);
}

#[inline(always)]
fn near_fe(z_next: ComplexFloatExp, checkpoint: ComplexFloatExp, epsilon: f64) -> bool {
    let eps = FloatExp::from(epsilon);
    (z_next.re - checkpoint.re).abs() <= eps && (z_next.im - checkpoint.im).abs() <= eps
}

fn init_delta(
    point: &mut Point<f64>,
    orbit: &ReferenceOrbit,
    generation: u64,
    abs_c: (f64, f64),
) {
    if std::ptr::eq(orbit, zero_orbit()) {
        init_delta_zero_orbit_f64(point, generation, abs_c);
        return;
    }
    let abs = to_delta_c_f64(abs_c);
    let dc = abs - reference_c_floatexp(orbit);
    let dz = dc;
    let dd = ComplexFloatExp::new(FloatExp::ONE, FloatExp::ZERO);
    let Some(z_ref) = orbit.get(1) else {
        point.delta = None;
        return;
    };
    let gear = gear_for_delta(dc, dz);
    let scale = scaled_scale_from_dz(dz);
    point.delta = Some(DeltaState {
        dz,
        checkpoint: ComplexFloatExp::ZERO,
        checkpoint_n: 0,
        dc,
        abs_c: abs,
        dd,
        generation,
        gear,
        scale,
    });
    point.iterations = 0;
    point.escapes = false;
    point.repeats = false;
    point.period = 0;
    point.loop_detection_point = ((point.c.0, point.c.1), 0);
    point.smallness_squared = f64::MAX;
    point.small_time = 0;
    sync_point_from_delta_fe(point, z_ref, dz, dd);
}

/// Zero-orbit floor: skip gear scan / orbit lookup — home f64 is always F64 here.
fn init_delta_zero_orbit_f64(point: &mut Point<f64>, generation: u64, abs_c: (f64, f64)) {
    let fe_c = floatexp_from_f64_pair(abs_c);
    point.delta = Some(DeltaState {
        dz: fe_c,
        checkpoint: ComplexFloatExp::ZERO,
        checkpoint_n: 0,
        dc: fe_c,
        abs_c: fe_c,
        dd: ComplexFloatExp::new(FloatExp::ONE, FloatExp::ZERO),
        generation,
        gear: ComputeGear::F64,
        scale: FloatExp::ONE,
    });
    point.iterations = 0;
    point.escapes = false;
    point.repeats = false;
    point.period = 0;
    point.loop_detection_point = (abs_c, 0);
    point.smallness_squared = f64::MAX;
    point.small_time = 0;
    point.z = abs_c;
    point.dc = (1.0, 0.0);
    update_point_results(point);
}

fn apply_series_skip(
    point: &mut Point<f64>,
    published: Option<&crate::assemblies::workgroup::reference_worker::PublishedReference>,
) {
    let Some(pub_ref) = published else {
        return;
    };
    if point.direct_only || pub_ref.orbit.escaped {
        return;
    }
    let Some(series) = pub_ref.series.as_ref() else {
        return;
    };
    let Some(delta) = point.delta.as_mut() else {
        return;
    };
    if point.iterations > 0 {
        return;
    }
    let skip = series.safe_skip(delta.dc, pub_ref.orbit.iterates.len().saturating_sub(1));
    if skip <= 1 {
        return;
    }
    let Some(dz) = series.evaluate(skip, delta.dc) else {
        return;
    };
    let dd = delta.dd;
    delta.dz = dz;
    delta.gear = gear_for_delta(delta.dc, dz);
    delta.scale = scaled_scale_from_dz(dz);
    point.iterations = skip.saturating_sub(1) as u32;
    let z_ref = pub_ref
        .orbit
        .get(point.iterations.saturating_add(1))
        .unwrap_or(ComplexFloatExp::ZERO);
    sync_point_from_delta_fe(point, z_ref, dz, dd);
}

fn fe_pair(z: ComplexFloatExp) -> (f64, f64) {
    (z.re.to_f64(), z.im.to_f64())
}

fn fe_iterate_step(
    point: &mut Point<f64>,
    orbit: &ReferenceOrbit,
    delta: &mut DeltaState,
    is_zero_ref: bool,
    z_ref: ComplexFloatExp,
    r_squared: f64,
    epsilon: f64,
) -> StepOutcome {
    let z = z_ref + delta.dz;
    let z_norm_sq = z.norm_squared();
    let z_ref_norm_sq = if is_zero_ref {
        FloatExp::ZERO
    } else {
        z_ref.norm_squared()
    };
    let glitch_factor = FloatExp::from(1.0e-6);
    let r_sq = FloatExp::from(r_squared);
    let four = FloatExp::from(4.0);
    let two = ComplexFloatExp::new(FloatExp::TWO, FloatExp::ZERO);
    let one = ComplexFloatExp::new(FloatExp::ONE, FloatExp::ZERO);

    if !is_zero_ref && delta.dz != ComplexFloatExp::ZERO && z_ref_norm_sq == four {
        let correction = FloatExp::TWO * (z_ref.re * delta.dz.re + z_ref.im * delta.dz.im)
            + delta.dz.norm_squared();
        if correction > FloatExp::ZERO {
            return StepOutcome::Escaped;
        }
        if correction == FloatExp::ZERO {
            return StepOutcome::Glitch;
        }
    }
    if z_norm_sq > r_sq {
        return StepOutcome::Escaped;
    }
    if !is_zero_ref
        && point.iterations > 0
        && z_norm_sq < z_ref_norm_sq * glitch_factor
    {
        return StepOutcome::Glitch;
    }
    let rad = z_norm_sq.to_f64();
    if rad < point.smallness_squared {
        point.smallness_squared = rad;
        point.small_time = point.iterations;
    }
    delta.dd = z * delta.dd * two + one;
    delta.dz = z_ref * delta.dz * two + delta.dz * delta.dz + delta.dc;
    delta.gear = ComputeGear::FloatExp;
    point.iterations = point.iterations.saturating_add(1);
    let z_next_ref = if is_zero_ref {
        ComplexFloatExp::ZERO
    } else {
        orbit
            .get(point.iterations.saturating_add(1))
            .unwrap_or(ComplexFloatExp::ZERO)
    };
    let z_next = z_next_ref + delta.dz;
    if near_fe(z_next, delta.checkpoint, epsilon) {
        return StepOutcome::Repeats;
    }
    if point.iterations >= delta.checkpoint_n.saturating_mul(2).max(1) {
        delta.checkpoint = z_next;
        delta.checkpoint_n = point.iterations;
    }
    StepOutcome::Continue
}

enum StepOutcome {
    Continue,
    Escaped,
    Repeats,
    Glitch,
    Exhausted,
}

/// Hoisted f64/scaled-f64 working state for a bout — avoids per-step FloatExp round-trips.
enum BoutWorking {
    FloatExp,
    F64 {
        dz: (f64, f64),
        dc: (f64, f64),
        dd: (f64, f64),
        checkpoint: (f64, f64),
    },
    ScaledF64 {
        w: (f64, f64),
        d: (f64, f64),
        dd: (f64, f64),
        scale: FloatExp,
        checkpoint: (f64, f64),
    },
}

impl BoutWorking {
    fn from_delta(delta: &DeltaState) -> Self {
        match delta.gear {
            ComputeGear::F64 => BoutWorking::F64 {
                dz: fe_pair(delta.dz),
                dc: fe_pair(delta.dc),
                dd: fe_pair(delta.dd),
                checkpoint: fe_pair(delta.checkpoint),
            },
            ComputeGear::ScaledF64 => {
                let s = delta.scale.to_f64();
                if !s.is_finite() || s == 0.0 {
                    return BoutWorking::FloatExp;
                }
                BoutWorking::ScaledF64 {
                    w: (delta.dz.re.to_f64() / s, delta.dz.im.to_f64() / s),
                    d: (delta.dc.re.to_f64() / s, delta.dc.im.to_f64() / s),
                    dd: fe_pair(delta.dd),
                    scale: delta.scale,
                    checkpoint: fe_pair(delta.checkpoint),
                }
            }
            _ => BoutWorking::FloatExp,
        }
    }

    fn flush_to(&self, delta: &mut DeltaState) {
        match self {
            BoutWorking::F64 {
                dz,
                dd,
                checkpoint,
                ..
            } => {
                delta.dz = floatexp_from_f64_pair(*dz);
                delta.dd = floatexp_from_f64_pair(*dd);
                delta.checkpoint = floatexp_from_f64_pair(*checkpoint);
            }
            BoutWorking::ScaledF64 {
                w,
                dd,
                scale,
                checkpoint,
                ..
            } => {
                let s = scale.to_f64();
                let dz = (w.0 * s, w.1 * s);
                delta.dz = floatexp_from_f64_pair(dz);
                delta.dd = floatexp_from_f64_pair(*dd);
                delta.checkpoint = floatexp_from_f64_pair(*checkpoint);
                delta.scale = *scale;
            }
            BoutWorking::FloatExp => {}
        }
    }
}

#[inline(always)]
fn sync_point_after_bout(
    point: &mut Point<f64>,
    working: &BoutWorking,
    delta: &DeltaState,
    z_ref_fe: ComplexFloatExp,
    is_zero_ref: bool,
) {
    match working {
        BoutWorking::F64 { dz, dd, .. } => {
            let z_ref = if is_zero_ref {
                (0.0, 0.0)
            } else {
                narrow_z_ref(z_ref_fe).unwrap_or(fe_pair(z_ref_fe))
            };
            sync_point_from_f64_locals(point, z_ref, *dz, *dd);
        }
        BoutWorking::ScaledF64 { w, dd, scale, .. } => {
            let s = scale.to_f64();
            let dz = (w.0 * s, w.1 * s);
            let z_ref = if is_zero_ref {
                (0.0, 0.0)
            } else {
                narrow_z_ref(z_ref_fe).unwrap_or(fe_pair(z_ref_fe))
            };
            sync_point_from_f64_locals(point, z_ref, dz, *dd);
        }
        BoutWorking::FloatExp => {
            sync_point_from_delta_fe(point, z_ref_fe, delta.dz, delta.dd);
        }
    }
}

#[inline(always)]
fn f64_period_check(
    z_next: (f64, f64),
    checkpoint: (f64, f64),
    epsilon: f64,
    iterations: u32,
    checkpoint_n: u32,
) -> (bool, (f64, f64), u32) {
    if (z_next.0 - checkpoint.0).abs() <= epsilon
        && (z_next.1 - checkpoint.1).abs() <= epsilon
    {
        return (true, checkpoint, checkpoint_n);
    }
    if iterations >= checkpoint_n.saturating_mul(2).max(1) {
        (false, z_next, iterations)
    } else {
        (false, checkpoint, checkpoint_n)
    }
}

fn promote_to_fe_step(
    point: &mut Point<f64>,
    orbit: &ReferenceOrbit,
    delta: &mut DeltaState,
    working: BoutWorking,
    is_zero_ref: bool,
    z_ref_fe: ComplexFloatExp,
    r_squared: f64,
    epsilon: f64,
) -> (StepOutcome, BoutWorking) {
    working.flush_to(delta);
    delta.gear = ComputeGear::FloatExp;
    let out = fe_iterate_step(
        point,
        orbit,
        delta,
        is_zero_ref,
        z_ref_fe,
        r_squared,
        epsilon,
    );
    (out, BoutWorking::FloatExp)
}

fn f64_bout_step(
    point: &mut Point<f64>,
    orbit: &ReferenceOrbit,
    delta: &mut DeltaState,
    z_ref_fe: ComplexFloatExp,
    r_squared: f64,
    epsilon: f64,
    is_zero_ref: bool,
    dz: (f64, f64),
    dc: (f64, f64),
    dd: (f64, f64),
    checkpoint: (f64, f64),
) -> (StepOutcome, BoutWorking) {
    let working = BoutWorking::F64 {
        dz,
        dc,
        dd,
        checkpoint,
    };
    let Some(z_ref) = narrow_z_ref(z_ref_fe) else {
        return promote_to_fe_step(
            point,
            orbit,
            delta,
            working,
            is_zero_ref,
            z_ref_fe,
            r_squared,
            epsilon,
        );
    };
    let z = (z_ref.0 + dz.0, z_ref.1 + dz.1);
    let z_norm = z.0 * z.0 + z.1 * z.1;
    let z_ref_norm = z_ref.0 * z_ref.0 + z_ref.1 * z_ref.1;
    if z_norm > r_squared {
        return (StepOutcome::Escaped, working);
    }
    if !is_zero_ref && point.iterations > 0 && z_norm < z_ref_norm * 1e-6 {
        return (StepOutcome::Glitch, working);
    }
    if z_norm < point.smallness_squared {
        point.smallness_squared = z_norm;
        point.small_time = point.iterations;
    }
    let (dz_next, dd_next, next_gear) = f64_step(z_ref, dz, dc, dd, is_zero_ref);
    // Orbit state is dz; dd is derivative coloring. Promote only when dz cannot
    // continue in f64 — a non-finite dd must not drag the seat onto FloatExp.
    if !dz_next.0.is_finite() || !dz_next.1.is_finite() {
        return promote_to_fe_step(
            point,
            orbit,
            delta,
            working,
            is_zero_ref,
            z_ref_fe,
            r_squared,
            epsilon,
        );
    }
    let dd_keep = if dd_next.0.is_finite() && dd_next.1.is_finite() {
        dd_next
    } else {
        dd // freeze last finite derivative; do not invent a value
    };
    let mut next_working = if next_gear == ComputeGear::ScaledF64 {
        let scale = scaled_scale_from_dz(floatexp_from_f64_pair(dz_next));
        delta.gear = ComputeGear::ScaledF64;
        let s = scale.to_f64();
        BoutWorking::ScaledF64 {
            w: (dz_next.0 / s, dz_next.1 / s),
            d: (dc.0 / s, dc.1 / s),
            dd: dd_keep,
            scale,
            checkpoint,
        }
    } else {
        BoutWorking::F64 {
            dz: dz_next,
            dc,
            dd: dd_keep,
            checkpoint,
        }
    };
    point.iterations = point.iterations.saturating_add(1);
    let z_next_ref = if is_zero_ref {
        (0.0, 0.0)
    } else {
        orbit
            .get(point.iterations.saturating_add(1))
            .and_then(narrow_z_ref)
            .unwrap_or((0.0, 0.0))
    };
    let z_next = (z_next_ref.0 + dz_next.0, z_next_ref.1 + dz_next.1);
    let (repeats, cp, cp_n) = f64_period_check(
        z_next,
        checkpoint,
        epsilon,
        point.iterations,
        delta.checkpoint_n,
    );
    delta.checkpoint_n = cp_n;
    match &mut next_working {
        BoutWorking::F64 {
            checkpoint: cp_out, ..
        }
        | BoutWorking::ScaledF64 {
            checkpoint: cp_out, ..
        } => *cp_out = cp,
        BoutWorking::FloatExp => {}
    }
    let out = if repeats {
        StepOutcome::Repeats
    } else {
        StepOutcome::Continue
    };
    (out, next_working)
}

fn scaled_bout_step(
    point: &mut Point<f64>,
    orbit: &ReferenceOrbit,
    delta: &mut DeltaState,
    z_ref_fe: ComplexFloatExp,
    r_squared: f64,
    epsilon: f64,
    is_zero_ref: bool,
    w: (f64, f64),
    d: (f64, f64),
    dd: (f64, f64),
    scale: FloatExp,
    checkpoint: (f64, f64),
) -> (StepOutcome, BoutWorking) {
    let working = BoutWorking::ScaledF64 {
        w,
        d,
        dd,
        scale,
        checkpoint,
    };
    let s = scale.to_f64();
    if !s.is_finite() || s == 0.0 {
        return promote_to_fe_step(
            point,
            orbit,
            delta,
            working,
            is_zero_ref,
            z_ref_fe,
            r_squared,
            epsilon,
        );
    }
    let Some(z_ref) = narrow_z_ref(z_ref_fe) else {
        return promote_to_fe_step(
            point,
            orbit,
            delta,
            working,
            is_zero_ref,
            z_ref_fe,
            r_squared,
            epsilon,
        );
    };
    let dz = (w.0 * s, w.1 * s);
    let z = (z_ref.0 + dz.0, z_ref.1 + dz.1);
    let z_norm = z.0 * z.0 + z.1 * z.1;
    let z_ref_norm = z_ref.0 * z_ref.0 + z_ref.1 * z_ref.1;
    if z_norm > r_squared {
        return (StepOutcome::Escaped, working);
    }
    if !is_zero_ref && point.iterations > 0 && z_norm < z_ref_norm * 1e-6 {
        return (StepOutcome::Glitch, working);
    }
    if z_norm < point.smallness_squared {
        point.smallness_squared = z_norm;
        point.small_time = point.iterations;
    }
    let (w_next, scale_next, next_gear) = scaled_f64_step(z_ref, w, d, scale, is_zero_ref);
    if next_gear == ComputeGear::FloatExp {
        return promote_to_fe_step(
            point,
            orbit,
            delta,
            working,
            is_zero_ref,
            z_ref_fe,
            r_squared,
            epsilon,
        );
    }
    let s_next = scale_next.to_f64();
    let dz_next = (w_next.0 * s_next, w_next.1 * s_next);
    if !dz_next.0.is_finite() || !dz_next.1.is_finite() || !s_next.is_finite() {
        return promote_to_fe_step(
            point,
            orbit,
            delta,
            working,
            is_zero_ref,
            z_ref_fe,
            r_squared,
            epsilon,
        );
    }
    let dd_next = (
        2.0 * (z.0 * dd.0 - z.1 * dd.1) + 1.0,
        2.0 * (z.0 * dd.1 + z.1 * dd.0),
    );
    let dd_keep = if dd_next.0.is_finite() && dd_next.1.is_finite() {
        dd_next
    } else {
        dd
    };
    point.iterations = point.iterations.saturating_add(1);
    let z_next_ref = if is_zero_ref {
        (0.0, 0.0)
    } else {
        orbit
            .get(point.iterations.saturating_add(1))
            .and_then(narrow_z_ref)
            .unwrap_or((0.0, 0.0))
    };
    let z_next = (z_next_ref.0 + dz_next.0, z_next_ref.1 + dz_next.1);
    let (repeats, cp, cp_n) = f64_period_check(
        z_next,
        checkpoint,
        epsilon,
        point.iterations,
        delta.checkpoint_n,
    );
    delta.checkpoint_n = cp_n;
    let next_working = BoutWorking::ScaledF64 {
        w: w_next,
        d,
        dd: dd_keep,
        scale: scale_next,
        checkpoint: cp,
    };
    let out = if repeats {
        StepOutcome::Repeats
    } else {
        StepOutcome::Continue
    };
    (out, next_working)
}

#[inline(always)]
fn flush_checkpoint_only(delta: &mut DeltaState, checkpoint: (f64, f64), checkpoint_n: u32) {
    delta.checkpoint_n = checkpoint_n;
    if checkpoint.0.is_finite() && checkpoint.1.is_finite() {
        delta.checkpoint = floatexp_from_f64_pair(checkpoint);
    }
}

#[inline(always)]
fn flush_delta_from_point(delta: &mut DeltaState, point: &Point<f64>, checkpoint: (f64, f64)) {
    if point.z.0.is_finite() && point.z.1.is_finite() {
        delta.dz = floatexp_from_f64_pair(point.z);
    }
    if point.dc.0.is_finite() && point.dc.1.is_finite() {
        delta.dd = floatexp_from_f64_pair(point.dc);
    }
    flush_checkpoint_only(delta, checkpoint, delta.checkpoint_n);
}

/// Zero-orbit F64 bout — DirectKernel iterate + perturbation checkpoint semantics.
/// point.z/dc authoritative; delta touched only for checkpoint (continue) or full flush (terminal).
fn zero_orbit_f64_iterate_bout(
    point: &mut Point<f64>,
    mut checkpoint: (f64, f64),
    mut checkpoint_n: u32,
    r_squared: f64,
    epsilon: f64,
    cap: BoutCap,
) -> (bool, (f64, f64), u32) {
    for _ in 0..cap.get() {
        update_point_results(point);
        if bailout_point(point, r_squared) {
            point.escapes = true;
            return (true, checkpoint, checkpoint_n);
        }
        let rad = point.real_squared + point.imag_squared;
        if rad < point.smallness_squared {
            point.smallness_squared = rad;
            point.small_time = point.iterations;
        }
        iterate(point);
        if (point.z.0 - checkpoint.0).abs() <= epsilon
            && (point.z.1 - checkpoint.1).abs() <= epsilon
        {
            point.repeats = true;
            point.period = point.iterations.saturating_sub(checkpoint_n);
            return (true, checkpoint, checkpoint_n);
        }
        if point.iterations >= checkpoint_n.saturating_mul(2).max(1) {
            checkpoint = point.z;
            checkpoint_n = point.iterations;
        }
    }
    (false, checkpoint, checkpoint_n)
}

impl SeatKernel<f64> for PerturbationKernel {
    fn start_seat(&self, context: &mut WorkContext<f64>, pos: (i32, i32)) {
        ensure_started(context, pos);
        let index = crate::utils::index_from_pos(&pos, context.res.0);
        maybe_clear_zero_bind(
            &mut context.points[index],
            context.latest_reference.as_deref(),
        );
        let generation = active_generation(
            &context.points[index],
            context.latest_reference.as_deref(),
        );
        let orbit = active_orbit(
            &context.points[index],
            context.latest_reference.as_ref().map(|r| &r.orbit),
        );
        let needs_restart = match &context.points[index].delta {
            None => true,
            Some(d) => d.generation != generation,
        };
        if needs_restart {
            let abs_c = if context.coords_are_relative {
                abs_plane_f64(context.points[index].c, &context.coord_anchor)
            } else {
                context.points[index].c
            };
            init_delta(&mut context.points[index], orbit, generation, abs_c);
            apply_series_skip(
                &mut context.points[index],
                context.latest_reference.as_deref(),
            );
        }
        // HUD gear aggregate is refreshed once per workshift — never per seat.
        // Scanning all seats here is O(n) per bout and collapses home fill.
    }

    fn iterate_bout(
        &self,
        point: &mut Point<f64>,
        reference: Option<&ReferenceOrbit>,
        r_squared: f64,
        epsilon: f64,
        cap: BoutCap,
    ) {
        if point.repeats || point.escapes {
            return;
        }
        let orbit = active_orbit(point, reference);
        let is_zero_ref = std::ptr::eq(orbit, zero_orbit());

        // Home / zero-orbit floor: same inner loop as DirectKernel; no delta take.
        if is_zero_ref && point.delta.as_ref().is_some_and(|d| d.gear == ComputeGear::F64) {
            let (checkpoint, checkpoint_n) = {
                let d = point.delta.as_ref().unwrap();
                (fe_pair(d.checkpoint), d.checkpoint_n)
            };
            let (terminal, checkpoint, checkpoint_n) =
                zero_orbit_f64_iterate_bout(point, checkpoint, checkpoint_n, r_squared, epsilon, cap);
            if terminal {
                if let Some(mut delta) = point.delta.take() {
                    flush_delta_from_point(&mut delta, point, checkpoint);
                    point.delta = Some(delta);
                }
            } else if let Some(delta) = point.delta.as_mut() {
                flush_checkpoint_only(delta, checkpoint, checkpoint_n);
            }
            return;
        }

        let mut orbit = orbit;
        let Some(mut delta) = point.delta.take() else {
            return;
        };
        let against_generation = if point.direct_only {
            point.bound_zero_generation
        } else {
            delta.generation
        };
        let mut is_zero_ref = is_zero_ref;
        let mut working = BoutWorking::from_delta(&delta);

        for _ in 0..cap.get() {
            let n = point.iterations.saturating_add(1);
            let z_ref_fe = if is_zero_ref {
                ComplexFloatExp::ZERO
            } else {
                match orbit.get(n) {
                    Some(z) => z,
                    None => {
                        working.flush_to(&mut delta);
                        let stamp = delta.generation;
                        rebind_to_zero_continuing(point, &mut delta, stamp);
                        working = BoutWorking::from_delta(&delta);
                        orbit = zero_orbit();
                        is_zero_ref = true;
                        ComplexFloatExp::ZERO
                    }
                }
            };

            let outcome = match std::mem::replace(&mut working, BoutWorking::FloatExp) {
                BoutWorking::FloatExp => fe_iterate_step(
                    point,
                    orbit,
                    &mut delta,
                    is_zero_ref,
                    z_ref_fe,
                    r_squared,
                    epsilon,
                ),
                BoutWorking::F64 {
                    mut dz,
                    dc,
                    mut dd,
                    mut checkpoint,
                } => {
                    let (out, next) = f64_bout_step(
                        point,
                        orbit,
                        &mut delta,
                        z_ref_fe,
                        r_squared,
                        epsilon,
                        is_zero_ref,
                        dz,
                        dc,
                        dd,
                        checkpoint,
                    );
                    working = next;
                    out
                }
                BoutWorking::ScaledF64 {
                    mut w,
                    d,
                    mut dd,
                    mut scale,
                    mut checkpoint,
                } => {
                    let (out, next) = scaled_bout_step(
                        point,
                        orbit,
                        &mut delta,
                        z_ref_fe,
                        r_squared,
                        epsilon,
                        is_zero_ref,
                        w,
                        d,
                        dd,
                        scale,
                        checkpoint,
                    );
                    working = next;
                    out
                }
            };

            match outcome {
                StepOutcome::Escaped => {
                    working.flush_to(&mut delta);
                    sync_point_after_bout(point, &working, &delta, z_ref_fe, is_zero_ref);
                    point.escapes = true;
                    point.delta = Some(delta);
                    return;
                }
                StepOutcome::Repeats => {
                    working.flush_to(&mut delta);
                    sync_point_after_bout(point, &working, &delta, z_ref_fe, is_zero_ref);
                    point.repeats = true;
                    point.period = point.iterations.saturating_sub(delta.checkpoint_n);
                    point.delta = Some(delta);
                    return;
                }
                StepOutcome::Glitch => {
                    reset_for_glitch(point, against_generation.max(delta.generation));
                    return;
                }
                StepOutcome::Exhausted => {
                    working.flush_to(&mut delta);
                    let stamp = delta.generation;
                    rebind_to_zero_continuing(point, &mut delta, stamp);
                    working = BoutWorking::from_delta(&delta);
                    orbit = zero_orbit();
                    is_zero_ref = true;
                }
                StepOutcome::Continue => {}
            }
        }

        working.flush_to(&mut delta);
        let z_ref = if is_zero_ref {
            ComplexFloatExp::ZERO
        } else {
            orbit
                .get(point.iterations.saturating_add(1))
                .unwrap_or(ComplexFloatExp::ZERO)
        };
        sync_point_after_bout(point, &working, &delta, z_ref, is_zero_ref);
        point.delta = Some(delta);
    }

    fn completion(&self, point: &mut Point<f64>) -> CompletedPoint<f64> {
        direct_completion(point)
    }
}
