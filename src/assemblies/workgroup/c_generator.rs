use std::ops::{Add, Mul, Sub};

use crate::constants::PIXELS_PER_UNIT_POT;
use crate::utils::IntExp;

/// Default C-generator render headroom beyond neighbor distinguishability.
/// Headed: 1 bit looks marginally better at type walls than 0; slider stays.
pub const DEFAULT_C_GENERATOR_MARGIN_BITS: u32 = 1;

/// IEEE binary32 significand (23 stored + implicit 1). Naive GPU F32 must
/// not run when [`stencil_bits_needed`] exceeds this.
pub const F32_SIGNIFICAND_BITS: u32 = 24;

/// Precision carried by a Mandelbrot host type. The C-generator gate is this
/// count: admit iff `significand_bits` covers |c| magnitude down to pixel pitch
/// and the pitch exponent is at or above `min_exponent`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HostPrecision {
    pub significand_bits: u32,
    /// Smallest binary exponent a finite value can hold (normals for IEEE).
    pub min_exponent: i32,
}

impl HostPrecision {
    pub const F32: Self = Self {
        significand_bits: F32_SIGNIFICAND_BITS,
        min_exponent: -126,
    };
    pub const F64: Self = Self {
        significand_bits: 53,
        min_exponent: -1022,
    };
    /// Same 53-bit mantissa as f64; exponent is not IEEE-bounded.
    pub const FLOAT_EXP: Self = Self {
        significand_bits: 53,
        min_exponent: i32::MIN,
    };
}

/// Binary exponent of `|x|` (`floor(log2(|x|))`), or `None` if zero.
pub fn intexp_magnitude_exp(x: &IntExp) -> Option<i32> {
    if x.val.is_zero() {
        return None;
    }
    let bits = x.val.significant_bits();
    if bits == 0 {
        return None;
    }
    Some((bits as i32) - 1 + x.exp)
}

/// Bits to keep neighboring seats distinct: from the MSB of |c| (near and far
/// on both axes) down to pixel pitch `2^(-(zoom+ppu))`, plus `margin_bits`.
pub fn stencil_bits_needed(
    loc: &(IntExp, IntExp),
    zoom_pot: i64,
    res: (u32, u32),
    margin_bits: u32,
) -> u32 {
    let pitch_exp = -(zoom_pot as i32).saturating_add(PIXELS_PER_UNIT_POT);
    let space = IntExp::from(1).shift(pitch_exp);
    let axis = |origin: &IntExp, count: u32, increasing: bool| -> u32 {
        if count <= 1 {
            return 0;
        }
        let span = space.clone() * IntExp::from((count - 1) as i32);
        let far = if increasing {
            origin.clone() + span
        } else {
            origin.clone() - span
        };
        let mag = [intexp_magnitude_exp(origin), intexp_magnitude_exp(&far)]
            .into_iter()
            .flatten()
            .max()
            .unwrap_or(pitch_exp);
        (mag as i64 - pitch_exp as i64 + 1).max(0) as u32
    };
    axis(&loc.0, res.0, true)
        .max(axis(&loc.1, res.1, false))
        .saturating_add(margin_bits)
}

fn type_covers_stencil<T: Mandelbrotable>(
    loc: &(IntExp, IntExp),
    zoom_pot: i64,
    res: (u32, u32),
    margin_bits: u32,
) -> Option<u32> {
    let pitch_exp = -(zoom_pot as i32).saturating_add(PIXELS_PER_UNIT_POT);
    if pitch_exp < T::PRECISION.min_exponent {
        return None;
    }
    let needed = stencil_bits_needed(loc, zoom_pot, res, margin_bits);
    if T::PRECISION.significand_bits < needed {
        return None;
    }
    Some(needed)
}

/// Numeric host type for CPU Mandelbrot arithmetic.
///
/// `PRECISION` is the admit gate. `From<IntExp>` is only used after that gate
/// to store `origin`/`space`.
pub trait Mandelbrotable:
    Copy
    + PartialEq
    + PartialOrd
    + Add<Output = Self>
    + Sub<Output = Self>
    + Mul<Output = Self>
    + From<IntExp>
    + std::fmt::Debug
{
    const ZERO: Self;
    const ONE: Self;
    const TWO: Self;
    const PRECISION: HostPrecision;

    fn from_u32(value: u32) -> Self;
    fn from_f32(value: f32) -> Self;
    fn to_f64(self) -> f64;
    fn abs(self) -> Self;
    fn neg(self) -> Self;
    fn max_value() -> Self;
    fn is_finite(self) -> bool;
}

impl Mandelbrotable for f64 {
    const ZERO: Self = 0.0;
    const ONE: Self = 1.0;
    const TWO: Self = 2.0;
    const PRECISION: HostPrecision = HostPrecision::F64;

    fn from_u32(value: u32) -> Self {
        value as f64
    }
    fn from_f32(value: f32) -> Self {
        value as f64
    }
    fn to_f64(self) -> f64 {
        self
    }
    fn abs(self) -> Self {
        self.abs()
    }
    fn neg(self) -> Self {
        -self
    }
    fn max_value() -> Self {
        f64::MAX
    }
    fn is_finite(self) -> bool {
        self.is_finite()
    }
}

impl Mandelbrotable for f32 {
    const ZERO: Self = 0.0;
    const ONE: Self = 1.0;
    const TWO: Self = 2.0;
    const PRECISION: HostPrecision = HostPrecision::F32;

    fn from_u32(value: u32) -> Self {
        value as f32
    }
    fn from_f32(value: f32) -> Self {
        value
    }
    fn to_f64(self) -> f64 {
        self as f64
    }
    fn abs(self) -> Self {
        self.abs()
    }
    fn neg(self) -> Self {
        -self
    }
    fn max_value() -> Self {
        f32::MAX
    }
    fn is_finite(self) -> bool {
        self.is_finite()
    }
}

/// Admitted generator for one frame: absolute plane coords, or relative to an
/// IntExp anchor (reference `c` when installed, else view center).
#[derive(Clone, Debug)]
pub enum GeneratorAdmission<T: Mandelbrotable> {
    Absolute(CGenerator<T>),
    Relative {
        generator: CGenerator<T>,
        anchor: (IntExp, IntExp),
    },
}

impl<T: Mandelbrotable> GeneratorAdmission<T> {
    pub fn is_relative(&self) -> bool {
        matches!(self, Self::Relative { .. })
    }

    pub fn generator(&self) -> &CGenerator<T> {
        match self {
            Self::Absolute(g) | Self::Relative { generator: g, .. } => g,
        }
    }
}

/// Shallowest host stack whose generator admits this stencil.
#[derive(Clone, Debug)]
pub enum AdmittedHostStack {
    F32(GeneratorAdmission<f32>),
    F64(GeneratorAdmission<f64>),
    CopyIntExp1(GeneratorAdmission<crate::copy_intexp::CopyIntExp1>),
    FloatExp(GeneratorAdmission<crate::floatexp::FloatExp>),
}

/// O(1) admission: absolute first, then relative to `relative_anchor` or
/// `view_center` when no anchor is supplied.
///
/// For live f64, when absolute still admits but pixel pitch is near the f64
/// ulp wall (~pot 43 at |c|~1), prefer relative-to-center so perturbation is
/// hard-bumped before absolute collapse (issue #5).
///
/// `margin_bits` is render headroom beyond neighbor distinguishability
/// (`DEFAULT_C_GENERATOR_MARGIN_BITS` in production; settings may override).
// r[impl cz.depth.c-generator-fails-closed+1]
pub fn admit_generator<T: Mandelbrotable>(
    compute_loc: &(IntExp, IntExp),
    zoom_pot: i64,
    res: (u32, u32),
    relative_anchor: Option<&(IntExp, IntExp)>,
    view_center: &(IntExp, IntExp),
) -> Option<GeneratorAdmission<T>> {
    admit_generator_with_margin(
        compute_loc,
        zoom_pot,
        res,
        relative_anchor,
        view_center,
        DEFAULT_C_GENERATOR_MARGIN_BITS,
    )
}

/// Same as [`admit_generator`] with an explicit render-margin bit count.
pub fn admit_generator_with_margin<T: Mandelbrotable>(
    compute_loc: &(IntExp, IntExp),
    zoom_pot: i64,
    res: (u32, u32),
    relative_anchor: Option<&(IntExp, IntExp)>,
    view_center: &(IntExp, IntExp),
    margin_bits: u32,
) -> Option<GeneratorAdmission<T>> {
    const ABSOLUTE_F64_RISKY_PITCH: f64 = 1e-14;
    let anchor = relative_anchor
        .map(|a| (a.0.clone(), a.1.clone()))
        .unwrap_or_else(|| (view_center.0.clone(), view_center.1.clone()));
    if let Some(generator) = CGenerator::<T>::new_with_margin(compute_loc, zoom_pot, res, margin_bits)
    {
        let (_, space) = generator.origin_and_space();
        let pitch = space.abs().to_f64();
        // Prefer relative when absolute still admits but pitch is near the f64
        // ulp wall (~pot 43 at |c|~1) so live f64 hard-bumps perturbation earlier.
        // Naive live path must not call this after absolute+margin fail — it
        // uses `CGenerator::new_with_margin` only (no view-center rescue).
        if pitch > 0.0 && pitch < ABSOLUTE_F64_RISKY_PITCH {
            if let Some(generator) = CGenerator::<T>::new_relative_with_margin(
                compute_loc,
                &anchor,
                zoom_pot,
                res,
                margin_bits,
            ) {
                return Some(GeneratorAdmission::Relative { generator, anchor });
            }
        }
        return Some(GeneratorAdmission::Absolute(generator));
    }
    CGenerator::<T>::new_relative_with_margin(compute_loc, &anchor, zoom_pot, res, margin_bits)
        .map(|generator| GeneratorAdmission::Relative { generator, anchor })
}

/// f64 before FloatExp — stack order for stencil admission probes.
pub fn pick_stack_admission(
    compute_loc: &(IntExp, IntExp),
    zoom_pot: i64,
    res: (u32, u32),
    relative_anchor: Option<&(IntExp, IntExp)>,
    view_center: &(IntExp, IntExp),
) -> Option<AdmittedHostStack> {
    pick_stack_admission_with_margin(
        compute_loc,
        zoom_pot,
        res,
        relative_anchor,
        view_center,
        DEFAULT_C_GENERATOR_MARGIN_BITS,
    )
}

/// Same as [`pick_stack_admission`] with an explicit render-margin bit count.
pub fn pick_stack_admission_with_margin(
    compute_loc: &(IntExp, IntExp),
    zoom_pot: i64,
    res: (u32, u32),
    relative_anchor: Option<&(IntExp, IntExp)>,
    view_center: &(IntExp, IntExp),
    margin_bits: u32,
) -> Option<AdmittedHostStack> {
    if let Some(admission) = admit_generator_with_margin::<f32>(
        compute_loc,
        zoom_pot,
        res,
        relative_anchor,
        view_center,
        margin_bits,
    ) {
        return Some(AdmittedHostStack::F32(admission));
    }
    if let Some(admission) = admit_generator_with_margin::<f64>(
        compute_loc,
        zoom_pot,
        res,
        relative_anchor,
        view_center,
        margin_bits,
    ) {
        return Some(AdmittedHostStack::F64(admission));
    }
    if let Some(admission) = admit_generator_with_margin::<crate::copy_intexp::CopyIntExp1>(
        compute_loc,
        zoom_pot,
        res,
        relative_anchor,
        view_center,
        margin_bits,
    ) {
        return Some(AdmittedHostStack::CopyIntExp1(admission));
    }
    admit_generator_with_margin::<crate::floatexp::FloatExp>(
        compute_loc,
        zoom_pot,
        res,
        relative_anchor,
        view_center,
        margin_bits,
    )
    .map(AdmittedHostStack::FloatExp)
}

/// Fail-closed objective-coordinate to compute-coordinate conversion.
///
/// The grid follows v0.0.9 exactly: `origin` is the top-left sample, +seat is
/// +real, +row is -imag, and there is no half-pixel offset.
///
/// Admission is O(1) bit counting against [`Mandelbrotable::PRECISION`]; the
/// hot `get_c` loop never touches `IntExp`.
#[derive(Clone, Copy, Debug)]
pub struct CGenerator<T: Mandelbrotable> {
    origin: (T, T),
    space: T,
    bits_needed: u32,
}

impl<T: Mandelbrotable> CGenerator<T> {
    /// Admit with [`DEFAULT_C_GENERATOR_MARGIN_BITS`] render headroom.
    // r[impl cz.depth.c-generator-fails-closed+1]
    pub fn new(loc: &(IntExp, IntExp), zoom_pot: i64, res: (u32, u32)) -> Option<Self> {
        Self::new_with_margin(loc, zoom_pot, res, DEFAULT_C_GENERATOR_MARGIN_BITS)
    }

    /// Fail-closed admit: [`HostPrecision::significand_bits`] covers magnitude
    /// plus pixel pitch (and optional `margin_bits`).
    // r[impl cz.depth.c-generator-fails-closed+1]
    pub fn new_with_margin(
        loc: &(IntExp, IntExp),
        zoom_pot: i64,
        res: (u32, u32),
        margin_bits: u32,
    ) -> Option<Self> {
        let bits_needed = type_covers_stencil::<T>(loc, zoom_pot, res, margin_bits)?;
        let space_objective = IntExp::from(1).shift(-(zoom_pot as i32 + PIXELS_PER_UNIT_POT));
        Some(Self {
            origin: (T::from(loc.0.clone()), T::from(loc.1.clone())),
            space: T::from(space_objective),
            bits_needed,
        })
    }

    pub fn new_relative(
        loc: &(IntExp, IntExp),
        reference: &(IntExp, IntExp),
        zoom_pot: i64,
        res: (u32, u32),
    ) -> Option<Self> {
        Self::new_relative_with_margin(
            loc,
            reference,
            zoom_pot,
            res,
            DEFAULT_C_GENERATOR_MARGIN_BITS,
        )
    }

    pub fn new_relative_with_margin(
        loc: &(IntExp, IntExp),
        reference: &(IntExp, IntExp),
        zoom_pot: i64,
        res: (u32, u32),
        margin_bits: u32,
    ) -> Option<Self> {
        let relative = (
            loc.0.clone() - reference.0.clone(),
            loc.1.clone() - reference.1.clone(),
        );
        Self::new_with_margin(&relative, zoom_pot, res, margin_bits)
    }

    #[inline]
    pub fn get_c(&self, seat_and_row: (u32, u32)) -> (T, T) {
        (
            self.origin.0 + self.space * T::from_u32(seat_and_row.0),
            self.origin.1 - self.space * T::from_u32(seat_and_row.1),
        )
    }

    pub fn origin_and_space(&self) -> ((T, T), T) {
        (self.origin, self.space)
    }

    pub fn bits_needed(&self) -> u32 {
        self.bits_needed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assemblies::workgroup::work_controller::get_points;

    use crate::constants::TEST_SCREEN_RES;

    #[test]
    // r[verify cz.depth.c-generator-fails-closed+1]
    fn generator_matches_v009_grid_bit_for_bit() {
        for zoom in [-8, -2, 0, 7, 30] {
            let loc = (IntExp::from(-2), IntExp::from(1));
            let res = TEST_SCREEN_RES;
            let generator = CGenerator::<f64>::new(&loc, zoom, res).unwrap();
            let old = get_points::<f64>(res, loc, zoom);
            for row in 0..res.1 {
                for seat in 0..res.0 {
                    let index = (row * res.0 + seat) as usize;
                    assert_eq!(generator.get_c((seat, row)), old[index].delta_c);
                }
            }
        }
    }

    #[test]
    // r[verify cz.depth.c-generator-fails-closed+1]
    fn rejects_collapse_at_far_end() {
        let loc = (
            IntExp {
                val: rug::Integer::from(9_007_199_254_740_990_i64),
                exp: 0,
            },
            IntExp::ZERO,
        );
        assert!(CGenerator::<f64>::new(&loc, -8, (4, 1)).is_none());
    }

    #[test]
    fn successful_generator_has_distinct_neighbors() {
        let loc = (IntExp::from(-2), IntExp::from(1));
        let generator = CGenerator::<f64>::new(&loc, 12, TEST_SCREEN_RES).unwrap();
        for seat in 0..(TEST_SCREEN_RES.0 - 1) {
            assert_ne!(generator.get_c((seat, 0)), generator.get_c((seat + 1, 0)));
        }
    }

    #[test]
    fn stack_picker_f32_then_f64_then_floatexp() {
        let loc = (IntExp::from(-2), IntExp::from(1));
        let res = TEST_SCREEN_RES;
        let view_center = (
            loc.0.clone() + IntExp::from(8),
            loc.1.clone() - IntExp::from(5),
        );
        let picked = pick_stack_admission(&loc, 0, res, None, &view_center).unwrap();
        assert!(matches!(picked, AdmittedHostStack::F32(_)));
    }

    #[test]
    fn admit_generator_probes_only_constant_work() {
        let loc = (IntExp::from(-2), IntExp::from(1));
        let res = TEST_SCREEN_RES;
        let view_center = view_center_for_test(&loc, 12, res);
        let start = std::time::Instant::now();
        for _ in 0..10_000 {
            let _ = admit_generator::<f64>(&loc, 12, res, None, &view_center);
        }
        assert!(start.elapsed() < std::time::Duration::from_millis(100));
    }

    #[test]
    fn abs_c_f64_preserves_intexp_precision_at_depth() {
        use crate::assemblies::headgroup::window::coords::f64_to_intexp;
        use crate::assemblies::workgroup::screen_worker::workshift::abs_c_f64;
        let anchor = (IntExp::from(-1).shift(40), IntExp::ZERO);
        let rel = (2.0f64.powi(-50), 0.0);
        let bad = (
            f64::from(anchor.0.clone()) + rel.0,
            f64::from(anchor.1.clone()) + rel.1,
        );
        let good = abs_c_f64(rel, &anchor);
        let exact = (
            f64::from(anchor.0.clone() + f64_to_intexp(rel.0)),
            f64::from(anchor.1.clone() + f64_to_intexp(rel.1)),
        );
        assert_eq!(good, exact);
        let sum_ie = anchor.0.clone() + f64_to_intexp(rel.0);
        assert_ne!(
            sum_ie,
            f64_to_intexp(bad.0),
            "f64 add must drop bits that the IntExp sum still holds"
        );
    }

    #[test]
    fn home_f64_absolute_wall_distinguish_only_at_zoom_43() {
        use crate::constants::{DEFAULT_WINDOW_RES, HOME_POSITION};
        let compute_loc = (
            IntExp::from(HOME_POSITION.0),
            IntExp::ZERO - IntExp::from(HOME_POSITION.1),
        );
        let res = DEFAULT_WINDOW_RES;
        // Bit-count wall: home |c|~2 needs zoom+11 bits; f64 has 53 → pot 42.
        assert!(CGenerator::<f64>::new_with_margin(&compute_loc, 42, res, 0).is_some());
        assert!(CGenerator::<f64>::new_with_margin(&compute_loc, 43, res, 0).is_none());
        assert_eq!(
            stencil_bits_needed(&compute_loc, 17, res, 0),
            28,
            "mag 2^17 at home is past f32 (24) and inside f64 (53)"
        );
        assert!(stencil_bits_needed(&compute_loc, 17, res, 0) > F32_SIGNIFICAND_BITS);
        assert!(stencil_bits_needed(&compute_loc, 17, res, 0) <= HostPrecision::F64.significand_bits);
        assert!(CGenerator::<f32>::new_with_margin(&compute_loc, 12, res, 1).is_some());
        assert!(CGenerator::<f32>::new_with_margin(&compute_loc, 13, res, 1).is_none());
    }

    #[test]
    // r[verify cz.depth.c-generator-fails-closed+1]
    fn home_f64_absolute_wall_moves_earlier_with_default_margin() {
        use crate::constants::{DEFAULT_WINDOW_RES, HOME_POSITION};
        let compute_loc = (
            IntExp::from(HOME_POSITION.0),
            IntExp::ZERO - IntExp::from(HOME_POSITION.1),
        );
        let res = DEFAULT_WINDOW_RES;
        // Default margin 1: home wall is one zoom earlier than distinguish-only.
        assert!(CGenerator::<f64>::new(&compute_loc, 41, res).is_some());
        assert!(CGenerator::<f64>::new(&compute_loc, 42, res).is_none());
        assert_eq!(DEFAULT_C_GENERATOR_MARGIN_BITS, 1);
        assert!(CGenerator::<f64>::new_with_margin(&compute_loc, 22, res, 20).is_some());
        assert!(CGenerator::<f64>::new_with_margin(&compute_loc, 23, res, 20).is_none());
        assert!(CGenerator::<f64>::new_with_margin(&compute_loc, 23, res, 10).is_some());
    }

    #[test]
    // r[verify cz.depth.c-generator-fails-closed+1]
    fn home_copy_intexp1_admits_after_f64_wall() {
        use crate::constants::{DEFAULT_WINDOW_RES, HOME_POSITION};
        use crate::copy_intexp::CopyIntExp1;
        let compute_loc = (
            IntExp::from(HOME_POSITION.0),
            IntExp::ZERO - IntExp::from(HOME_POSITION.1),
        );
        let res = DEFAULT_WINDOW_RES;
        // Default margin 1: f64 none at 42; one-word tape still covers 42 and 49.
        assert!(CGenerator::<f64>::new(&compute_loc, 42, res).is_none());
        assert!(CGenerator::<CopyIntExp1>::new(&compute_loc, 42, res).is_some());
        assert!(CGenerator::<CopyIntExp1>::new(&compute_loc, 49, res).is_some());
        assert!(CGenerator::<CopyIntExp1>::new(&compute_loc, 52, res).is_some());
        assert!(CGenerator::<CopyIntExp1>::new(&compute_loc, 53, res).is_none());
    }

    #[test]
    fn naive_margin_does_not_rescue_with_view_center_relative() {
        use crate::constants::{DEFAULT_WINDOW_RES, HOME_POSITION};
        let compute_loc = (
            IntExp::from(HOME_POSITION.0),
            IntExp::ZERO - IntExp::from(HOME_POSITION.1),
        );
        let res = DEFAULT_WINDOW_RES;
        // Absolute+margin-32 dies around pot 11. Naive must use this probe,
        // not admit_generator's view-center relative rescue (that lasts to ~47).
        assert!(CGenerator::<f64>::new_with_margin(&compute_loc, 20, res, 32).is_none());
        assert!(CGenerator::<f64>::new_with_margin(&compute_loc, 10, res, 32).is_some());
    }

    #[test]
    // r[verify cz.depth.c-generator-fails-closed+1]
    fn margin_bits_zero_matches_prior_distinguish_only_admit() {
        use crate::constants::{DEFAULT_WINDOW_RES, HOME_POSITION};
        let compute_loc = (
            IntExp::from(HOME_POSITION.0),
            IntExp::ZERO - IntExp::from(HOME_POSITION.1),
        );
        let res = DEFAULT_WINDOW_RES;
        for zoom in [0i64, 20, 40, 42, 43] {
            let with0 = CGenerator::<f64>::new_with_margin(&compute_loc, zoom, res, 0);
            assert_eq!(with0.is_some(), zoom <= 42);
        }
    }

    #[test]
    fn seahorse_admission_zoom_scan() {
        use crate::constants::DEFAULT_WINDOW_RES;
        use crate::utils::ObjectivePosAndZoom;
        let res = DEFAULT_WINDOW_RES;
        let frame_base = ObjectivePosAndZoom {
            pos: (
                IntExp::from(-1).shift(-1),
                IntExp::from(0).shift(-1),
            ),
            zoom_pot: 19,
        };
        let mut first_relative = None;
        let mut first_abs_collapse = None;
        for zoom_pot in 20..55i32 {
            let frame = (
                ObjectivePosAndZoom {
                    zoom_pot,
                    ..frame_base.clone()
                },
                res,
            );
            let compute_loc = (
                frame.0.pos.0.clone(),
                IntExp::ZERO - frame.0.pos.1.clone(),
            );
            let view_center = view_center_for_test(&compute_loc, zoom_pot, res);
            let abs = CGenerator::<f64>::new(&compute_loc, zoom_pot as i64, res);
            if abs.is_none() && first_abs_collapse.is_none() {
                first_abs_collapse = Some(zoom_pot);
            }
            let picked = admit_generator::<f64>(
                &compute_loc,
                zoom_pot as i64,
                res,
                None,
                &view_center,
            );
            if picked.as_ref().is_some_and(|a| a.is_relative()) && first_relative.is_none() {
                first_relative = Some(zoom_pot);
            }
        }
        // With default margin, absolute collapses earlier; relative fills the gap
        // (prefer-relative on risky pitch, or absolute fail → relative fallback).
        assert!(
            first_relative.is_some_and(|z| z <= first_abs_collapse.unwrap_or(i32::MAX)),
            "relative must appear by absolute collapse (rel={first_relative:?} collapse={first_abs_collapse:?})"
        );
        assert!(
            first_abs_collapse.is_some_and(|z| (43..=46).contains(&z)),
            "seahorse absolute collapse (bit count, margin 0): got {first_abs_collapse:?}"
        );
    }

    #[test]
    fn home_absolute_vs_relative_admission_zoom_scan() {
        use crate::constants::{DEFAULT_WINDOW_RES, HOME_POSITION};
        let compute_loc = (
            IntExp::from(HOME_POSITION.0),
            IntExp::ZERO - IntExp::from(HOME_POSITION.1),
        );
        let res = DEFAULT_WINDOW_RES;
        let view_center = view_center_for_test(&compute_loc, HOME_POSITION.2, res);
        // Absolute+relative both fail once bit count exceeds f64 (home |c|~2).
        for zoom_pot in 43..46i64 {
            assert!(
                admit_generator_with_margin::<f64>(
                    &compute_loc,
                    zoom_pot,
                    res,
                    None,
                    &view_center,
                    0
                )
                .is_none(),
                "home at zoom {zoom_pot}: neither absolute nor relative f64 admits (margin 0)"
            );
        }
    }

    #[test]
    // r[verify cz.depth.relative-coords+1]
    fn home_f64_admission_has_legal_path_or_hard_wall() {
        use crate::constants::{DEFAULT_WINDOW_RES, HOME_POSITION};
        let compute_loc = (
            IntExp::from(HOME_POSITION.0),
            IntExp::ZERO - IntExp::from(HOME_POSITION.1),
        );
        let res = DEFAULT_WINDOW_RES;
        let view_center = view_center_for_test(&compute_loc, HOME_POSITION.2, res);
        // Shallow: must admit (absolute or relative) under default margin.
        for zoom_pot in 0..30i64 {
            assert!(
                admit_generator::<f64>(&compute_loc, zoom_pot, res, None, &view_center).is_some(),
                "home zoom_pot={zoom_pot} must admit f64 absolute or relative"
            );
        }
        for zoom_pot in 43..46i64 {
            assert!(
                admit_generator_with_margin::<f64>(
                    &compute_loc,
                    zoom_pot,
                    res,
                    None,
                    &view_center,
                    0
                )
                .is_none(),
                "home zoom_pot={zoom_pot}: f64 must fail closed (margin 0) so FloatExp host can own the view"
            );
        }
    }

    fn view_center_for_test(
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

    #[test]
    fn relative_generator_subtracts_before_narrowing() {
        let reference = (IntExp::from(-1), IntExp::from(0));
        let loc = (
            reference.0.clone() + IntExp::from(1).shift(-100),
            reference.1.clone(),
        );
        let generator = CGenerator::<f64>::new_relative(&loc, &reference, 100, (2, 1)).unwrap();
        assert_eq!(generator.get_c((0, 0)).0, 2.0f64.powi(-100));
        assert_ne!(generator.get_c((0, 0)), generator.get_c((1, 0)));
    }

    /// Thought-killed pins for `get_c` +/− / `*` and admission Absolute vs Relative.
    #[test]
    fn mutant_kill_c_generator_get_c_and_admission() {
        let loc = (IntExp::from(-2), IntExp::from(1));
        let g = CGenerator::<f64>::new(&loc, 0, (4, 3)).expect("shallow grid");
        let (origin, space) = g.origin_and_space();
        assert!(space > 0.0);
        // +seat → +real; +row → −imag (not flipped signs / *→+).
        let c00 = g.get_c((0, 0));
        let c10 = g.get_c((1, 0));
        let c01 = g.get_c((0, 1));
        assert_eq!(c00, origin);
        assert!((c10.0 - (origin.0 + space)).abs() < 1e-15);
        assert!((c01.1 - (origin.1 - space)).abs() < 1e-15);
        assert_ne!(c10.0, origin.0 - space); // wrong real sign
        assert_ne!(c01.1, origin.1 + space); // wrong imag sign
        assert_ne!(c10.0, origin.0 + space + space); // *→+ on seat*space
        assert_ne!(c10, c01);

        // axis_distinct && : both axes must pass; count<=1 short-circuit stays true.
        assert!(CGenerator::<f64>::new(&loc, 0, (1, 1)).is_some());

        let view = view_center_for_test(&loc, 0, (4, 3));
        let abs = admit_generator::<f64>(&loc, 0, (4, 3), None, &view).expect("admit");
        assert!(!abs.is_relative());
        assert!(matches!(abs, GeneratorAdmission::Absolute(_)));

        // Deep home: absolute fails under default margin; relative must still admit.
        use crate::constants::{DEFAULT_WINDOW_RES, HOME_POSITION};
        let home = (
            IntExp::from(HOME_POSITION.0),
            IntExp::ZERO - IntExp::from(HOME_POSITION.1),
        );
        let center = view_center_for_test(&home, 42, DEFAULT_WINDOW_RES);
        let deep = admit_generator::<f64>(&home, 42, DEFAULT_WINDOW_RES, None, &center)
            .expect("home zoom 42 must admit under default margin");
        assert!(
            deep.is_relative(),
            "past absolute+margin wall should be Relative"
        );
        assert_ne!(deep.is_relative(), false);

        // Settings override: margin 0 admits absolute deeper than default.
        assert!(CGenerator::<f64>::new_with_margin(&home, 40, DEFAULT_WINDOW_RES, 0).is_some());
        assert!(CGenerator::<f64>::new_with_margin(&home, 40, DEFAULT_WINDOW_RES, 10).is_none());

        let picked = pick_stack_admission(&loc, 0, (4, 3), None, &view).unwrap();
        assert!(matches!(picked, AdmittedHostStack::F32(_)));
    }
}
