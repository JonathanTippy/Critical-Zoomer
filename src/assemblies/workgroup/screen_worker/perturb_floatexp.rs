//! FloatExp-coordinate perturbation kernel for depth tests / benchmarks.
//!
//! Production live actors use `SeatKernel<f64>` in `perturb_kernel.rs`.

use std::sync::OnceLock;

use crate::floatexp::{ComplexFloatExp, FloatExp};
use crate::reference::ReferenceOrbit;
use crate::series::SeriesApproximation;

use super::workshift::{
    absolute_plane_c, direct_completion, ensure_started, update_point_results, BoutCap,
    CompletedPoint, DeltaState, Point, SeatKernel, WorkContext,
};

#[derive(Clone, Copy, Debug, Default)]
pub struct FloatExpPerturbationKernel;

fn zero_orbit() -> &'static ReferenceOrbit {
    static ZERO: OnceLock<ReferenceOrbit> = OnceLock::new();
    ZERO.get_or_init(ReferenceOrbit::zero_orbit)
}

fn to_delta_c(c: (FloatExp, FloatExp)) -> ComplexFloatExp {
    ComplexFloatExp::new(c.0, c.1)
}

fn reference_c_floatexp(orbit: &ReferenceOrbit) -> ComplexFloatExp {
    ComplexFloatExp::new(
        FloatExp::from_rug(&orbit.c.0),
        FloatExp::from_rug(&orbit.c.1),
    )
}

fn active_orbit<'a>(
    point: &Point<FloatExp>,
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
    point: &Point<FloatExp>,
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
    point: &mut Point<FloatExp>,
    published: Option<&crate::assemblies::workgroup::reference_worker::PublishedReference>,
) {
    let gen = published_generation(published);
    if point.direct_only && gen != point.bound_zero_generation {
        point.direct_only = false;
        point.delta = None;
        point.initialized = false;
    }
}

fn reset_for_glitch(point: &mut Point<FloatExp>, against_generation: u64) {
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
    point: &mut Point<FloatExp>,
    delta: &mut DeltaState,
    against_generation: u64,
) {
    point.direct_only = true;
    point.bound_zero_generation = against_generation;
    delta.dc = delta.abs_c;
    delta.dz = ComplexFloatExp::new(point.z.0, point.z.1);
    delta.generation = 0;
    delta.gear = crate::delta_gear::ComputeGear::FloatExp;
}

#[inline(always)]
fn sync_point_from_delta(
    point: &mut Point<FloatExp>,
    z_ref: ComplexFloatExp,
    dz: ComplexFloatExp,
    dd: ComplexFloatExp,
) {
    let z = z_ref + dz;
    point.z = (z.re, z.im);
    point.dc = (dd.re, dd.im);
    update_point_results(point);
}

#[inline(always)]
fn near_complex(a: ComplexFloatExp, b: ComplexFloatExp, epsilon: FloatExp) -> bool {
    (a.re - b.re).abs() <= epsilon && (a.im - b.im).abs() <= epsilon
}

fn init_delta(
    point: &mut Point<FloatExp>,
    orbit: &ReferenceOrbit,
    generation: u64,
    abs_c: (FloatExp, FloatExp),
) {
    let abs = to_delta_c(abs_c);
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
    point.delta = Some(DeltaState {
        dz,
        checkpoint: ComplexFloatExp::ZERO,
        checkpoint_n: 0,
        dc,
        abs_c: abs,
        dd,
        generation,
        gear: crate::delta_gear::ComputeGear::FloatExp,
        scale: FloatExp::ONE,
    });
    point.iterations = 0;
    point.escapes = false;
    point.repeats = false;
    point.period = 0;
    point.loop_detection_point = ((point.c.0, point.c.1), 0);
    point.smallness_squared = <FloatExp as crate::assemblies::workgroup::c_generator::Mandelbrotable>::max_value();
    point.small_time = 0;
    sync_point_from_delta(point, z_ref, dz, dd);
}

fn apply_series_skip(
    point: &mut Point<FloatExp>,
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
    let mut skip = series.safe_skip(delta.dc, pub_ref.orbit.iterates.len().saturating_sub(1));
    if skip <= 1 {
        return;
    }
    // Never skip past the first bailout of Z_n+δz_n (same rule as f64 kernel).
    // r[impl cz.depth.series-approximation+1]
    let bailout = FloatExp::from(4.0);
    for n in 1..=skip {
        let Some(dz_n) = series.evaluate(n, delta.dc) else {
            break;
        };
        let Some(z_ref_n) = pub_ref.orbit.get(n as u32) else {
            break;
        };
        if (z_ref_n + dz_n).norm_squared() > bailout {
            skip = n;
            break;
        }
    }
    if skip <= 1 {
        return;
    }
    let Some(dz) = series.evaluate(skip, delta.dc) else {
        return;
    };
    let dd = delta.dd;
    let dc = delta.dc;
    let abs_c = delta.abs_c;
    for n in 0..=skip {
        let rad = if n == 0 {
            abs_c.norm_squared().to_f64()
        } else {
            let Some(dz_n) = series.evaluate(n, dc) else {
                break;
            };
            let Some(z_ref_n) = pub_ref.orbit.get(n as u32) else {
                break;
            };
            (z_ref_n + dz_n).norm_squared().to_f64()
        };
        if rad < point.smallness_squared.to_f64() {
            point.smallness_squared = FloatExp::from(rad);
            point.small_time = n.saturating_sub(1) as u32;
        }
    }
    delta.dz = dz;
    point.iterations = skip.saturating_sub(1) as u32;
    let z_ref = pub_ref
        .orbit
        .get(point.iterations.saturating_add(1))
        .unwrap_or(ComplexFloatExp::ZERO);
    sync_point_from_delta(point, z_ref, dz, dd);
}

impl SeatKernel<FloatExp> for FloatExpPerturbationKernel {
    fn start_seat(&self, context: &mut WorkContext<FloatExp>, pos: (i32, i32)) {
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
                absolute_plane_c(context.points[index].c, &context.coord_anchor)
            } else {
                context.points[index].c
            };
            init_delta(&mut context.points[index], orbit, generation, abs_c);
            apply_series_skip(
                &mut context.points[index],
                context.latest_reference.as_deref(),
            );
        }
    }

    fn iterate_bout(
        &self,
        point: &mut Point<FloatExp>,
        reference: Option<&ReferenceOrbit>,
        r_squared: FloatExp,
        epsilon: FloatExp,
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
        let glitch_factor = FloatExp::from(1.0e-6);
        let four = FloatExp::from(4.0);
        let two = ComplexFloatExp::new(FloatExp::TWO, FloatExp::ZERO);
        let one = ComplexFloatExp::new(FloatExp::ONE, FloatExp::ZERO);
        let mut is_zero_ref = std::ptr::eq(orbit, zero_orbit());

        for _ in 0..cap.get() {
            let n = point.iterations.saturating_add(1);
            let z_ref = if is_zero_ref {
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

            let z = z_ref + delta.dz;
            let z_norm_sq = z.norm_squared();
            let z_ref_norm_sq = if is_zero_ref {
                FloatExp::ZERO
            } else {
                z_ref.norm_squared()
            };

            if !is_zero_ref
                && delta.dz != ComplexFloatExp::ZERO
                && z_ref_norm_sq == four
            {
                let correction = FloatExp::TWO
                    * (z_ref.re * delta.dz.re + z_ref.im * delta.dz.im)
                    + delta.dz.norm_squared();
                if correction > FloatExp::ZERO {
                    sync_point_from_delta(point, z_ref, delta.dz, delta.dd);
                    point.escapes = true;
                    point.delta = Some(delta);
                    return;
                }
                if correction == FloatExp::ZERO {
                    reset_for_glitch(point, against_generation.max(delta.generation));
                    return;
                }
            }

            if z_norm_sq > r_squared {
                sync_point_from_delta(point, z_ref, delta.dz, delta.dd);
                point.escapes = true;
                point.delta = Some(delta);
                return;
            }

            if !is_zero_ref
                && point.iterations > 0
                && z_norm_sq < z_ref_norm_sq * glitch_factor
            {
                reset_for_glitch(point, against_generation.max(delta.generation));
                return;
            }

            let rad = z_norm_sq.to_f64();
            if rad < point.smallness_squared.to_f64() {
                point.smallness_squared = FloatExp::from(rad);
                point.small_time = point.iterations;
            }

            delta.dd = z * delta.dd * two + one;
            delta.dz = z_ref * delta.dz * two + delta.dz * delta.dz + delta.dc;
            point.iterations = point.iterations.saturating_add(1);

            let z_next_ref = if is_zero_ref {
                ComplexFloatExp::ZERO
            } else {
                match orbit.get(point.iterations.saturating_add(1)) {
                    Some(z) => z,
                    None => {
                        let stamp = delta.generation;
                        sync_point_from_delta(point, z_ref, delta.dz, delta.dd);
                        rebind_to_zero_continuing(point, &mut delta, stamp);
                        orbit = zero_orbit();
                        is_zero_ref = true;
                        ComplexFloatExp::ZERO
                    }
                }
            };
            let z_next = z_next_ref + delta.dz;

            if near_complex(z_next, delta.checkpoint, epsilon) {
                sync_point_from_delta(point, z_next_ref, delta.dz, delta.dd);
                point.repeats = true;
                point.period = point.iterations.saturating_sub(delta.checkpoint_n);
                point.delta = Some(delta);
                return;
            }

            if point.iterations >= delta.checkpoint_n.saturating_mul(2).max(1) {
                delta.checkpoint = z_next;
                delta.checkpoint_n = point.iterations;
            }
        }

        let z_ref = if is_zero_ref {
            ComplexFloatExp::ZERO
        } else {
            orbit
                .get(point.iterations.saturating_add(1))
                .unwrap_or(ComplexFloatExp::ZERO)
        };
        sync_point_from_delta(point, z_ref, delta.dz, delta.dd);
        point.delta = Some(delta);
    }

    fn completion(&self, point: &mut Point<FloatExp>) -> CompletedPoint<FloatExp> {
        let saved = point.c;
        if let Some(delta) = point.delta.as_ref() {
            point.c = (delta.abs_c.re, delta.abs_c.im);
        }
        let out = direct_completion(point);
        point.c = saved;
        out
    }
}

/// Depth-test alias matching inventory expectations.
pub type PerturbationKernel = FloatExpPerturbationKernel;
