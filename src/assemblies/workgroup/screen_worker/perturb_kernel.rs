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
    direct_completion, ensure_started, refresh_active_gear, update_point_results, BoutCap,
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
    let abs = to_delta_c_f64(abs_c);
    let dc = if std::ptr::eq(orbit, zero_orbit()) {
        abs
    } else {
        abs - reference_c_floatexp(orbit)
    };
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
            let abs_c = abs_plane_f64(context.points[index].c, &context.coord_anchor);
            init_delta(&mut context.points[index], orbit, generation, abs_c);
            apply_series_skip(
                &mut context.points[index],
                context.latest_reference.as_deref(),
            );
        }
        refresh_active_gear(context);
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
        let mut orbit = active_orbit(point, reference);
        let Some(mut delta) = point.delta.take() else {
            return;
        };
        let against_generation = if point.direct_only {
            point.bound_zero_generation
        } else {
            delta.generation
        };
        let mut is_zero_ref = std::ptr::eq(orbit, zero_orbit());

        for _ in 0..cap.get() {
            let n = point.iterations.saturating_add(1);
            let z_ref_fe = if is_zero_ref {
                ComplexFloatExp::ZERO
            } else {
                match orbit.get(n) {
                    Some(z) => z,
                    None => {
                        let stamp = delta.generation;
                        rebind_to_zero_continuing(point, &mut delta, stamp);
                        orbit = zero_orbit();
                        is_zero_ref = true;
                        ComplexFloatExp::ZERO
                    }
                }
            };

            let outcome = if delta.gear == ComputeGear::FloatExp || delta.gear == ComputeGear::Mixed {
                fe_iterate_step(
                    point,
                    orbit,
                    &mut delta,
                    is_zero_ref,
                    z_ref_fe,
                    r_squared,
                    epsilon,
                )
            } else if narrow_z_ref(z_ref_fe).is_none() {
                delta.gear = ComputeGear::FloatExp;
                fe_iterate_step(
                    point,
                    orbit,
                    &mut delta,
                    is_zero_ref,
                    z_ref_fe,
                    r_squared,
                    epsilon,
                )
            } else {
                let z_ref = narrow_z_ref(z_ref_fe).unwrap_or((0.0, 0.0));
                match delta.gear {
                    ComputeGear::F64 => {
                        let dz = fe_pair(delta.dz);
                        let dc = fe_pair(delta.dc);
                        let dd = fe_pair(delta.dd);
                        let (dz_next, dd_next, next_gear) =
                            f64_step(z_ref, dz, dc, dd, is_zero_ref);
                        if next_gear == ComputeGear::ScaledF64 {
                            delta.gear = ComputeGear::ScaledF64;
                            delta.scale = scaled_scale_from_dz(delta.dz);
                        }
                        delta.dz = floatexp_from_f64_pair(dz_next);
                        delta.dd = floatexp_from_f64_pair(dd_next);
                        let z = (z_ref.0 + dz_next.0, z_ref.1 + dz_next.1);
                        if z.0 * z.0 + z.1 * z.1 > r_squared {
                            StepOutcome::Escaped
                        } else if !is_zero_ref
                            && point.iterations > 0
                            && (z.0 * z.0 + z.1 * z.1)
                                < (z_ref.0 * z_ref.0 + z_ref.1 * z_ref.1) * 1e-6
                        {
                            StepOutcome::Glitch
                        } else {
                            point.iterations = point.iterations.saturating_add(1);
                            let z_next_ref = if is_zero_ref {
                                (0.0, 0.0)
                            } else {
                                orbit
                                    .get(point.iterations.saturating_add(1))
                                    .and_then(narrow_z_ref)
                                    .unwrap_or((0.0, 0.0))
                            };
                            let z_next = (
                                z_next_ref.0 + dz_next.0,
                                z_next_ref.1 + dz_next.1,
                            );
                            if (z_next.0 - delta.checkpoint.re.to_f64()).abs() <= epsilon
                                && (z_next.1 - delta.checkpoint.im.to_f64()).abs() <= epsilon
                            {
                                StepOutcome::Repeats
                            } else {
                                let rad = z.0 * z.0 + z.1 * z.1;
                                if rad < point.smallness_squared {
                                    point.smallness_squared = rad;
                                    point.small_time = point.iterations - 1;
                                }
                                if point.iterations
                                    >= delta.checkpoint_n.saturating_mul(2).max(1)
                                {
                                    delta.checkpoint = floatexp_from_f64_pair(z_next);
                                    delta.checkpoint_n = point.iterations;
                                }
                                StepOutcome::Continue
                            }
                        }
                    }
                    ComputeGear::ScaledF64 => {
                        let s = delta.scale.to_f64();
                        if !s.is_finite() || s == 0.0 {
                            delta.gear = ComputeGear::FloatExp;
                            fe_iterate_step(
                                point,
                                orbit,
                                &mut delta,
                                is_zero_ref,
                                z_ref_fe,
                                r_squared,
                                epsilon,
                            )
                        } else {
                            let w = (
                                delta.dz.re.to_f64() / s,
                                delta.dz.im.to_f64() / s,
                            );
                            let d = (
                                delta.dd.re.to_f64() / s,
                                delta.dd.im.to_f64() / s,
                            );
                            let (w_next, d_next, scale, next_gear) = scaled_f64_step(
                                z_ref,
                                w,
                                d,
                                delta.scale,
                                delta.dc,
                                is_zero_ref,
                            );
                            if next_gear == ComputeGear::FloatExp {
                                delta.gear = ComputeGear::FloatExp;
                                fe_iterate_step(
                                    point,
                                    orbit,
                                    &mut delta,
                                    is_zero_ref,
                                    z_ref_fe,
                                    r_squared,
                                    epsilon,
                                )
                            } else {
                                delta.scale = scale;
                                delta.dz = floatexp_from_f64_pair((
                                    w_next.0 * scale.to_f64(),
                                    w_next.1 * scale.to_f64(),
                                ));
                                delta.dd = floatexp_from_f64_pair((
                                    d_next.0 * scale.to_f64(),
                                    d_next.1 * scale.to_f64(),
                                ));
                                let z = (
                                    z_ref.0 + w_next.0 * scale.to_f64(),
                                    z_ref.1 + w_next.1 * scale.to_f64(),
                                );
                                if z.0 * z.0 + z.1 * z.1 > r_squared {
                                    StepOutcome::Escaped
                                } else {
                                    point.iterations = point.iterations.saturating_add(1);
                                    StepOutcome::Continue
                                }
                            }
                        }
                    }
                    _ => StepOutcome::Continue,
                }
            };

            match outcome {
                StepOutcome::Escaped => {
                    sync_point_from_delta_fe(point, z_ref_fe, delta.dz, delta.dd);
                    point.escapes = true;
                    point.delta = Some(delta);
                    return;
                }
                StepOutcome::Repeats => {
                    sync_point_from_delta_fe(point, z_ref_fe, delta.dz, delta.dd);
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
                    let stamp = delta.generation;
                    rebind_to_zero_continuing(point, &mut delta, stamp);
                    orbit = zero_orbit();
                    is_zero_ref = true;
                }
                StepOutcome::Continue => {}
            }
        }

        let z_ref = if is_zero_ref {
            ComplexFloatExp::ZERO
        } else {
            orbit
                .get(point.iterations.saturating_add(1))
                .unwrap_or(ComplexFloatExp::ZERO)
        };
        sync_point_from_delta_fe(point, z_ref, delta.dz, delta.dd);
        point.delta = Some(delta);
    }

    fn completion(&self, point: &mut Point<f64>) -> CompletedPoint<f64> {
        direct_completion(point)
    }
}
