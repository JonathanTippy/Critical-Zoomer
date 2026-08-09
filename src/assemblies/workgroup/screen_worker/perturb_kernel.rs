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
    abs_plane_f64, direct_completion_with_plane_c, ensure_started, update_point_results, bailout_point,
    iterate_with_plane_c, plane_c_from_little_c_f64, plane_c_floatexp_from_little_c, plane_c_for_seat_f64,
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

fn to_delta_plane_c_f64(plane_c: (f64, f64)) -> ComplexFloatExp {
    ComplexFloatExp::new(FloatExp::from(plane_c.0), FloatExp::from(plane_c.1))
}

fn reference_plane_c_floatexp(orbit: &ReferenceOrbit) -> ComplexFloatExp {
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
) {
    point.direct_only = true;
    point.bound_zero_generation = against_generation;
    delta.little_c = delta.plane_c;
    delta.little_z = floatexp_from_f64_pair(point.plane_z);
    delta.generation = 0;
    delta.gear = gear_for_delta(delta.little_c, delta.little_z);
    delta.scale = scaled_scale_from_dz(delta.little_z);
}

#[inline(always)]
fn sync_point_from_delta_fe(
    point: &mut Point<f64>,
    z_ref: ComplexFloatExp,
    little_z: ComplexFloatExp,
    dd: ComplexFloatExp,
) {
    let plane_z = z_ref + little_z;
    point.plane_z = (plane_z.re.to_f64(), plane_z.im.to_f64());
    point.dc = (dd.re.to_f64(), dd.im.to_f64());
    update_point_results(point);
}

#[inline(always)]
fn sync_point_from_f64_locals(
    point: &mut Point<f64>,
    z_ref: (f64, f64),
    little_z: (f64, f64),
    dd: (f64, f64),
) {
    point.plane_z = (z_ref.0 + little_z.0, z_ref.1 + little_z.1);
    point.dc = dd;
    update_point_results(point);
}

#[inline(always)]
fn near_fe(plane_z_next: ComplexFloatExp, checkpoint: ComplexFloatExp, epsilon: f64) -> bool {
    let eps = FloatExp::from(epsilon);
    (plane_z_next.re - checkpoint.re).abs() <= eps && (plane_z_next.im - checkpoint.im).abs() <= eps
}

fn init_delta(
    point: &mut Point<f64>,
    orbit: &ReferenceOrbit,
    generation: u64,
    little_c: (f64, f64),
    plane_c: (f64, f64),
    coords_are_relative: bool,
    anchor: &(IntExp, IntExp),
) {
    if std::ptr::eq(orbit, zero_orbit()) {
        init_delta_zero_orbit_f64(
            point,
            generation,
            little_c,
            plane_c,
            coords_are_relative,
            anchor,
        );
        return;
    }
    let plane_fe = if coords_are_relative {
        plane_c_floatexp_from_little_c(little_c, anchor)
    } else {
        to_delta_plane_c_f64(plane_c)
    };
    // Relative shells: generator little_c is already δc vs reference/anchor.
    let little_c_fe = if coords_are_relative {
        floatexp_from_f64_pair(little_c)
    } else {
        plane_fe.clone() - reference_plane_c_floatexp(orbit)
    };
    let little_z_fe = little_c_fe.clone();
    let dd = ComplexFloatExp::new(FloatExp::ONE, FloatExp::ZERO);
    let Some(z_ref) = orbit.get(1) else {
        point.delta = None;
        return;
    };
    let gear = gear_for_delta(little_c_fe, little_z_fe);
    let scale = scaled_scale_from_dz(little_z_fe);
    point.delta = Some(DeltaState {
        little_z: little_z_fe,
        checkpoint: ComplexFloatExp::ZERO,
        checkpoint_n: 0,
        little_c: little_c_fe,
        plane_c: plane_fe,
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
    point.plane_c = plane_c;
    sync_point_from_delta_fe(point, z_ref, little_z_fe, dd);
    point.loop_detection_point = (point.plane_z, 0);
}

/// Zero-orbit floor: skip gear scan / orbit lookup when shallow absolute f64.
fn init_delta_zero_orbit_f64(
    point: &mut Point<f64>,
    generation: u64,
    little_c: (f64, f64),
    plane_c: (f64, f64),
    coords_are_relative: bool,
    _anchor: &(IntExp, IntExp),
) {
    // Zero orbit requires δc = plane C. Relative shells without a published
    // reference should not stay here long — bootstrap installs a view-center orbit.
    let (little_c_fe, little_z_fe, fe_plane_c, gear, scale) = if coords_are_relative {
        let pc = floatexp_from_f64_pair(plane_c);
        let gear = gear_for_delta(pc.clone(), pc.clone());
        let scale = scaled_scale_from_dz(pc.clone());
        (pc.clone(), pc.clone(), pc, gear, scale)
    } else {
        let pc = floatexp_from_f64_pair(plane_c);
        let gear = gear_for_delta(pc.clone(), pc.clone());
        let scale = scaled_scale_from_dz(pc.clone());
        (pc.clone(), pc.clone(), pc, gear, scale)
    };
    point.delta = Some(DeltaState {
        little_z: little_z_fe,
        checkpoint: ComplexFloatExp::ZERO,
        checkpoint_n: 0,
        little_c: little_c_fe,
        plane_c: fe_plane_c,
        dd: ComplexFloatExp::new(FloatExp::ONE, FloatExp::ZERO),
        generation,
        gear,
        scale,
    });
    point.iterations = 0;
    point.escapes = false;
    point.repeats = false;
    point.period = 0;
    point.plane_c = plane_c;
    point.plane_z = if coords_are_relative {
        (fe_plane_c.re.to_f64(), fe_plane_c.im.to_f64())
    } else {
        plane_c
    };
    point.dc = (1.0, 0.0);
    point.loop_detection_point = (plane_c, 0);
    point.smallness_squared = f64::MAX;
    point.small_time = 0;
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
    let mut skip = series.safe_skip(delta.little_c, pub_ref.orbit.iterates.len().saturating_sub(1));
    if skip <= 1 {
        return;
    }
    // Series validity is not membership: never skip past the first bailout of
    // Z_n+δz_n. Doing so left exterior seats (incl. |c|>2) with inflated
    // escape_time (headed: always 2 after a published reference) and shattered
    // escape-time / small-time shading after navigate.
    // r[impl cz.depth.series-approximation+1]
    let bailout = FloatExp::from(4.0);
    for n in 1..=skip {
        let Some(little_z_n) = series.evaluate(n, delta.little_c) else {
            break;
        };
        let Some(z_ref_n) = pub_ref.orbit.get(n as u32) else {
            break;
        };
        if (z_ref_n + little_z_n).norm_squared() > bailout {
            // #region agent log
            crate::debug_agent::log(
                "D",
                "perturb_kernel.rs:series_skip",
                "series_skip_clamped_at_bailout",
                &format!(
                    "{{\"raw_would_continue\":true,\"clamp_n\":{n},\"dc2\":{}}}",
                    (delta.little_c.re.to_f64() * delta.little_c.re.to_f64()
                        + delta.little_c.im.to_f64() * delta.little_c.im.to_f64())
                ),
            );
            // #endregion
            skip = n;
            break;
        }
    }
    if skip <= 1 {
        return;
    }
    let Some(little_z) = series.evaluate(skip, delta.little_c) else {
        return;
    };
    let dd = delta.dd;
    // Series skip advances plane Z / little_z but must still collect min-|plane_z| along the
    // skipped prefix — otherwise small_time sticks at the skip index (square
    // STE bands) while escape_time is already correct.
    // r[impl cz.depth.series-approximation+1]
    let plane_c = delta.plane_c;
    for n in 0..=skip {
        let rad = if n == 0 {
            plane_c.norm_squared().to_f64()
        } else {
            let Some(little_z_n) = series.evaluate(n, delta.little_c) else {
                break;
            };
            let Some(z_ref_n) = pub_ref.orbit.get(n as u32) else {
                break;
            };
            (z_ref_n + little_z_n).norm_squared().to_f64()
        };
        if rad < point.smallness_squared {
            point.smallness_squared = rad;
            // Series step n lands at perturb iterations n-1 (see skip assignment).
            point.small_time = n.saturating_sub(1) as u32;
        }
    }
    delta.little_z = little_z;
    delta.gear = gear_for_delta(delta.little_c, little_z);
    delta.scale = scaled_scale_from_dz(little_z);
    point.iterations = skip.saturating_sub(1) as u32;
    let z_ref = pub_ref
        .orbit
        .get(point.iterations.saturating_add(1))
        .unwrap_or(ComplexFloatExp::ZERO);
    sync_point_from_delta_fe(point, z_ref, little_z, dd);
    // #region agent log
    {
        let st_after = point.small_time;
        crate::debug_agent::log(
            "A",
            "perturb_kernel.rs:series_skip",
            "series_skip_small_time_after_scan",
            &format!(
                "{{\"skip\":{skip},\"iters\":{},\"st\":{st_after},\"min_rad\":{}}}",
                point.iterations, point.smallness_squared
            ),
        );
    }
    // #endregion
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
    let plane_z = z_ref + delta.little_z;
    let plane_z_norm_sq = plane_z.norm_squared();
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

    if !is_zero_ref && delta.little_z != ComplexFloatExp::ZERO && z_ref_norm_sq == four {
        let correction = FloatExp::TWO * (z_ref.re * delta.little_z.re + z_ref.im * delta.little_z.im)
            + delta.little_z.norm_squared();
        if correction > FloatExp::ZERO {
            return StepOutcome::Escaped;
        }
        if correction == FloatExp::ZERO {
            return StepOutcome::Glitch;
        }
    }
    if plane_z_norm_sq > r_sq {
        return StepOutcome::Escaped;
    }
    if !is_zero_ref
        && point.iterations > 0
        && plane_z_norm_sq < z_ref_norm_sq * glitch_factor
    {
        return StepOutcome::Glitch;
    }
    let rad = plane_z_norm_sq.to_f64();
    if rad < point.smallness_squared {
        point.smallness_squared = rad;
        point.small_time = point.iterations;
    }
    delta.dd = plane_z * delta.dd * two + one;
    delta.little_z = z_ref * delta.little_z * two + delta.little_z * delta.little_z + delta.little_c;
    delta.gear = ComputeGear::FloatExp;
    point.iterations = point.iterations.saturating_add(1);
    let z_next_ref = if is_zero_ref {
        ComplexFloatExp::ZERO
    } else {
        orbit
            .get(point.iterations.saturating_add(1))
            .unwrap_or(ComplexFloatExp::ZERO)
    };
    let plane_z_next = z_next_ref + delta.little_z;
    if near_fe(plane_z_next, delta.checkpoint, epsilon) {
        return StepOutcome::Repeats;
    }
    if point.iterations >= delta.checkpoint_n.saturating_mul(2).max(1) {
        delta.checkpoint = plane_z_next;
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
        little_z: (f64, f64),
        little_c: (f64, f64),
        dd: (f64, f64),
        checkpoint: (f64, f64),
    },
    ScaledF64 {
        little_z_scaled: (f64, f64),
        little_c_scaled: (f64, f64),
        dd: (f64, f64),
        scale: FloatExp,
        checkpoint: (f64, f64),
    },
}

impl BoutWorking {
    fn from_delta(delta: &DeltaState) -> Self {
        match delta.gear {
            ComputeGear::F64 => BoutWorking::F64 {
                little_z: fe_pair(delta.little_z),
                little_c: fe_pair(delta.little_c),
                dd: fe_pair(delta.dd),
                checkpoint: fe_pair(delta.checkpoint),
            },
            ComputeGear::ScaledF64 => {
                let s = delta.scale.to_f64();
                if !s.is_finite() || s == 0.0 {
                    return BoutWorking::FloatExp;
                }
                BoutWorking::ScaledF64 {
                    little_z_scaled: (delta.little_z.re.to_f64() / s, delta.little_z.im.to_f64() / s),
                    little_c_scaled: (delta.little_c.re.to_f64() / s, delta.little_c.im.to_f64() / s),
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
                little_z,
                dd,
                checkpoint,
                ..
            } => {
                delta.little_z = floatexp_from_f64_pair(*little_z);
                delta.dd = floatexp_from_f64_pair(*dd);
                delta.checkpoint = floatexp_from_f64_pair(*checkpoint);
            }
            BoutWorking::ScaledF64 {
                little_z_scaled,
                dd,
                scale,
                checkpoint,
                ..
            } => {
                let s = scale.to_f64();
                let little_z = (little_z_scaled.0 * s, little_z_scaled.1 * s);
                delta.little_z = floatexp_from_f64_pair(little_z);
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
        BoutWorking::F64 { little_z, dd, .. } => {
            let z_ref = if is_zero_ref {
                (0.0, 0.0)
            } else {
                narrow_z_ref(z_ref_fe).unwrap_or(fe_pair(z_ref_fe))
            };
            sync_point_from_f64_locals(point, z_ref, *little_z, *dd);
        }
        BoutWorking::ScaledF64 { little_z_scaled, dd, scale, .. } => {
            let s = scale.to_f64();
            let little_z = (little_z_scaled.0 * s, little_z_scaled.1 * s);
            let z_ref = if is_zero_ref {
                (0.0, 0.0)
            } else {
                narrow_z_ref(z_ref_fe).unwrap_or(fe_pair(z_ref_fe))
            };
            sync_point_from_f64_locals(point, z_ref, little_z, *dd);
        }
        BoutWorking::FloatExp => {
            sync_point_from_delta_fe(point, z_ref_fe, delta.little_z, delta.dd);
        }
    }
}

#[inline(always)]
fn f64_period_check(
    plane_z_next: (f64, f64),
    checkpoint: (f64, f64),
    epsilon: f64,
    iterations: u32,
    checkpoint_n: u32,
) -> (bool, (f64, f64), u32) {
    if (plane_z_next.0 - checkpoint.0).abs() <= epsilon
        && (plane_z_next.1 - checkpoint.1).abs() <= epsilon
    {
        return (true, checkpoint, checkpoint_n);
    }
    if iterations >= checkpoint_n.saturating_mul(2).max(1) {
        (false, plane_z_next, iterations)
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
    little_z: (f64, f64),
    little_c: (f64, f64),
    dd: (f64, f64),
    checkpoint: (f64, f64),
) -> (StepOutcome, BoutWorking) {
    let working = BoutWorking::F64 {
        little_z,
        little_c,
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
    let plane_z = (z_ref.0 + little_z.0, z_ref.1 + little_z.1);
    let plane_z_norm = plane_z.0 * plane_z.0 + plane_z.1 * plane_z.1;
    let z_ref_norm = z_ref.0 * z_ref.0 + z_ref.1 * z_ref.1;
    if plane_z_norm > r_squared {
        return (StepOutcome::Escaped, working);
    }
    if !is_zero_ref && point.iterations > 0 && plane_z_norm < z_ref_norm * 1e-6 {
        return (StepOutcome::Glitch, working);
    }
    if plane_z_norm < point.smallness_squared {
        point.smallness_squared = plane_z_norm;
        point.small_time = point.iterations;
    }
    let (little_z_next, dd_next, next_gear) =
        f64_step(z_ref, little_z, little_c, dd, is_zero_ref);
    // Orbit state is little_z; dd is derivative coloring. Promote only when little_z cannot
    // continue in f64 — a non-finite dd must not drag the seat onto FloatExp.
    if !little_z_next.0.is_finite() || !little_z_next.1.is_finite() {
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
        let scale = scaled_scale_from_dz(floatexp_from_f64_pair(little_z_next));
        delta.gear = ComputeGear::ScaledF64;
        let s = scale.to_f64();
        BoutWorking::ScaledF64 {
            little_z_scaled: (little_z_next.0 / s, little_z_next.1 / s),
            little_c_scaled: (little_c.0 / s, little_c.1 / s),
            dd: dd_keep,
            scale,
            checkpoint,
        }
    } else {
        BoutWorking::F64 {
            little_z: little_z_next,
            little_c,
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
    let plane_z_next = (z_next_ref.0 + little_z_next.0, z_next_ref.1 + little_z_next.1);
    let (repeats, cp, cp_n) = f64_period_check(
        plane_z_next,
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
    little_z_scaled: (f64, f64),
    little_c_scaled: (f64, f64),
    dd: (f64, f64),
    scale: FloatExp,
    checkpoint: (f64, f64),
) -> (StepOutcome, BoutWorking) {
    let working = BoutWorking::ScaledF64 {
        little_z_scaled,
        little_c_scaled,
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
    let little_z = (little_z_scaled.0 * s, little_z_scaled.1 * s);
    let plane_z = (z_ref.0 + little_z.0, z_ref.1 + little_z.1);
    let plane_z_norm = plane_z.0 * plane_z.0 + plane_z.1 * plane_z.1;
    let z_ref_norm = z_ref.0 * z_ref.0 + z_ref.1 * z_ref.1;
    if plane_z_norm > r_squared {
        return (StepOutcome::Escaped, working);
    }
    if !is_zero_ref && point.iterations > 0 && plane_z_norm < z_ref_norm * 1e-6 {
        return (StepOutcome::Glitch, working);
    }
    if plane_z_norm < point.smallness_squared {
        point.smallness_squared = plane_z_norm;
        point.small_time = point.iterations;
    }
    let (little_z_scaled_next, scale_next, next_gear) =
        scaled_f64_step(z_ref, little_z_scaled, little_c_scaled, scale, is_zero_ref);
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
    let little_z_next = (little_z_scaled_next.0 * s_next, little_z_scaled_next.1 * s_next);
    if !little_z_next.0.is_finite() || !little_z_next.1.is_finite() || !s_next.is_finite() {
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
        2.0 * (plane_z.0 * dd.0 - plane_z.1 * dd.1) + 1.0,
        2.0 * (plane_z.0 * dd.1 + plane_z.1 * dd.0),
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
    let plane_z_next = (z_next_ref.0 + little_z_next.0, z_next_ref.1 + little_z_next.1);
    let (repeats, cp, cp_n) = f64_period_check(
        plane_z_next,
        checkpoint,
        epsilon,
        point.iterations,
        delta.checkpoint_n,
    );
    delta.checkpoint_n = cp_n;
    let next_working = BoutWorking::ScaledF64 {
        little_z_scaled: little_z_scaled_next,
        little_c_scaled,
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
    if point.plane_z.0.is_finite() && point.plane_z.1.is_finite() {
        delta.little_z = floatexp_from_f64_pair(point.plane_z);
    }
    if point.dc.0.is_finite() && point.dc.1.is_finite() {
        delta.dd = floatexp_from_f64_pair(point.dc);
    }
    flush_checkpoint_only(delta, checkpoint, delta.checkpoint_n);
}

/// Zero-orbit F64 bout — DirectKernel iterate + perturbation checkpoint semantics.
/// point.plane_z / point.dc authoritative; delta touched only for checkpoint (continue) or full flush (terminal).
fn zero_orbit_f64_iterate_bout(
    point: &mut Point<f64>,
    mut checkpoint: (f64, f64),
    mut checkpoint_n: u32,
    r_squared: f64,
    epsilon: f64,
    cap: BoutCap,
) -> (bool, (f64, f64), u32) {
    let plane_c = point.plane_c;
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
        iterate_with_plane_c(point, plane_c);
        if (point.plane_z.0 - checkpoint.0).abs() <= epsilon
            && (point.plane_z.1 - checkpoint.1).abs() <= epsilon
        {
            point.repeats = true;
            point.period = point.iterations.saturating_sub(checkpoint_n);
            return (true, checkpoint, checkpoint_n);
        }
        if point.iterations >= checkpoint_n.saturating_mul(2).max(1) {
            checkpoint = point.plane_z;
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
            let little_c = context.points[index].little_c;
            let plane_c = plane_c_for_seat_f64(context, little_c);
            context.points[index].plane_c = plane_c;
            // #region agent log
            if context.coords_are_relative && index == 0 {
                crate::debug_agent::log_hud(
                    "H3",
                    "perturb_kernel.rs:start_seat",
                    "relative_plane_c",
                    &format!(
                        "{{\"little_c\":[{:.3e},{:.3e}],\"plane_c\":[{:.6},{:.6}],\"has_ref\":{},\"ref_floor\":{}}}",
                        little_c.0,
                        little_c.1,
                        plane_c.0,
                        plane_c.1,
                        context.latest_reference.is_some(),
                        context.reference_floor_active,
                    ),
                );
            }
            // #endregion
            init_delta(
                &mut context.points[index],
                orbit,
                generation,
                little_c,
                plane_c,
                context.coords_are_relative,
                &context.coord_anchor,
            );
            apply_series_skip(&mut context.points[index], published);
            // #region agent log
            if context.coords_are_relative && index == 0 {
                let lc = context.points[index].little_c;
                let gear = context.points[index]
                    .delta
                    .as_ref()
                    .map(|d| d.gear.hud_label())
                    .unwrap_or("none");
                let lc_mag = (lc.0 * lc.0 + lc.1 * lc.1).sqrt();
                let delta_lc_mag = context.points[index]
                    .delta
                    .as_ref()
                    .map(|d| d.little_c.norm_squared().to_f64().sqrt())
                    .unwrap_or(0.0);
                crate::debug_agent::log_hud(
                    "A,B",
                    "perturb_kernel.rs:start_seat",
                    "relative_init_gear",
                    &format!(
                        "{{\"gear\":\"{gear}\",\"little_c_mag\":{lc_mag:.3e},\"delta_little_c_mag\":{delta_lc_mag:.3e},\"zero_orbit\":{},\"has_ref\":{}}}",
                        std::ptr::eq(orbit, zero_orbit()),
                        published.is_some(),
                    ),
                );
            }
            // #endregion
            // #region agent log
            if context.coords_are_relative && index == 0 {
                let lc = context.points[index].little_c;
                crate::debug_agent::log_hud(
                    "H4",
                    "perturb_kernel.rs:start_seat",
                    "post_init_little_c",
                    &format!(
                        "{{\"little_c\":[{:.6},{:.6}],\"plane_c\":[{:.6},{:.6}],\"little_c_mag\":{:.6}}}",
                        lc.0,
                        lc.1,
                        plane_c.0,
                        plane_c.1,
                        (lc.0 * lc.0 + lc.1 * lc.1).sqrt()
                    ),
                );
            }
            // #endregion
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
                    mut little_z,
                    little_c,
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
                        little_z,
                        little_c,
                        dd,
                        checkpoint,
                    );
                    working = next;
                    out
                }
                BoutWorking::ScaledF64 {
                    mut little_z_scaled,
                    little_c_scaled,
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
                        little_z_scaled,
                        little_c_scaled,
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
        let plane_c = point
            .delta
            .as_ref()
            .map(|d| (d.plane_c.re.to_f64(), d.plane_c.im.to_f64()))
            .unwrap_or(point.plane_c);
        let out = direct_completion_with_plane_c(point, plane_c);
        // #region agent log
        {
            let little_c2 = point.little_c.0 * point.little_c.0 + point.little_c.1 * point.little_c.1;
            let plane_c2 = plane_c.0 * plane_c.0 + plane_c.1 * plane_c.1;
            if point.escapes || point.repeats {
                static COMP_N: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
                let n = COMP_N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                if n < 5 {
                    let (sl0, sl1, et, st) = match &out {
                        CompletedPoint::Escapes {
                            start_location,
                            escape_time,
                            small_time,
                            ..
                        } => (
                            start_location.0.into(),
                            start_location.1.into(),
                            *escape_time,
                            *small_time,
                        ),
                        CompletedPoint::Repeats { small_time, .. } => {
                            (plane_c.0, plane_c.1, point.iterations, *small_time)
                        }
                        _ => (0.0, 0.0, 0, 0),
                    };
                    crate::debug_agent::log_hud(
                        "H4",
                        "perturb_kernel.rs:completion",
                        "completion_start_location",
                        &format!(
                            "{{\"start_loc\":[{sl0:.6},{sl1:.6}],\"little_c2\":{little_c2:.6},\"plane_c2\":{plane_c2:.6},\"et\":{et},\"st\":{st},\"escapes\":{},\"repeats\":{}}}",
                            point.escapes,
                            point.repeats
                        ),
                    );
                }
            }
            let st = match &out {
                CompletedPoint::Escapes { small_time, .. }
                | CompletedPoint::Repeats { small_time, .. } => *small_time,
                _ => 999,
            };
            if plane_c2 <= 4.0 && (point.escapes || point.repeats) {
                crate::debug_agent::log(
                    "B,E",
                    "perturb_kernel.rs:completion",
                    "interior_small_time",
                    &format!(
                        "{{\"plane_c2\":{plane_c2},\"st\":{st},\"et\":{},\"escapes\":{},\"repeats\":{}}}",
                        point.iterations, point.escapes, point.repeats
                    ),
                );
            } else if point.escapes && plane_c2 > 4.0 {
                static OUTER_N: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
                let n = OUTER_N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                if n < 30 || n % 200 == 0 {
                    let zero = point
                        .delta
                        .as_ref()
                        .map(|d| d.generation == 0)
                        .unwrap_or(true);
                    let et = match &out {
                        CompletedPoint::Escapes { escape_time, small_time, .. } => (*escape_time, *small_time),
                        _ => (999, 999),
                    };
                    crate::debug_agent::log(
                        "A,C,D",
                        "perturb_kernel.rs:completion",
                        "outer_escape_completion",
                        &format!(
                            "{{\"plane_c2\":{plane_c2},\"et\":{},\"st\":{},\"zero_gen\":{zero},\"direct_only\":{},\"iters\":{}}}",
                            et.0, et.1, point.direct_only, point.iterations
                        ),
                    );
                }
            }
        }
        // #endregion
        out
    }
}
