use std::ops::{Add, Mul, Sub};

use crate::constants::PIXELS_PER_UNIT_POT;
use crate::utils::IntExp;

/// Numeric host type for CPU Mandelbrot arithmetic.
///
/// `From<IntExp>` rounds to the host type. It may only be used for a screen
/// grid after `CGenerator::new` has proved that adjacent objective points stay
/// distinct in that type.
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
// r[impl cz.depth.c-generator-fails-closed+1]
pub fn admit_generator<T: Mandelbrotable>(
    compute_loc: &(IntExp, IntExp),
    zoom_pot: i64,
    res: (u32, u32),
    relative_anchor: Option<&(IntExp, IntExp)>,
    view_center: &(IntExp, IntExp),
) -> Option<GeneratorAdmission<T>> {
    if let Some(generator) = CGenerator::<T>::new(compute_loc, zoom_pot, res) {
        return Some(GeneratorAdmission::Absolute(generator));
    }
    let anchor = relative_anchor
        .map(|a| (a.0.clone(), a.1.clone()))
        .unwrap_or_else(|| (view_center.0.clone(), view_center.1.clone()));
    CGenerator::<T>::new_relative(compute_loc, &anchor, zoom_pot, res)
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
    if let Some(admission) =
        admit_generator::<f64>(compute_loc, zoom_pot, res, relative_anchor, view_center)
    {
        return Some(AdmittedHostStack::F64(admission));
    }
    admit_generator::<crate::floatexp::FloatExp>(
        compute_loc,
        zoom_pot,
        res,
        relative_anchor,
        view_center,
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
    // r[impl cz.depth.c-generator-fails-closed+1]
    pub fn new(loc: &(IntExp, IntExp), zoom_pot: i64, res: (u32, u32)) -> Option<Self> {
        let space_objective = IntExp::from(1).shift(-(zoom_pot as i32 + PIXELS_PER_UNIT_POT));
        let generator = Self {
            origin: (T::from(loc.0.clone()), T::from(loc.1.clone())),
            space: T::from(space_objective.clone()),
        };

        // Float ulp grows with magnitude. Prove adjacency at both ends of
        // each axis, including the max-magnitude end; span alone is not proof.
        let axis_distinct = |origin: T, count: u32, sign: T| {
            if count <= 1 {
                return true;
            }
            let first_next = origin + sign * generator.space;
            if first_next == origin {
                return false;
            }
            let before_last = origin + sign * generator.space * T::from_u32(count - 2);
            let last = origin + sign * generator.space * T::from_u32(count - 1);
            before_last != last
        };

        (axis_distinct(generator.origin.0, res.0, T::ONE)
            && axis_distinct(generator.origin.1, res.1, T::ONE.neg()))
        .then_some(generator)
    }

    pub fn new_relative(
        loc: &(IntExp, IntExp),
        reference: &(IntExp, IntExp),
        zoom_pot: i64,
        res: (u32, u32),
    ) -> Option<Self> {
        let relative = (
            loc.0.clone() - reference.0.clone(),
            loc.1.clone() - reference.1.clone(),
        );
        Self::new(&relative, zoom_pot, res)
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

    #[test]
    // r[verify cz.depth.c-generator-fails-closed+1]
    fn generator_matches_v009_grid_bit_for_bit() {
        for zoom in [-8, -2, 0, 7, 30] {
            let loc = (IntExp::from(-2), IntExp::from(1));
            let res = (17, 11);
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
        let generator = CGenerator::<f64>::new(&loc, 12, (800, 480)).unwrap();
        for seat in 0..799 {
            assert_ne!(generator.get_c((seat, 0)), generator.get_c((seat + 1, 0)));
        }
    }

    #[test]
    fn stack_picker_f64_before_floatexp() {
        let loc = (IntExp::from(-2), IntExp::from(1));
        let res = (17, 11);
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
        let res = (800, 480);
        let view_center = view_center_for_test(&loc, 12, res);
        let start = std::time::Instant::now();
        for _ in 0..10_000 {
            let _ = admit_generator::<f64>(&loc, 12, res, None, &view_center);
        }
        assert!(start.elapsed() < std::time::Duration::from_millis(500));
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
        assert_ne!(bad, exact, "naive f64 anchor add must collapse at depth");
    }

    #[test]
    fn home_f64_absolute_wall_at_zoom_43() {
        use crate::constants::{DEFAULT_WINDOW_RES, HOME_POSITION};
        let compute_loc = (
            IntExp::from(HOME_POSITION.0),
            IntExp::ZERO - IntExp::from(HOME_POSITION.1),
        );
        let res = DEFAULT_WINDOW_RES;
        assert!(CGenerator::<f64>::new(&compute_loc, 43, res).is_some());
        assert!(!CGenerator::<f64>::new(&compute_loc, 44, res).is_some());
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
        for zoom_pot in 40..55i32 {
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
        assert_eq!(
            first_relative,
            Some(46),
            "seahorse-class view: relative f64 admits when absolute collapses"
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
        for zoom_pot in 44..46i64 {
            assert!(
                admit_generator::<f64>(&compute_loc, zoom_pot, res, None, &view_center).is_none(),
                "home at zoom {zoom_pot}: neither absolute nor relative f64 admits"
            );
        }
    }

    #[test]
    #[ignore = "diagnostic only"]
    fn home_absolute_vs_relative_admission_zoom_scan_verbose() {
        use crate::constants::{DEFAULT_WINDOW_RES, HOME_POSITION};
        let compute_loc = (
            IntExp::from(HOME_POSITION.0),
            IntExp::ZERO - IntExp::from(HOME_POSITION.1),
        );
        let res = DEFAULT_WINDOW_RES;
        let view_center = view_center_for_test(&compute_loc, HOME_POSITION.2, res);
        for zoom_pot in 40..55i64 {
            let abs = CGenerator::<f64>::new(&compute_loc, zoom_pot, res).is_some();
            let picked = admit_generator::<f64>(
                &compute_loc,
                zoom_pot,
                res,
                None,
                &view_center,
            );
            eprintln!("zoom_pot={zoom_pot} abs={abs} picked={:?}", picked.map(|p| if p.is_relative() { "rel" } else { "abs" }));
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
}
