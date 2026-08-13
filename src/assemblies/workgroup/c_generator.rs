use std::ops::{Add, Mul, Sub};

use crate::constants::PIXELS_PER_UNIT_POT;
use crate::utils::IntExp;

/// Default C-generator render headroom beyond neighbor distinguishability.
/// Interview 2026-08-12: ~10 bits at shallow depth; leave fixed unless settings override.
pub const DEFAULT_C_GENERATOR_MARGIN_BITS: u32 = 10;

/// Numeric host type for CPU Mandelbrot arithmetic.
///
/// `From<IntExp>` rounds to the host type. It may only be used for a screen
/// grid after `CGenerator::new` has proved that adjacent objective points stay
/// distinct in that type **with** the configured render margin.
pub trait Mandelbrotable:
    Copy
    + PartialEq
    + PartialOrd
    + Add<Output = Self>
    + Sub<Output = Self>
    + Mul<Output = Self>
    + From<IntExp>
{
    const ZERO: Self;
    const ONE: Self;
    const TWO: Self;

    fn from_u32(value: u32) -> Self;
    fn to_f64(self) -> f64;
    fn abs(self) -> Self;
    fn neg(self) -> Self;
    fn max_value() -> Self;
}

impl Mandelbrotable for f64 {
    const ZERO: Self = 0.0;
    const ONE: Self = 1.0;
    const TWO: Self = 2.0;

    fn from_u32(value: u32) -> Self {
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
    F64(GeneratorAdmission<f64>),
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
/// Admission is O(1): only the near and far ends of each axis are probed in
/// `T`; the hot `get_c` loop never touches `IntExp`.
#[derive(Clone, Copy, Debug)]
pub struct CGenerator<T: Mandelbrotable> {
    origin: (T, T),
    space: T,
}

impl<T: Mandelbrotable> CGenerator<T> {
    /// Admit with [`DEFAULT_C_GENERATOR_MARGIN_BITS`] render headroom.
    // r[impl cz.depth.c-generator-fails-closed+1]
    pub fn new(loc: &(IntExp, IntExp), zoom_pot: i64, res: (u32, u32)) -> Option<Self> {
        Self::new_with_margin(loc, zoom_pot, res, DEFAULT_C_GENERATOR_MARGIN_BITS)
    }

    /// Fail-closed admit: neighbors distinct in `T` with `margin_bits` of headroom.
    ///
    /// Headroom is checked by probing a pitch of `space / 2^margin_bits`. If
    /// that finer step is still nonzero at near and far ends, the real pitch
    /// has that many bits to spare for Mandelbrot dynamics.
    // r[impl cz.depth.c-generator-fails-closed+1]
    pub fn new_with_margin(
        loc: &(IntExp, IntExp),
        zoom_pot: i64,
        res: (u32, u32),
        margin_bits: u32,
    ) -> Option<Self> {
        let space_objective = IntExp::from(1).shift(-(zoom_pot as i32 + PIXELS_PER_UNIT_POT));
        let probe = if margin_bits == 0 {
            space_objective.clone()
        } else {
            space_objective.clone().shift(-(margin_bits as i32))
        };

        // Exact IntExp probe points, then `T: From<IntExp>`. Adding the probe in
        // `T` false-admits when origin+space already rounded (blocky type).
        let axis_distinct = |origin: &IntExp, count: u32, increasing: bool| {
            if count <= 1 {
                return true;
            }
            let span = space_objective.clone() * IntExp::from((count - 1) as i32);
            let (next_ie, last_ie, last_margin_ie) = if increasing {
                (
                    origin.clone() + probe.clone(),
                    origin.clone() + span.clone(),
                    origin.clone() + span - probe.clone(),
                )
            } else {
                (
                    origin.clone() - probe.clone(),
                    origin.clone() - span.clone(),
                    origin.clone() - span + probe.clone(),
                )
            };
            T::from(origin.clone()) != T::from(next_ie)
                && T::from(last_ie) != T::from(last_margin_ie)
        };

        if !(axis_distinct(&loc.0, res.0, true) && axis_distinct(&loc.1, res.1, false)) {
            return None;
        }
        Some(Self {
            origin: (T::from(loc.0.clone()), T::from(loc.1.clone())),
            space: T::from(space_objective),
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
    fn stack_picker_f64_before_floatexp() {
        let loc = (IntExp::from(-2), IntExp::from(1));
        let res = TEST_SCREEN_RES;
        let view_center = (
            loc.0.clone() + IntExp::from(8),
            loc.1.clone() - IntExp::from(5),
        );
        let picked = pick_stack_admission(&loc, 0, res, None, &view_center).unwrap();
        assert!(matches!(picked, AdmittedHostStack::F64(_)));
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
        // Distinguish-only (margin 0): document the raw ulp wall.
        assert!(CGenerator::<f64>::new_with_margin(&compute_loc, 43, res, 0).is_some());
        assert!(CGenerator::<f64>::new_with_margin(&compute_loc, 44, res, 0).is_none());
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
        // Default ~10-bit margin fails closed ~10 pots before distinguish-only.
        assert!(CGenerator::<f64>::new(&compute_loc, 33, res).is_some());
        assert!(CGenerator::<f64>::new(&compute_loc, 34, res).is_none());
        assert!(CGenerator::<f64>::new_with_margin(&compute_loc, 34, res, 0).is_some());
        // Slider must move the wall: +10 bits ≈ +10 pots earlier.
        assert!(CGenerator::<f64>::new_with_margin(&compute_loc, 23, res, 20).is_some());
        assert!(CGenerator::<f64>::new_with_margin(&compute_loc, 24, res, 20).is_none());
        assert!(CGenerator::<f64>::new_with_margin(&compute_loc, 24, res, 10).is_some());
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
        for zoom in [0i64, 20, 40, 43] {
            let with0 = CGenerator::<f64>::new_with_margin(&compute_loc, zoom, res, 0);
            // Old distinguish-only check used space at both ends; margin 0 must match.
            assert_eq!(with0.is_some(), zoom <= 43);
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
            first_abs_collapse.is_some_and(|z| (28..=40).contains(&z)),
            "seahorse absolute collapse with default margin: got {first_abs_collapse:?}"
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
        // Distinguish-only hard wall (margin 0) still at 44–45.
        for zoom_pot in 44..46i64 {
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
        // Hard wall with default margin is earlier than distinguish-only; relative
        // still covers until the relative grid itself fails. Document margin-0 wall.
        for zoom_pot in 44..46i64 {
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
        let center = view_center_for_test(&home, 40, DEFAULT_WINDOW_RES);
        let deep = admit_generator::<f64>(&home, 40, DEFAULT_WINDOW_RES, None, &center)
            .expect("home zoom 40 must admit under default margin");
        assert!(
            deep.is_relative(),
            "past absolute+margin wall should be Relative"
        );
        assert_ne!(deep.is_relative(), false);

        // Settings override: margin 0 admits absolute deeper than default.
        assert!(CGenerator::<f64>::new_with_margin(&home, 40, DEFAULT_WINDOW_RES, 0).is_some());
        assert!(CGenerator::<f64>::new_with_margin(&home, 40, DEFAULT_WINDOW_RES, 10).is_none());

        let picked = pick_stack_admission(&loc, 0, (4, 3), None, &view).unwrap();
        assert!(matches!(picked, AdmittedHostStack::F64(_)));
    }
}
