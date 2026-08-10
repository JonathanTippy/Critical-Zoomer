//! Perturbation delta kernel (`mode:pert` production path for f64 seats).
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
use crate::utils::IntExp;

use super::workshift::{
    abs_c_f64, direct_completion_with_c, ensure_started, update_point_results, bailout_point,
    iterate_with_c, c_from_delta_c_f64, c_floatexp_from_delta_c, c_for_seat_f64,
    iterate, BoutCap, CompletedPoint, DeltaState, Point, SeatKernel, WorkContext,
};

/// Perturbation delta kernel (`mode:pert`).
// r[impl cz.depth.delta-kernel+1]
// r[impl cz.perf.pps-selected-kernel+1]
// r[impl cz.ref.zero-orbit-same-path+1]
// r[impl cz.depth.compute-gear+1]
#[derive(Clone, Copy, Debug, Default)]
pub struct PerturbationKernel;

fn zero_orbit() -> &'static ReferenceOrbit {
    static ZERO: OnceLock<ReferenceOrbit> = OnceLock::new();
    ZERO.get_or_init(ReferenceOrbit::zero_orbit)
}

fn absolute_c_floatexp_from_f64(c: (f64, f64)) -> ComplexFloatExp {
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
        // Escaped orbits still supply pre-escape iterates; soft-continue after the tip.
        r
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
    absolute_z: ComplexFloatExp,
) {
    // Soft-continue / zero-orbit: δc slot holds absolute c; δz slot holds absolute z.
    // Never copy generator Point.delta_c into delta.delta_c (that paints exterior as "in").
    point.direct_only = true;
    point.bound_zero_generation = against_generation;
    delta.delta_c = delta.c;
    delta.delta_z = absolute_z;
    delta.generation = 0;
    // Prefer FloatExp when absolute c/z lose bits under f64 (deep relative soft-continue);
    // keep f64 gear on shallow absolute shells so home fill stays fast.
    let c_bits_lost = floatexp_from_f64_pair((delta.c.re.to_f64(), delta.c.im.to_f64())) != delta.c;
    let z_bits_lost =
        floatexp_from_f64_pair((absolute_z.re.to_f64(), absolute_z.im.to_f64())) != absolute_z;
    delta.gear = if c_bits_lost || z_bits_lost {
        ComputeGear::FloatExp
    } else {
        gear_for_delta(delta.delta_c, delta.delta_z)
    };
    delta.scale = scaled_scale_from_dz(delta.delta_z);
}

#[inline(always)]
fn sync_point_from_delta_fe(
    point: &mut Point<f64>,
    z_ref: ComplexFloatExp,
    delta_z: ComplexFloatExp,
    dd: ComplexFloatExp,
) {
    let z = z_ref + delta_z;
    point.z = (z.re.to_f64(), z.im.to_f64());
    point.dc = (dd.re.to_f64(), dd.im.to_f64());
    update_point_results(point);
}

#[inline(always)]
fn sync_point_from_f64_locals(
    point: &mut Point<f64>,
    z_ref: (f64, f64),
    delta_z: (f64, f64),
    dd: (f64, f64),
) {
    point.z = (z_ref.0 + delta_z.0, z_ref.1 + delta_z.1);
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
    delta_c: (f64, f64),
    c: (f64, f64),
    coords_are_relative: bool,
    anchor: &(IntExp, IntExp),
) {
    if std::ptr::eq(orbit, zero_orbit()) {
        init_delta_zero_orbit_f64(
            point,
            generation,
            delta_c,
            c,
            coords_are_relative,
            anchor,
        );
        return;
    }
    let plane_fe = if coords_are_relative {
        c_floatexp_from_delta_c(delta_c, anchor)
    } else {
        absolute_c_floatexp_from_f64(c)
    };
    // Relative shells: generator delta_c is already δc vs reference/anchor.
    let delta_c_fe = if coords_are_relative {
        floatexp_from_f64_pair(delta_c)
    } else {
        plane_fe.clone() - reference_c_floatexp(orbit)
    };
    let delta_z_fe = delta_c_fe.clone();
    let dd = ComplexFloatExp::new(FloatExp::ONE, FloatExp::ZERO);
    let Some(z_ref) = orbit.get(1) else {
        point.delta = None;
        return;
    };
    let gear = gear_for_delta(delta_c_fe, delta_z_fe);
    let scale = scaled_scale_from_dz(delta_z_fe);
    point.delta = Some(DeltaState {
        delta_z: delta_z_fe,
        checkpoint: ComplexFloatExp::ZERO,
        checkpoint_n: 0,
        delta_c: delta_c_fe,
        c: plane_fe,
        dd,
        generation,
        gear,
        scale,
    });
    point.iterations = 0;
    point.escapes = false;
    point.repeats = false;
    point.period = 0;
    point.smallness_squared = f64::MAX;
    point.small_time = 0;
    point.c = c;
    sync_point_from_delta_fe(point, z_ref, delta_z_fe, dd);
    point.loop_detection_point = (point.z, 0);
}

/// Zero-orbit floor: skip gear scan / orbit lookup when shallow absolute f64.
///
/// The δc / δz slots must hold absolute `c` / `z` (same math as naive). Never put
/// generator `delta_c` in those slots.
fn init_delta_zero_orbit_f64(
    point: &mut Point<f64>,
    generation: u64,
    generator_delta_c: (f64, f64),
    c: (f64, f64),
    coords_are_relative: bool,
    anchor: &(IntExp, IntExp),
) {
    // Absolute c in the δc slot. Relative shells reconstruct via FloatExp (anchor +
    // generator delta_c); do not trust collapsed f64 `c` alone.
    let (delta_c_fe, delta_z_fe, fe_c, gear, scale) = if coords_are_relative {
        let pc = c_floatexp_from_delta_c(generator_delta_c, anchor);
        let gear = gear_for_delta(pc.clone(), pc.clone());
        let scale = scaled_scale_from_dz(pc.clone());
        (pc.clone(), pc.clone(), pc, gear, scale)
    } else {
        let pc = floatexp_from_f64_pair(c);
        let gear = gear_for_delta(pc.clone(), pc.clone());
        let scale = scaled_scale_from_dz(pc.clone());
        (pc.clone(), pc.clone(), pc, gear, scale)
    };
    point.delta = Some(DeltaState {
        delta_z: delta_z_fe,
        checkpoint: ComplexFloatExp::ZERO,
        checkpoint_n: 0,
        delta_c: delta_c_fe,
        c: fe_c,
        dd: ComplexFloatExp::new(FloatExp::ONE, FloatExp::ZERO),
        generation,
        gear,
        scale,
    });
    point.iterations = 0;
    point.escapes = false;
    point.repeats = false;
    point.period = 0;
    point.c = c;
    point.z = if coords_are_relative {
        (fe_c.re.to_f64(), fe_c.im.to_f64())
    } else {
        c
    };
    point.dc = (1.0, 0.0);
    point.loop_detection_point = (c, 0);
    point.smallness_squared = f64::MAX;
    point.small_time = 0;
    update_point_results(point);
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
    let z = z_ref + delta.delta_z;
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

    if !is_zero_ref && delta.delta_z != ComplexFloatExp::ZERO && z_ref_norm_sq == four {
        let correction = FloatExp::TWO * (z_ref.re * delta.delta_z.re + z_ref.im * delta.delta_z.im)
            + delta.delta_z.norm_squared();
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
    delta.delta_z = z_ref * delta.delta_z * two + delta.delta_z * delta.delta_z + delta.delta_c;
    delta.gear = ComputeGear::FloatExp;
    point.iterations = point.iterations.saturating_add(1);
    let z_next_ref = if is_zero_ref {
        ComplexFloatExp::ZERO
    } else {
        orbit
            .get(point.iterations.saturating_add(1))
            .unwrap_or(ComplexFloatExp::ZERO)
    };
    let z_next = z_next_ref + delta.delta_z;
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
        delta_z: (f64, f64),
        delta_c: (f64, f64),
        dd: (f64, f64),
        checkpoint: (f64, f64),
    },
    ScaledF64 {
        delta_z_scaled: (f64, f64),
        delta_c_scaled: (f64, f64),
        dd: (f64, f64),
        scale: FloatExp,
        checkpoint: (f64, f64),
    },
}

impl BoutWorking {
    fn from_delta(delta: &DeltaState) -> Self {
        match delta.gear {
            ComputeGear::F64 => BoutWorking::F64 {
                delta_z: fe_pair(delta.delta_z),
                delta_c: fe_pair(delta.delta_c),
                dd: fe_pair(delta.dd),
                checkpoint: fe_pair(delta.checkpoint),
            },
            ComputeGear::ScaledF64 => {
                let s = delta.scale.to_f64();
                if !s.is_finite() || s == 0.0 {
                    return BoutWorking::FloatExp;
                }
                BoutWorking::ScaledF64 {
                    delta_z_scaled: (delta.delta_z.re.to_f64() / s, delta.delta_z.im.to_f64() / s),
                    delta_c_scaled: (delta.delta_c.re.to_f64() / s, delta.delta_c.im.to_f64() / s),
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
                delta_z,
                dd,
                checkpoint,
                ..
            } => {
                delta.delta_z = floatexp_from_f64_pair(*delta_z);
                delta.dd = floatexp_from_f64_pair(*dd);
                delta.checkpoint = floatexp_from_f64_pair(*checkpoint);
            }
            BoutWorking::ScaledF64 {
                delta_z_scaled,
                dd,
                scale,
                checkpoint,
                ..
            } => {
                let s = scale.to_f64();
                let delta_z = (delta_z_scaled.0 * s, delta_z_scaled.1 * s);
                delta.delta_z = floatexp_from_f64_pair(delta_z);
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
        BoutWorking::F64 { delta_z, dd, .. } => {
            let z_ref = if is_zero_ref {
                (0.0, 0.0)
            } else {
                narrow_z_ref(z_ref_fe).unwrap_or(fe_pair(z_ref_fe))
            };
            sync_point_from_f64_locals(point, z_ref, *delta_z, *dd);
        }
        BoutWorking::ScaledF64 { delta_z_scaled, dd, scale, .. } => {
            let s = scale.to_f64();
            let delta_z = (delta_z_scaled.0 * s, delta_z_scaled.1 * s);
            let z_ref = if is_zero_ref {
                (0.0, 0.0)
            } else {
                narrow_z_ref(z_ref_fe).unwrap_or(fe_pair(z_ref_fe))
            };
            sync_point_from_f64_locals(point, z_ref, delta_z, *dd);
        }
        BoutWorking::FloatExp => {
            sync_point_from_delta_fe(point, z_ref_fe, delta.delta_z, delta.dd);
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
    delta_z: (f64, f64),
    delta_c: (f64, f64),
    dd: (f64, f64),
    checkpoint: (f64, f64),
) -> (StepOutcome, BoutWorking) {
    let working = BoutWorking::F64 {
        delta_z,
        delta_c,
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
    let z = (z_ref.0 + delta_z.0, z_ref.1 + delta_z.1);
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
    let (delta_z_next, dd_next, next_gear) =
        f64_step(z_ref, delta_z, delta_c, dd, is_zero_ref);
    // Orbit state is delta_z; dd is derivative coloring. Promote only when delta_z cannot
    // continue in f64 — a non-finite dd must not drag the seat onto FloatExp.
    if !delta_z_next.0.is_finite() || !delta_z_next.1.is_finite() {
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
        let scale = scaled_scale_from_dz(floatexp_from_f64_pair(delta_z_next));
        delta.gear = ComputeGear::ScaledF64;
        let s = scale.to_f64();
        BoutWorking::ScaledF64 {
            delta_z_scaled: (delta_z_next.0 / s, delta_z_next.1 / s),
            delta_c_scaled: (delta_c.0 / s, delta_c.1 / s),
            dd: dd_keep,
            scale,
            checkpoint,
        }
    } else {
        BoutWorking::F64 {
            delta_z: delta_z_next,
            delta_c,
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
    let z_next = (z_next_ref.0 + delta_z_next.0, z_next_ref.1 + delta_z_next.1);
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
    delta_z_scaled: (f64, f64),
    delta_c_scaled: (f64, f64),
    dd: (f64, f64),
    scale: FloatExp,
    checkpoint: (f64, f64),
) -> (StepOutcome, BoutWorking) {
    let working = BoutWorking::ScaledF64 {
        delta_z_scaled,
        delta_c_scaled,
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
    let delta_z = (delta_z_scaled.0 * s, delta_z_scaled.1 * s);
    let z = (z_ref.0 + delta_z.0, z_ref.1 + delta_z.1);
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
    let (delta_z_scaled_next, scale_next, next_gear) =
        scaled_f64_step(z_ref, delta_z_scaled, delta_c_scaled, scale, is_zero_ref);
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
    let delta_z_next = (delta_z_scaled_next.0 * s_next, delta_z_scaled_next.1 * s_next);
    if !delta_z_next.0.is_finite() || !delta_z_next.1.is_finite() || !s_next.is_finite() {
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
    let z_next = (z_next_ref.0 + delta_z_next.0, z_next_ref.1 + delta_z_next.1);
    let (repeats, cp, cp_n) = f64_period_check(
        z_next,
        checkpoint,
        epsilon,
        point.iterations,
        delta.checkpoint_n,
    );
    delta.checkpoint_n = cp_n;
    let next_working = BoutWorking::ScaledF64 {
        delta_z_scaled: delta_z_scaled_next,
        delta_c_scaled,
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
        delta.delta_z = floatexp_from_f64_pair(point.z);
    }
    if point.dc.0.is_finite() && point.dc.1.is_finite() {
        delta.dd = floatexp_from_f64_pair(point.dc);
    }
    flush_checkpoint_only(delta, checkpoint, delta.checkpoint_n);
}

/// Zero-orbit F64 bout — DirectKernel iterate + perturbation checkpoint semantics.
/// point.z / point.dc authoritative; delta touched only for checkpoint (continue) or full flush (terminal).
fn zero_orbit_f64_iterate_bout(
    point: &mut Point<f64>,
    mut checkpoint: (f64, f64),
    mut checkpoint_n: u32,
    r_squared: f64,
    epsilon: f64,
    cap: BoutCap,
) -> (bool, (f64, f64), u32) {
    let c = point.c;
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
        iterate_with_c(point, c);
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
        let published = if context.perturbation_reference_active() {
            context.latest_reference.as_deref()
        } else {
            None
        };
        maybe_clear_zero_bind(&mut context.points[index], published);
        let generation = active_generation(&context.points[index], published);
        let orbit = active_orbit(
            &context.points[index],
            published.map(|r| &r.orbit),
        );
        let needs_restart = match &context.points[index].delta {
            None => true,
            Some(d) => d.generation != generation,
        };
        if needs_restart {
            let delta_c = context.points[index].delta_c;
            let c = c_for_seat_f64(context, delta_c);
            context.points[index].c = c;
            init_delta(
                &mut context.points[index],
                orbit,
                generation,
                delta_c,
                c,
                context.coords_are_relative,
                &context.coord_anchor,
            );
            // Series approximation deferred — no apply_series_skip.
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
                        // Absolute z at last finished step: reference_z + delta_z (not stale Point.z).
                        let absolute_z = orbit
                            .get(point.iterations)
                            .map(|z_ref| z_ref + delta.delta_z)
                            .unwrap_or_else(|| floatexp_from_f64_pair(point.z));
                        rebind_to_zero_continuing(point, &mut delta, stamp, absolute_z);
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
                    mut delta_z,
                    delta_c,
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
                        delta_z,
                        delta_c,
                        dd,
                        checkpoint,
                    );
                    working = next;
                    out
                }
                BoutWorking::ScaledF64 {
                    mut delta_z_scaled,
                    delta_c_scaled,
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
                        delta_z_scaled,
                        delta_c_scaled,
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
                    let absolute_z = orbit
                        .get(point.iterations)
                        .map(|z_ref| z_ref + delta.delta_z)
                        .unwrap_or_else(|| floatexp_from_f64_pair(point.z));
                    rebind_to_zero_continuing(point, &mut delta, stamp, absolute_z);
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
        let c = point
            .delta
            .as_ref()
            .map(|d| (d.c.re.to_f64(), d.c.im.to_f64()))
            .unwrap_or(point.c);
        let out = direct_completion_with_c(point, c);
        out
    }
}
