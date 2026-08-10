//! Compute gears for perturbation delta recurrence: f64, scaled-f64, FloatExp.
//!
//! Legal transitions: F64 → ScaledF64 → FloatExp. No silent underflow.

use crate::floatexp::{ComplexFloatExp, FloatExp};

// r[impl cz.depth.compute-gear+1]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ComputeGear {
    /// Naive GPU F32 compute (HUD only; perturbation never uses this).
    F32,
    #[default]
    F64,
    ScaledF64,
    FloatExp,
    Mixed,
}

impl ComputeGear {
    pub fn hud_label(self) -> &'static str {
        match self {
            ComputeGear::F32 => "F32",
            ComputeGear::F64 => "F64",
            ComputeGear::ScaledF64 => "S-F64",
            ComputeGear::FloatExp => "FE",
            ComputeGear::Mixed => "MIXED",
        }
    }
}

/// Floor below which f64 delta components must promote (not flush to zero).
const F64_UNDERFLOW_FLOOR: f64 = 1e-300;
/// Below this magnitude, plain f64 perturbation loses precision vs the reference; use scaled-f64.
/// Chosen above deep-view pixel pitch (2^-28 ≈ 3.7e-9 at pot 19) so filaments keep structure.
pub const F64_PERTURB_USEFUL_FLOOR: f64 = 1e-7;
/// Ceiling above which scaled-f64 inner values must rescale or promote.
const F64_OVERFLOW_CEIL: f64 = 1e300;

#[inline(always)]
fn cscale(a: (f64, f64), s: f64) -> (f64, f64) {
    (a.0 * s, a.1 * s)
}

#[inline(always)]
fn cmul(a: (f64, f64), b: (f64, f64)) -> (f64, f64) {
    (a.0 * b.0 - a.1 * b.1, a.0 * b.1 + a.1 * b.0)
}

#[inline(always)]
fn cadd(a: (f64, f64), b: (f64, f64)) -> (f64, f64) {
    (a.0 + b.0, a.1 + b.1)
}

#[inline(always)]
fn cnorm_sq(a: (f64, f64)) -> f64 {
    a.0 * a.0 + a.1 * a.1
}

#[inline(always)]
fn fe_to_f64_pair(z: ComplexFloatExp) -> Option<(f64, f64)> {
    let re = z.re.to_f64();
    let im = z.im.to_f64();
    if re.is_finite() && im.is_finite() {
        Some((re, im))
    } else {
        None
    }
}

#[inline(always)]
fn fe_to_f64_or_zero(z: FloatExp) -> f64 {
    let v = z.to_f64();
    if v.is_finite() {
        v
    } else {
        0.0
    }
}

/// Pick the view's default gear from generator admission (no user setting).
pub fn view_gear_from_generators<T: crate::assemblies::workgroup::c_generator::Mandelbrotable>(
    relative_ok: bool,
) -> ComputeGear {
    if relative_ok {
        // Shallow / moderate views: f64 relative grid is admitted.
        if T::max_value().to_f64() > 1e200 {
            ComputeGear::F64
        } else {
            ComputeGear::FloatExp
        }
    } else {
        ComputeGear::FloatExp
    }
}

/// Whether a complex pair is safely representable as hardware f64 deltas.
pub fn f64_delta_admitted(re: f64, im: f64) -> bool {
    let m = re.abs().max(im.abs());
    m == 0.0 || (m >= F64_UNDERFLOW_FLOOR && m <= F64_OVERFLOW_CEIL)
}

#[inline]
fn pair_magnitude(v: (f64, f64)) -> f64 {
    v.0.abs().max(v.1.abs())
}

/// Plain f64 perturbation is accurate enough at this delta magnitude.
fn f64_perturbation_useful(re: f64, im: f64) -> bool {
    let m = re.abs().max(im.abs());
    m == 0.0 || m >= F64_PERTURB_USEFUL_FLOOR
}

/// Promote FloatExp complex delta to the strongest gear that still admits it.
pub fn gear_for_delta(dc: ComplexFloatExp, dz: ComplexFloatExp) -> ComputeGear {
    let dc_f = fe_to_f64_pair(dc);
    let dz_f = fe_to_f64_pair(dz);
    match (dc_f, dz_f) {
        (Some(dc), Some(dz))
            if f64_delta_admitted(dc.0, dc.1) && f64_delta_admitted(dz.0, dz.1) =>
        {
            let m = pair_magnitude(dc).max(pair_magnitude(dz));
            if m > 0.0
                && (!f64_perturbation_useful(dc.0, dc.1)
                    || !f64_perturbation_useful(dz.0, dz.1))
            {
                ComputeGear::ScaledF64
            } else {
                ComputeGear::F64
            }
        }
        _ => {
            // Scaled-f64 when inner f64 can hold scaled w after choosing scale from dz.
            if let Some(dz_f) = dz_f {
                let scale = scale_for_pair(dz_f);
                if scale.mantissa != 0.0 {
                    return ComputeGear::ScaledF64;
                }
            }
            ComputeGear::FloatExp
        }
    }
}

fn scale_for_pair(v: (f64, f64)) -> FloatExp {
    let m = v.0.abs().max(v.1.abs());
    if m == 0.0 {
        return FloatExp::ONE;
    }
    FloatExp::from(m)
}

/// Scaled-f64 scale from current mathematical dz.
pub fn scaled_scale_from_dz(dz: ComplexFloatExp) -> FloatExp {
    let re = dz.re.abs();
    let im = dz.im.abs();
    let m = if re > im { re } else { im };
    if m == FloatExp::ZERO {
        FloatExp::ONE
    } else {
        m
    }
}

/// One scaled-f64 step: w' = 2·Z·w + S·w² + d  (d = δc/S).
/// Returns (new_w, new_scale, next_gear). Mathematical δz = S·w is reconstructed by caller.
pub fn scaled_f64_step(
    z_ref: (f64, f64),
    w: (f64, f64),
    d: (f64, f64),
    scale: FloatExp,
    is_zero_ref: bool,
) -> ((f64, f64), FloatExp, ComputeGear) {
    let s = scale.to_f64();
    if !s.is_finite() || s == 0.0 {
        return (w, scale, ComputeGear::FloatExp);
    }
    let two_zw = if is_zero_ref {
        (0.0, 0.0)
    } else {
        cscale(cmul(z_ref, w), 2.0)
    };
    // When S underflows in f64, skip S·w² (dead term decided at rescale time).
    let sw2 = if s.abs() < F64_UNDERFLOW_FLOOR {
        (0.0, 0.0)
    } else {
        let w2 = cmul(w, w);
        (w2.0 * s, w2.1 * s)
    };
    let w_next = cadd(cadd(two_zw, sw2), d);
    let mag = w_next.0.abs().max(w_next.1.abs());
    if !mag.is_finite() {
        return (w_next, scale, ComputeGear::FloatExp);
    }
    if mag > F64_OVERFLOW_CEIL {
        // Rescale: pick new S' so |w'| ≈ 1, promote only if new scale cannot help.
        let new_scale = FloatExp::from(mag) * scale;
        let inv = 1.0 / mag;
        let w_rescaled = (w_next.0 * inv, w_next.1 * inv);
        if !f64_delta_admitted(w_rescaled.0, w_rescaled.1) {
            return (w_next, scale, ComputeGear::FloatExp);
        }
        return (w_rescaled, new_scale, ComputeGear::ScaledF64);
    }
    if mag > 0.0 && mag < 1e-8 {
        // Bring |w| back near 1 when it drifts too small.
        let new_scale = FloatExp::from(mag) * scale;
        let inv = 1.0 / mag;
        return (
            (w_next.0 * inv, w_next.1 * inv),
            new_scale,
            ComputeGear::ScaledF64,
        );
    }
    (w_next, scale, ComputeGear::ScaledF64)
}

/// One f64-hardware step. Returns promoted gear if f64 cannot continue.
pub fn f64_step(
    z_ref: (f64, f64),
    dz: (f64, f64),
    dc: (f64, f64),
    dd: (f64, f64),
    is_zero_ref: bool,
) -> ((f64, f64), (f64, f64), ComputeGear) {
    let z = cadd(z_ref, dz);
    let two_zdz = if is_zero_ref {
        (0.0, 0.0)
    } else {
        cscale(cmul(z_ref, dz), 2.0)
    };
    let dz2 = cmul(dz, dz);
    let dz_next = cadd(cadd(two_zdz, dz2), dc);
    let dd_next = cadd(cscale(cmul(z, dd), 2.0), (1.0, 0.0));
    if !f64_delta_admitted(dz_next.0, dz_next.1) || !f64_delta_admitted(dc.0, dc.1) {
        return (dz_next, dd_next, ComputeGear::ScaledF64);
    }
    let m = pair_magnitude(dz_next).max(pair_magnitude(dc));
    if m > 0.0
        && (!f64_perturbation_useful(dz_next.0, dz_next.1)
            || !f64_perturbation_useful(dc.0, dc.1))
    {
        return (dz_next, dd_next, ComputeGear::ScaledF64);
    }
    (dz_next, dd_next, ComputeGear::F64)
}

/// Aggregate per-seat gears for HUD (honest mixed display).
pub fn aggregate_seat_gears(gears: &[ComputeGear]) -> ComputeGear {
    let mut saw_f64 = false;
    let mut saw_scaled = false;
    let mut saw_fe = false;
    for g in gears {
        match g {
            ComputeGear::F32 => {} // naive-GPU HUD only; ignore in pert aggregates
            ComputeGear::F64 => saw_f64 = true,
            ComputeGear::ScaledF64 => saw_scaled = true,
            ComputeGear::FloatExp => saw_fe = true,
            ComputeGear::Mixed => return ComputeGear::Mixed,
        }
    }
    let count = saw_f64 as u8 + saw_scaled as u8 + saw_fe as u8;
    if count > 1 {
        ComputeGear::Mixed
    } else if saw_fe {
        ComputeGear::FloatExp
    } else if saw_scaled {
        ComputeGear::ScaledF64
    } else {
        ComputeGear::F64
    }
}

/// Narrow reference iterate to f64 when safe; None forces seat promotion.
pub fn narrow_z_ref(z: ComplexFloatExp) -> Option<(f64, f64)> {
    let re = z.re.to_f64();
    let im = z.im.to_f64();
    if re.is_finite() && im.is_finite() {
        Some((re, im))
    } else {
        None
    }
}

pub fn floatexp_from_f64_pair(v: (f64, f64)) -> ComplexFloatExp {
    ComplexFloatExp::new(FloatExp::from(v.0), FloatExp::from(v.1))
}

pub fn f64_from_fe(z: ComplexFloatExp) -> (f64, f64) {
    (fe_to_f64_or_zero(z.re), fe_to_f64_or_zero(z.im))
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    // r[verify cz.depth.compute-gear+1]
    #[test]
    fn gear_uses_scaled_f64_below_perturb_useful_floor() {
        let tiny = ComplexFloatExp::new(FloatExp::from(1e-10), FloatExp::ZERO);
        assert_eq!(gear_for_delta(tiny, tiny), ComputeGear::ScaledF64);
        let homeish = ComplexFloatExp::new(FloatExp::from(1e-4), FloatExp::ZERO);
        assert_eq!(gear_for_delta(homeish, homeish), ComputeGear::F64);
    }

    // r[verify cz.depth.compute-gear+1]
    #[test]
    fn gear_promotes_from_view_pitch() {
        // Pixel pitch at pot 19: 2^-(19+9) = 2^-28.
        let pitch = 2f64.powi(-28);
        assert!(pitch < F64_PERTURB_USEFUL_FLOOR);
        let dc = ComplexFloatExp::new(FloatExp::from(pitch), FloatExp::ZERO);
        assert_eq!(gear_for_delta(dc, dc), ComputeGear::ScaledF64);
    }

    // r[verify cz.depth.compute-gear+1]
    #[test]
    fn gear_promotes_at_f64_underflow_floor() {
        assert!(!f64_delta_admitted(1e-301, 0.0));
        let tiny = ComplexFloatExp::new(FloatExp::from(1e-320), FloatExp::ZERO);
        assert_ne!(gear_for_delta(tiny, tiny), ComputeGear::F64);
    }

    // r[verify cz.depth.compute-gear+1]
    #[test]
    fn zero_orbit_f64_skips_two_z_term() {
        let (dz, dd, gear) = f64_step((0.0, 0.0), (0.1, 0.0), (0.1, 0.0), (1.0, 0.0), true);
        // δz' = δz² + δc = 0.01 + 0.1 = 0.11 when Z=0 (no 2Zδz).
        assert!((dz.0 - 0.11).abs() < 1e-12);
        assert_eq!(gear, ComputeGear::F64);
        let _ = dd;
    }

    // r[verify cz.depth.compute-gear+1]
    #[test]
    fn scaled_f64_matches_floatexp_on_moderate_delta() {
        let z_ref = (0.25, 0.0);
        let dz0 = ComplexFloatExp::new(FloatExp::from(1e-40), FloatExp::ZERO);
        let dc = dz0;
        let scale = scaled_scale_from_dz(dz0);
        let s = scale.to_f64();
        let w = (dz0.re.to_f64() / s, 0.0);
        let d = (dc.re.to_f64() / s, 0.0);
        let (w1, scale1, gear) = scaled_f64_step(z_ref, w, d, scale, false);
        assert_eq!(gear, ComputeGear::ScaledF64);
        let dz1 = w1.0 * scale1.to_f64();
        // FloatExp reference step: δz' = 2 Z δz + δz² + δc
        let z_fe = ComplexFloatExp::new(FloatExp::from(z_ref.0), FloatExp::ZERO);
        let two = ComplexFloatExp::new(FloatExp::TWO, FloatExp::ZERO);
        let dz_fe = z_fe * dz0 * two + dz0 * dz0 + dc;
        let expected = dz_fe.re.to_f64();
        assert!(
            (dz1 - expected).abs() / expected.abs().max(1e-300) < 1e-6,
            "scaled {dz1} vs fe {expected}"
        );
    }

    // r[verify cz.depth.compute-gear+1]
    #[test]
    fn aggregate_seat_gears_reports_mixed() {
        assert_eq!(
            aggregate_seat_gears(&[ComputeGear::F64, ComputeGear::ScaledF64]),
            ComputeGear::Mixed
        );
        assert_eq!(
            aggregate_seat_gears(&[ComputeGear::F64, ComputeGear::F64]),
            ComputeGear::F64
        );
    }

    /// Thought-killed pins for the dense `delta_gear` caught-mutant cluster
    /// (`mutants.out/caught.txt`: cmul/cadd/cnorm/cscale/scaled_f64_step/…).
    #[test]
    fn complex_helpers_kill_arithmetic_mutants() {
        assert_eq!(cscale((3.0, -4.0), 2.0), (6.0, -8.0));
        assert_ne!(cscale((3.0, -4.0), 2.0), (3.0 + 2.0, -4.0 + 2.0)); // *→+
        assert_ne!(cscale((3.0, -4.0), 2.0), (3.0 / 2.0, -4.0 / 2.0)); // *→/

        // (1+2i)(3+4i) = -5 + 10i
        assert_eq!(cmul((1.0, 2.0), (3.0, 4.0)), (-5.0, 10.0));
        assert_ne!(cmul((1.0, 2.0), (3.0, 4.0)), (1.0 * 3.0 + 2.0 * 4.0, 0.0));

        assert_eq!(cadd((1.0, 2.0), (3.0, 4.0)), (4.0, 6.0));
        assert_ne!(cadd((1.0, 2.0), (3.0, 4.0)), (1.0 - 3.0, 2.0 - 4.0));
        assert_ne!(cadd((1.0, 2.0), (3.0, 4.0)), (1.0 * 3.0, 2.0 * 4.0));

        assert_eq!(cnorm_sq((3.0, 4.0)), 25.0);
        assert_ne!(cnorm_sq((3.0, 4.0)), 3.0 * 3.0 - 4.0 * 4.0);
        assert_ne!(cnorm_sq((3.0, 4.0)), 3.0 + 4.0);

        assert_eq!(pair_magnitude((-2.0, 5.0)), 5.0);
        assert_eq!(hud_labels_are_distinct(), true);
    }

    fn hud_labels_are_distinct() -> bool {
        let labels = [
            ComputeGear::F32.hud_label(),
            ComputeGear::F64.hud_label(),
            ComputeGear::ScaledF64.hud_label(),
            ComputeGear::FloatExp.hud_label(),
            ComputeGear::Mixed.hud_label(),
        ];
        labels.iter().collect::<std::collections::BTreeSet<_>>().len() == labels.len()
    }

    #[test]
    fn f64_admission_and_useful_floors() {
        assert!(f64_delta_admitted(0.0, 0.0));
        assert!(f64_delta_admitted(1e-200, 0.0));
        assert!(!f64_delta_admitted(1e-301, 0.0));
        assert!(!f64_delta_admitted(1e301, 0.0));
        // Kill ==→!= on the zero short-circuit and ||→&& / threshold flips.
        assert!(f64_perturbation_useful(0.0, 0.0));
        assert!(f64_perturbation_useful(1e-6, 0.0));
        assert!(!f64_perturbation_useful(1e-8, 0.0));
        assert!(f64_perturbation_useful(F64_PERTURB_USEFUL_FLOOR, 0.0));
        assert!(!f64_perturbation_useful(F64_PERTURB_USEFUL_FLOOR * 0.5, 0.0));
    }

    #[test]
    fn f64_step_applies_two_z_term_when_ref_nonzero() {
        let z_ref = (0.5, 0.0);
        let dz = (0.1, 0.0);
        let dc = (0.01, 0.0);
        let (dz_on, _, gear_on) = f64_step(z_ref, dz, dc, (1.0, 0.0), false);
        let (dz_off, _, gear_off) = f64_step(z_ref, dz, dc, (1.0, 0.0), true);
        // With Z: 2*0.5*0.1 + 0.01 + 0.01 = 0.12; without: 0.01+0.01=0.02
        assert!((dz_on.0 - 0.12).abs() < 1e-12, "got {}", dz_on.0);
        assert!((dz_off.0 - 0.02).abs() < 1e-12, "got {}", dz_off.0);
        assert_ne!(dz_on.0, dz_off.0);
        assert_eq!(gear_on, ComputeGear::F64);
        assert_eq!(gear_off, ComputeGear::F64);
    }

    #[test]
    fn f64_step_promotes_when_delta_becomes_useless() {
        let (dz, _, gear) = f64_step(
            (0.0, 0.0),
            (1e-9, 0.0),
            (1e-9, 0.0),
            (1.0, 0.0),
            true,
        );
        assert!(dz.0.abs() < F64_PERTURB_USEFUL_FLOOR);
        assert_eq!(gear, ComputeGear::ScaledF64);
    }

    #[test]
    fn scaled_f64_step_rescales_large_and_small_w() {
        let scale = FloatExp::ONE;
        // |w| large enough that |S·w²| is finite but > F64_OVERFLOW_CEIL → rescale.
        let (w_big, s_big, g_big) =
            scaled_f64_step((0.0, 0.0), (1e152, 0.0), (0.0, 0.0), scale, true);
        assert_eq!(g_big, ComputeGear::ScaledF64);
        assert!((w_big.0.abs() - 1.0).abs() < 1e-9, "w={}", w_big.0);
        assert!(s_big.to_f64().is_finite() && s_big.to_f64() > 1.0);

        // Tiny |w| (but above underflow) → bring back near 1.
        let (w_tiny, s_tiny, g_tiny) =
            scaled_f64_step((0.0, 0.0), (1e-10, 0.0), (0.0, 0.0), FloatExp::ONE, true);
        assert_eq!(g_tiny, ComputeGear::ScaledF64);
        assert!((w_tiny.0.abs() - 1.0).abs() < 1e-6, "w={}", w_tiny.0);
        assert!(s_tiny.to_f64() < 1.0);

        // Non-finite / zero scale → FloatExp promotion.
        let (_, _, g_bad) =
            scaled_f64_step((0.0, 0.0), (1.0, 0.0), (0.0, 0.0), FloatExp::from(0.0), true);
        assert_eq!(g_bad, ComputeGear::FloatExp);
    }

    #[test]
    fn scaled_f64_step_matches_algebra_on_unit_scale() {
        // is_zero_ref: w' = S·w² + d with S=1 → w² + d
        let (w1, s1, g) = scaled_f64_step((0.0, 0.0), (0.5, 0.25), (0.1, -0.2), FloatExp::ONE, true);
        assert_eq!(g, ComputeGear::ScaledF64);
        assert_eq!(s1, FloatExp::ONE);
        let w2 = cmul((0.5, 0.25), (0.5, 0.25));
        let expect = cadd(w2, (0.1, -0.2));
        assert!((w1.0 - expect.0).abs() < 1e-12);
        assert!((w1.1 - expect.1).abs() < 1e-12);
    }

    #[test]
    fn gear_for_delta_requires_both_admitted_for_f64() {
        let ok = ComplexFloatExp::new(FloatExp::from(1e-4), FloatExp::ZERO);
        let underflow = ComplexFloatExp::new(FloatExp::from(1e-320), FloatExp::ZERO);
        // One side not admitted → not plain F64 (&& mutant would incorrectly stay F64).
        assert_ne!(gear_for_delta(ok, underflow), ComputeGear::F64);
        assert_ne!(gear_for_delta(underflow, ok), ComputeGear::F64);
        assert_eq!(gear_for_delta(ok, ok), ComputeGear::F64);
    }

    #[test]
    fn fe_pair_helpers_and_narrow() {
        let z = ComplexFloatExp::new(FloatExp::from(1.25), FloatExp::from(-0.5));
        assert_eq!(fe_to_f64_pair(z), Some((1.25, -0.5)));
        assert_eq!(f64_from_fe(z), (1.25, -0.5));
        assert_eq!(narrow_z_ref(z), Some((1.25, -0.5)));
        let round = floatexp_from_f64_pair((2.0, 3.0));
        assert_eq!(f64_from_fe(round), (2.0, 3.0));
        assert_eq!(fe_to_f64_or_zero(FloatExp::from(7.0)), 7.0);
        // Non-finite → 0 for fe_to_f64_or_zero
        let huge = FloatExp::new(1.0, 10_000);
        assert_eq!(fe_to_f64_or_zero(huge), 0.0);
    }

    #[test]
    fn aggregate_covers_all_single_and_mixed_paths() {
        assert_eq!(aggregate_seat_gears(&[]), ComputeGear::F64);
        assert_eq!(
            aggregate_seat_gears(&[ComputeGear::FloatExp]),
            ComputeGear::FloatExp
        );
        assert_eq!(
            aggregate_seat_gears(&[ComputeGear::ScaledF64]),
            ComputeGear::ScaledF64
        );
        assert_eq!(
            aggregate_seat_gears(&[ComputeGear::F64, ComputeGear::FloatExp]),
            ComputeGear::Mixed
        );
        assert_eq!(
            aggregate_seat_gears(&[ComputeGear::Mixed]),
            ComputeGear::Mixed
        );
        // F32 ignored in pert aggregates.
        assert_eq!(
            aggregate_seat_gears(&[ComputeGear::F32, ComputeGear::F64]),
            ComputeGear::F64
        );
    }

    #[test]
    fn scaled_scale_from_dz_picks_max_abs_component() {
        let dz = ComplexFloatExp::new(FloatExp::from(1e-20), FloatExp::from(-3e-20));
        let s = scaled_scale_from_dz(dz);
        assert_eq!(s, FloatExp::from(3e-20));
        assert_eq!(
            scaled_scale_from_dz(ComplexFloatExp::new(FloatExp::ZERO, FloatExp::ZERO)),
            FloatExp::ONE
        );
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(128))]

        #[test]
        fn cmul_agrees_with_schoolbook(
            a0 in -1e3f64..1e3,
            a1 in -1e3f64..1e3,
            b0 in -1e3f64..1e3,
            b1 in -1e3f64..1e3,
        ) {
            let got = cmul((a0, a1), (b0, b1));
            prop_assert!((got.0 - (a0 * b0 - a1 * b1)).abs() < 1e-9);
            prop_assert!((got.1 - (a0 * b1 + a1 * b0)).abs() < 1e-9);
        }

        #[test]
        fn f64_step_zero_ref_is_dz2_plus_dc(
            dz0 in -1e2f64..1e2,
            dz1 in -1e2f64..1e2,
            dc0 in -1e2f64..1e2,
            dc1 in -1e2f64..1e2,
        ) {
            let (dz, _, _) = f64_step((0.0, 0.0), (dz0, dz1), (dc0, dc1), (1.0, 0.0), true);
            let expect = cadd(cmul((dz0, dz1), (dz0, dz1)), (dc0, dc1));
            prop_assert!((dz.0 - expect.0).abs() < 1e-6);
            prop_assert!((dz.1 - expect.1).abs() < 1e-6);
        }
    }
}
