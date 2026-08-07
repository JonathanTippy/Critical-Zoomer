//! Perturbation delta kernel — the sole production numerical path.
//!
//! Plane coordinates are `FloatExp` (no plain f64). Easier conditions (no
//! published reference, post-glitch seats) change the *reference* to the zero
//! orbit; they never change the algorithm.

use std::sync::OnceLock;

use crate::floatexp::{ComplexFloatExp, FloatExp};
use crate::reference::ReferenceOrbit;
use crate::utils::IntExp;

use super::workshift::{
    absolute_plane_c, direct_completion, ensure_started, update_point_results, BoutCap,
    CompletedPoint, DeltaState, Point, SeatKernel, WorkContext,
};

/// The only production kernel. Always runs delta iteration on FloatExp coords.
// r[impl cz.depth.delta-kernel+1]
// r[impl cz.perf.one-kernel-path+1]
// r[impl cz.ref.zero-orbit-same-path+1]
// r[impl cz.depth.floatexp-host-coords+1]
#[derive(Clone, Copy, Debug, Default)]
pub struct PerturbationKernel;

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
    // A newer published reference may reclaim seats that glitched or exhausted
    // against an older / short orbit. Same-generation binds stay on the floor.
    let gen = published_generation(published);
    if point.direct_only && gen != point.bound_zero_generation {
        point.direct_only = false;
        point.delta = None;
        point.initialized = false;
    }
}

fn reset_for_glitch(point: &mut Point<FloatExp>, against_generation: u64) {
    // r[impl cz.depth.glitch-is-unfinished+1]
    // r[impl cz.depth.perturb-never-wrong+1]
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
    // Exhausted published orbit: keep unfinished progress, switch δ to the
    // zero-orbit floor (Z≡0 ⇒ δz ≡ z, δc ≡ abs_c) and continue the bout.
    point.direct_only = true;
    point.bound_zero_generation = against_generation;
    delta.dc = delta.abs_c;
    delta.dz = ComplexFloatExp::new(point.z.0, point.z.1);
    delta.generation = 0;
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
        checkpoint: ComplexFloatExp::ZERO,
        checkpoint_n: 0,
        dc,
        abs_c: abs,
        dd,
        generation,
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

/// Apply a safe series skip when the published reference carries coeffs.
/// Never invents a final answer — only advances δz / iteration index.
// r[impl cz.depth.series-approximation+1]
fn apply_series_skip(
    point: &mut Point<FloatExp>,
    published: Option<&crate::assemblies::workgroup::reference_worker::PublishedReference>,
) {
    let Some(pub_ref) = published else {
        return;
    };
    if point.direct_only {
        return;
    }
    let Some(series) = pub_ref.series.as_ref() else {
        return;
    };
    // Escaped references are not used as the active orbit; their series must
    // not skip either (would advance δ against the wrong Z_n).
    if pub_ref.orbit.escaped {
        return;
    }
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
    // Derivative series omitted in v1 simple SA — keep dd as first-order approx
    // from δc (honest unfinished risk absorbed by glitch detector / soft-continue).
    delta.dz = dz;
    // Codebase iterations count advances from 0; standard index n = iterations+1 = skip.
    point.iterations = skip.saturating_sub(1) as u32;
    drop(delta);
    let z_ref = pub_ref
        .orbit
        .get(point.iterations.saturating_add(1))
        .unwrap_or(ComplexFloatExp::ZERO);
    let dz = point.delta.as_ref().map(|d| d.dz).unwrap_or(ComplexFloatExp::ZERO);
    sync_point_from_delta(point, z_ref, dz, dd);
}

impl SeatKernel<FloatExp> for PerturbationKernel {
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
        // r[impl cz.depth.reference-generation-restart+1]
        if needs_restart {
            let abs_c = absolute_plane_c(context.points[index].c, &context.coord_anchor);
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

        // Same FloatExp delta recurrence for every reference, including the
        // zero-orbit floor. Never branch to DirectKernel / iterate_max_n_times.
        let glitch_factor = FloatExp::from(1.0e-6);
        let r_sq = r_squared;
        let four = FloatExp::from(4.0);
        let eps = epsilon;
        let two = ComplexFloatExp::new(FloatExp::TWO, FloatExp::ZERO);
        let one = ComplexFloatExp::new(FloatExp::ONE, FloatExp::ZERO);
        // Period-1 zero reference is still the same recurrence; skip Option/modulo
        // get traffic on the static floor without forking the algorithm.
        let mut is_zero_ref = std::ptr::eq(orbit, zero_orbit());

        for _ in 0..cap.get() {
            // Current z sits at standard index n = iterations+1.
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
            // Zero-orbit |Z|² is identically 0; skip the multiply when possible.
            let z_ref_norm_sq = if is_zero_ref {
                FloatExp::ZERO
            } else {
                z_ref.norm_squared()
            };

            if !is_zero_ref && delta.dz != ComplexFloatExp::ZERO && z_ref_norm_sq == four {
                let correction = FloatExp::TWO * (z_ref.re * delta.dz.re + z_ref.im * delta.dz.im)
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

            if z_norm_sq > r_sq {
                sync_point_from_delta(point, z_ref, delta.dz, delta.dd);
                point.escapes = true;
                point.delta = Some(delta);
                return;
            }

            // Glitch sad-exit (kept): only meaningful when |Z_ref| is nonzero.
            if !is_zero_ref
                && point.iterations > 0
                && z_norm_sq < z_ref_norm_sq * glitch_factor
            {
                reset_for_glitch(point, against_generation.max(delta.generation));
                return;
            }

            // Track smallness in FloatExp (no plain-f64 coordinate mirror).
            if z_norm_sq < point.smallness_squared {
                point.smallness_squared = z_norm_sq;
                point.small_time = point.iterations;
            }

            // Advance δ (same recurrence for zero orbit and published references).
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
                        // Sync before rebind so point.z feeds δz ≡ z.
                        sync_point_from_delta(point, z_ref, delta.dz, delta.dd);
                        rebind_to_zero_continuing(point, &mut delta, stamp);
                        orbit = zero_orbit();
                        is_zero_ref = true;
                        ComplexFloatExp::ZERO
                    }
                }
            };
            let z_next = z_next_ref + delta.dz;

            if near_complex(z_next, delta.checkpoint, eps) {
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

        // One mirror sync at bout end (completion paths read FloatExp z/dc).
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
        // Period Newton needs absolute plane c; seats store relative-to-center.
        let saved = point.c;
        if let Some(delta) = point.delta.as_ref() {
            point.c = (delta.abs_c.re, delta.abs_c.im);
        }
        let out = direct_completion(point);
        point.c = saved;
        out
    }
}
