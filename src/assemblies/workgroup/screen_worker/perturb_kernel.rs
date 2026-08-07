//! Perturbation delta kernel — the sole production numerical path.
//!
//! Easier conditions (no published reference, post-glitch seats) change the
//! *reference* to the zero orbit; they never change the algorithm.

use std::sync::OnceLock;

use crate::floatexp::{ComplexFloatExp, FloatExp};
use crate::reference::ReferenceOrbit;

use super::workshift::{
    direct_completion, ensure_started, update_point_results, BoutCap, CompletedPoint, DeltaState,
    Point, SeatKernel, WorkContext,
};

/// The only production kernel. Always runs delta iteration.
// r[impl cz.depth.delta-kernel+1]
// r[impl cz.perf.one-kernel-path+1]
// r[impl cz.ref.zero-orbit-same-path+1]
#[derive(Clone, Copy, Debug, Default)]
pub struct PerturbationKernel;

fn zero_orbit() -> &'static ReferenceOrbit {
    static ZERO: OnceLock<ReferenceOrbit> = OnceLock::new();
    ZERO.get_or_init(ReferenceOrbit::zero_orbit)
}

fn to_delta_c(c: (f64, f64)) -> ComplexFloatExp {
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
        // Escaped references are proven bad (`depth-design.md`); never perturb against them.
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

#[inline(always)]
fn sync_point_from_delta(
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
fn near_complex(a: ComplexFloatExp, b: ComplexFloatExp, epsilon: FloatExp) -> bool {
    (a.re - b.re).abs() <= epsilon && (a.im - b.im).abs() <= epsilon
}

fn init_delta(point: &mut Point<f64>, orbit: &ReferenceOrbit, generation: u64) {
    let dc = to_delta_c(point.c) - reference_c_floatexp(orbit);

    // Match DirectKernel's z₀ = c convention: at iterations=0 we sit at
    // standard n=1 (Z₁ + δ₁ = c). For Mandelbrot Z₀=0, Z₁=c_ref ⇒ δ₁ = dc.
    let dz = dc;
    let dd = ComplexFloatExp::new(FloatExp::ONE, FloatExp::ZERO);
    let Some(z_ref) = orbit.get(1) else {
        point.delta = None;
        return;
    };
    point.delta = Some(DeltaState {
        dz,
        // Mirror Point's initial direct-kernel checkpoint at standard z₀ = 0.
        // The first post-advance comparison is z₂ against zero; then the
        // doubling schedule stores z₂ at codebase iteration 1.
        checkpoint: ComplexFloatExp::ZERO,
        checkpoint_n: 0,
        dc,
        dd,
        generation,
    });
    point.iterations = 0;
    point.escapes = false;
    point.repeats = false;
    point.period = 0;
    point.loop_detection_point = ((point.c.0, point.c.1), 0);
    point.smallness_squared = f64::MAX;
    point.small_time = 0;
    sync_point_from_delta(point, z_ref, dz, dd);
}

fn reset_for_glitch(point: &mut Point<f64>) {
    // r[impl cz.depth.glitch-is-unfinished+1]
    // r[impl cz.depth.perturb-never-wrong+1]
    point.direct_only = true;
    point.delta = None;
    point.initialized = false;
    point.iterations = 0;
    point.escapes = false;
    point.repeats = false;
    point.delivered = false;
}

impl SeatKernel<f64> for PerturbationKernel {
    fn start_seat(&self, context: &mut WorkContext<f64>, pos: (i32, i32)) {
        ensure_started(context, pos);
        let index = crate::utils::index_from_pos(&pos, context.res.0);
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
        // r[impl cz.depth.reference-generation-restart+1]
        if needs_restart {
            init_delta(&mut context.points[index], orbit, generation);
        }
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
        let Some(mut delta) = point.delta.take() else {
            return;
        };

        // Same FloatExp delta recurrence for every reference, including the
        // zero-orbit floor. Never branch to DirectKernel / iterate_max_n_times.
        let glitch_factor = FloatExp::from(1.0e-6);
        let r_sq = FloatExp::from(r_squared);
        let four = FloatExp::from(4.0);
        let eps = FloatExp::from(epsilon);
        let two = ComplexFloatExp::new(FloatExp::TWO, FloatExp::ZERO);
        let one = ComplexFloatExp::new(FloatExp::ONE, FloatExp::ZERO);

        for _ in 0..cap.get() {
            // Current z sits at standard index n = iterations+1.
            let n = point.iterations.saturating_add(1);
            let Some(z_ref) = orbit.get(n) else {
                point.delta = Some(delta);
                return;
            };

            let z = z_ref + delta.dz;
            let z_norm_sq = z.norm_squared();
            let z_ref_norm_sq = z_ref.norm_squared();

            if delta.dz != ComplexFloatExp::ZERO && z_ref_norm_sq == four {
                let correction = FloatExp::TWO * (z_ref.re * delta.dz.re + z_ref.im * delta.dz.im)
                    + delta.dz.norm_squared();
                if correction > FloatExp::ZERO {
                    sync_point_from_delta(point, z_ref, delta.dz, delta.dd);
                    point.escapes = true;
                    point.delta = Some(delta);
                    return;
                }
                if correction == FloatExp::ZERO {
                    reset_for_glitch(point);
                    return;
                }
            }

            if z_norm_sq > r_sq {
                sync_point_from_delta(point, z_ref, delta.dz, delta.dd);
                point.escapes = true;
                point.delta = Some(delta);
                return;
            }

            if point.iterations > 0 && z_norm_sq < z_ref_norm_sq * glitch_factor {
                reset_for_glitch(point);
                return;
            }

            // Track smallness on the pre-advance reconstruct (DirectKernel order).
            let z_f64 = (z.re.to_f64(), z.im.to_f64());
            let rad = z_f64.0 * z_f64.0 + z_f64.1 * z_f64.1;
            if rad < point.smallness_squared {
                point.smallness_squared = rad;
                point.small_time = point.iterations;
            }

            // Advance δ (same recurrence for zero orbit and published references).
            delta.dd = z * delta.dd * two + one;
            delta.dz = z_ref * delta.dz * two + delta.dz * delta.dz + delta.dc;
            point.iterations = point.iterations.saturating_add(1);

            let Some(z_next_ref) = orbit.get(point.iterations.saturating_add(1)) else {
                point.delta = Some(delta);
                return;
            };
            let z_next = z_next_ref + delta.dz;
            // One full sync after advance (point.z/dc match iterations).
            sync_point_from_delta(point, z_next_ref, delta.dz, delta.dd);

            if near_complex(z_next, delta.checkpoint, eps) {
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

        point.delta = Some(delta);
    }

    fn completion(&self, point: &mut Point<f64>) -> CompletedPoint<f64> {
        direct_completion(point)
    }
}
