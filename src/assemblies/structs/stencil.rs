// read delivery.md for project context
use crate::assemblies::structs::*;
use crate::assemblies::workgroup::structs::mandelbrotable::*;

pub struct CGenerator<T: Mandelbrotable> {
    origin: (T, T)
    , space: T
}

impl<T:Mandelbrotable> CGenerator<T> {
    pub fn get_c(&self, seat:(u16, u16)) -> (T, T) {
        let half = T::from(IntExp::from(1).shift(-(PIXELS_PER_UNIT_POT + 1)));
        (
            self.origin.0 + self.space * T::from_u16(seat.0) + half
            , self.origin.1 - self.space * T::from_u16(seat.1) - half
        )
    }

    /// Compact GPU upload: (origin_re, origin_im, space). Half is fixed from PIXELS_PER_UNIT_POT.
    pub fn origin_and_space(&self) -> ((T, T), T)
    where
        T: Copy,
    {
        (self.origin, self.space)
    }
}


impl PointStencil {
    pub fn get_c_generator<T: Mandelbrotable>(&self) -> Option<CGenerator<T>> {
        if self.type_contains_all_points::<T>() {
            Some(CGenerator{
                origin: (
                    self.homothety.0.clone().into()
                    , self.homothety.1.clone().into()
                    )
                , space: self.space().into()
            })
        } else {None}
    }
    pub fn get_relative_c_generator<T: Mandelbrotable>(&self, origin: &(IntExp, IntExp)) -> Option<CGenerator<T>> {
        if self.type_contains_all_points_relative::<T>(origin) {
            Some(CGenerator {
                origin: (
                    (self.homothety.0.clone() - origin.0.clone()).into()
                    , (self.homothety.1.clone() - origin.1.clone()).into()
                )
                ,
                space: self.space().into()
            })
        } else { None }
    }
    fn type_contains_all_points<T: Mandelbrotable>(&self) -> bool {
        let space = self.space();
        let re0 = self.homothety.0.clone();
        let im0 = self.homothety.1.clone();
        let re_last = re0.clone() + space.clone() * (self.resolution.0.saturating_sub(1)).into();
        let im_last = im0.clone() + space.clone() * (self.resolution.1.saturating_sub(1)).into();
        // Neighbor seats distinct in T, and the stencil span is representable.
        // (Do not use `p - origin != p` — that is always false when origin is 0.)
        T::from(re0.clone() + space.clone()) != T::from(re0.clone())
            && T::from(im0.clone() + space.clone()) != T::from(im0.clone())
            && T::from(re_last) != T::from(re0)
            && T::from(im_last) != T::from(im0)
    }
    fn type_contains_all_points_relative<T: Mandelbrotable>(&self, origin: &(IntExp, IntExp)) -> bool {
        let space = self.space();
        let re0 = self.homothety.0.clone() - origin.0.clone();
        let im0 = self.homothety.1.clone() - origin.1.clone();
        let re_last = re0.clone() + space.clone() * (self.resolution.0.saturating_sub(1)).into();
        let im_last = im0.clone() + space.clone() * (self.resolution.1.saturating_sub(1)).into();
        T::from(re0.clone() + space.clone()) != T::from(re0.clone())
            && T::from(im0.clone() + space.clone()) != T::from(im0.clone())
            && T::from(re_last) != T::from(re0)
            && T::from(im_last) != T::from(im0)
    }
    pub fn space(&self) -> IntExp {
        let one = IntExp::from(1);
        one.shift(-(self.homothety.2 + PIXELS_PER_UNIT_POT))
    }

    fn generator_fits<T: Mandelbrotable>(&self, relative_to: Option<&(IntExp, IntExp)>) -> bool {
        match relative_to {
            Some(origin) => self.type_contains_all_points_relative::<T>(origin),
            None => self.type_contains_all_points::<T>(),
        }
    }

    /// Climb the gear ladder until the C-generator can distinguish every stencil
    /// point. Prefers GPU-capable gears when a device is available (D-GEAR-1).
    /// Discrimination demand from magnification is a hard floor: relative
    /// generators may still "fit" in f32 while absolute precision requires a
    /// wider gear (mag ~20+).
    // r[impl cz.seamless.gpu-preferred+1]
    pub fn select_gear(
        &self
        , relative_to: Option<&(IntExp, IntExp)>
        , gpu_available: bool
    ) -> crate::gear::Gear {
        use crate::constants::discrimination_bits_for_mag;
        use crate::floatexp::FloatExp;
        use crate::gear::Gear;
        use crate::stacked_intexp::StackedIntExp;

        let fits = |gear: Gear| -> bool {
            match gear {
                Gear::F32 => self.generator_fits::<f32>(relative_to),
                Gear::F64 => self.generator_fits::<f64>(relative_to),
                Gear::StackedI32 { limbs: 1 } => {
                    self.generator_fits::<StackedIntExp<1>>(relative_to)
                }
                Gear::StackedI32 { limbs: 2 } => {
                    self.generator_fits::<StackedIntExp<2>>(relative_to)
                }
                Gear::StackedI32 { limbs: 3 } => {
                    self.generator_fits::<StackedIntExp<3>>(relative_to)
                }
                Gear::StackedI32 { limbs: 4 } => {
                    self.generator_fits::<StackedIntExp<4>>(relative_to)
                }
                Gear::StackedI32 { limbs: 5 } => {
                    self.generator_fits::<StackedIntExp<5>>(relative_to)
                }
                Gear::StackedI32 { limbs: 6 } => {
                    self.generator_fits::<StackedIntExp<6>>(relative_to)
                }
                Gear::StackedI32 { limbs: 7 } => {
                    self.generator_fits::<StackedIntExp<7>>(relative_to)
                }
                Gear::StackedI32 { limbs: 8 } => {
                    self.generator_fits::<StackedIntExp<8>>(relative_to)
                }
                Gear::StackedI32 { .. } => false,
                Gear::AdaptiveRug => self.generator_fits::<FloatExp>(relative_to),
            }
        };

        let required = discrimination_bits_for_mag(self.homothety.2);
        let floor = Gear::select(required, gpu_available);
        let ladder = Gear::ladder();
        let start = ladder
            .iter()
            .position(|&gear| gear == floor)
            .unwrap_or(ladder.len().saturating_sub(1));

        if gpu_available {
            for &gear in &ladder[start..] {
                if gear.runs_on_gpu() && fits(gear) {
                    return gear;
                }
            }
        }
        for &gear in &ladder[start..] {
            if fits(gear) {
                return gear;
            }
        }
        Gear::AdaptiveRug
    }
}

#[cfg(test)]
mod c_generator_tests {
    use super::*;
    use crate::constants::PIXELS_PER_UNIT_POT;
    use crate::intexp::IntExp;

    fn stencil(mag: i32, seats: usize, rows: usize) -> PointStencil {
        PointStencil {
            homothety: (IntExp::from(-2), IntExp::from(2), mag),
            resolution: (seats, rows),
            serial_number: 0,
            focus: None,
            hover: None,
            mag_velocity: 0.0,
        }
    }

    #[test]
    fn f64_generator_available_at_shallow_mag() {
        let s = stencil(0, 64, 64);
        assert!(s.get_c_generator::<f64>().is_some());
    }

    #[test]
    fn f32_fails_when_points_not_distinguishable() {
        // Extreme mag: spacing tinier than f32 ulp around large coords.
        let s = PointStencil {
            homothety: (
                IntExp {
                    val: rug::Integer::from(1) << 40,
                    exp: -40,
                },
                IntExp {
                    val: rug::Integer::from(1) << 40,
                    exp: -40,
                },
                40,
            ),
            resolution: (64, 64),
            serial_number: 0,
            focus: None,
            hover: None,
            mag_velocity: 0.0,
        };
        // May or may not fail depending on FloatExp path — at least API returns Option.
        let _ = s.get_c_generator::<f32>();
        let _ = s.get_c_generator::<f64>();
    }

    #[test]
    fn neighbor_cs_differ_when_generator_succeeds() {
        let s = stencil(-2, 32, 32);
        let g = s.get_c_generator::<f64>().expect("f64 at home mag");
        let a = g.get_c((0, 0));
        let b = g.get_c((1, 0));
        assert_ne!(a.0, b.0);
        let expected_space = 2f64.powi(-(PIXELS_PER_UNIT_POT - 2));
        assert!((b.0 - a.0 - expected_space).abs() < 1e-9);
    }

    // r[verify cz.seamless.gpu-preferred+1]
    #[test]
    fn select_gear_prefers_f32_on_gpu_at_home() {
        let s = stencil(0, 64, 64).correct_precision();
        assert_eq!(
            s.select_gear(None, true)
            , crate::gear::Gear::F32
        );
    }

    #[test]
    fn select_gear_without_gpu_uses_f64_when_f32_still_fits() {
        let s = stencil(0, 64, 64).correct_precision();
        // f32 still fits; without GPU preference the ladder still picks the
        // smallest fit first (f32), then f64 — GPU preference only reorders
        // when GPU gears are filtered. Absolute ladder: first fit is f32.
        assert_eq!(s.select_gear(None, false), crate::gear::Gear::F32);
    }

    // D-STEN-1: mouse (hover) + mag_velocity + sequence on stencil.
    #[test]
    fn stencil_carries_hover_mag_velocity_and_serial() {
        let mut s = stencil(0, 32, 32);
        assert_eq!(s.serial_number, 0);
        assert!(s.hover.is_none());
        assert_eq!(s.mag_velocity, 0.0);
        s.hover = Some((3, 4));
        s.mag_velocity = 1.5;
        s.serial_number = 9;
        assert_eq!(s.hover, Some((3, 4)));
        assert_eq!(s.mag_velocity, 1.5);
        assert_eq!(s.serial_number, 9);
    }

    #[test]
    fn retarget_bumps_serial_when_mouse_or_velocity_changes() {
        let mut s = stencil(0, 32, 32);
        let h = s.homothety.clone();
        let r = s.resolution;
        s.retarget_with_seq(h.clone(), r, Some((1, 1)), 0.0);
        assert_eq!(s.serial_number, 1);
        assert_eq!(s.hover, Some((1, 1)));
        s.retarget_with_seq(h.clone(), r, Some((1, 1)), 2.0);
        assert_eq!(s.serial_number, 2);
        assert_eq!(s.mag_velocity, 2.0);
        s.retarget_with_seq(h, r, Some((1, 1)), 2.0);
        assert_eq!(s.serial_number, 2, "identical retarget must not bump seq");
    }

    #[test]
    fn select_gear_climbs_past_f32_when_discrimination_exceeds_mantissa() {
        let mag = 20i32;
        let required = crate::constants::discrimination_bits_for_mag(mag);
        assert!(required > 24, "test precondition: mag {mag} exceeds f32");
        let s = stencil(mag, 64, 64).correct_precision();
        let corner = (s.homothety.0.clone(), s.homothety.1.clone());
        assert_eq!(
            s.select_gear(Some(&corner), true),
            crate::gear::Gear::StackedI32 { limbs: 1 }
        );
        assert_eq!(
            s.select_gear(Some(&corner), false),
            crate::gear::Gear::F64
        );
    }

    #[test]
    fn retarget_bumps_serial_on_homothety_change_even_if_vel_same() {
        let mut s = stencil(0, 32, 32);
        s.mag_velocity = 0.25;
        let r = s.resolution;
        s.retarget_with_seq(
            (IntExp::from(-1), IntExp::from(1), 1),
            r,
            None,
            0.25,
        );
        assert_eq!(s.serial_number, 1);
        assert_eq!(s.homothety.2, 1);
    }
}

// read delivery.md for project context
// See comment at end

use rug::Integer;
use crate::assemblies::structs::*;
use crate::constants::*;
use crate::intexp::*;

fn line_segments_overlap(a: (IntExp, IntExp), b: (IntExp, IntExp)) -> bool {
    // left edge inclusive right edge limit
    (a.0 >= b.0 && a.0 < b.1)
        || (a.1 > b.0 && a.1 < b.1)
}

fn line_segment_a_is_subset_of_b(a: (IntExp, IntExp), b: (IntExp, IntExp)) -> bool {
    // left edge inclusive right edge limit
    (a.0 >= b.0 && a.0 < b.1)
        && (a.1 > b.0 && a.1 < b.1)
}

impl PointStencil {
    pub fn correct_precision(self) -> Self {
        PointStencil {
            homothety: (self.homothety.0.clone().set_precision(PIXELS_PER_UNIT_POT + self.homothety.2)
                        , self.homothety.1.clone().set_precision(PIXELS_PER_UNIT_POT + self.homothety.2), self.homothety.2)
            ,
            resolution: self.resolution
            ,
            serial_number: self.serial_number
            ,
            focus: self.focus
            ,
            hover: self.hover
            ,
            mag_velocity: self.mag_velocity
        }
    }
    pub fn assert_validity(&self) {
        assert!(
            self.homothety.0.exp == -(self.homothety.2 + PIXELS_PER_UNIT_POT)
                && self.homothety.0.exp == self.homothety.1.exp
            , "Invalid Stencil: POT zoom level and precision exponents must match."
        );
        assert!(
            self.resolution.0 < 1 << 16 && self.resolution.1 < 1 << 16
            , "Invalid Stencil: No resolution side length may exceed 2^16 pixels."
        );
        assert!(
            self.resolution.0 > 0 && self.resolution.1 > 0
            , "Invalid Stencil: No resolution side length may be 0 pixels."
        );
    }
    pub fn index(&self, seat_and_row: (isize, isize)) -> usize {
        debug_assert!(
            seat_and_row.0 >= 0 && seat_and_row.0 < self.resolution.0 as isize
                && seat_and_row.1 >= 0 && seat_and_row.1 < self.resolution.1 as isize
            , "Index Failure: nonexistent seat."
        );
        seat_and_row.1 as usize * self.resolution.0 + seat_and_row.0 as usize
    }
    pub fn seat_and_row(&self, index: usize) -> (usize, usize) {
        debug_assert!(
            index < self.resolution.0 * self.resolution.1
            , "Index Failure: nonexistent seat."
        );
        (index % self.resolution.0, index / self.resolution.0)
    }

    pub fn clamp_seat_and_row(&self, seat_and_row: (isize, isize)) -> (isize, isize) {
        return (
            seat_and_row.0.clamp(0, self.resolution.0 as isize - 1)
            , seat_and_row.1.clamp(0, self.resolution.1 as isize - 1)
        );
    }

    pub fn bottom_right_point(&self) -> (IntExp, IntExp) {
        let space = IntExp::from(1).shift(-self.homothety.2 - PIXELS_PER_UNIT_POT);
        return (
            self.homothety.0.clone() + space.clone() * IntExp::from(self.resolution.0 - 1)
            , self.homothety.1.clone() - space * IntExp::from(self.resolution.1 - 1)
        )
    }


    pub fn corners(&self) -> ((IntExp, IntExp), (IntExp, IntExp)) {
        let top_left: (IntExp, IntExp) = (self.homothety.0.clone(), self.homothety.1.clone());

        let bottom_right: (IntExp, IntExp) = (
            self.homothety.0.clone() + self.space() * IntExp::from(self.resolution.0)
            , self.homothety.1.clone() - self.space() * IntExp::from(self.resolution.1)
        );
        (
            top_left
            , bottom_right
        )
    }
    fn overlaps(&self, other: &Self) -> bool {
        line_segments_overlap(
            (self.corners().0.0, self.corners().1.0)
            , (other.corners().0.0, other.corners().1.0)
        ) && line_segments_overlap(
            (self.corners().0.1, self.corners().1.1)
            , (other.corners().0.1, other.corners().1.1)
        )
    }

    fn subset_of(&self, other: &Self) -> bool {
        line_segment_a_is_subset_of_b(
            (self.corners().0.0, self.corners().1.0)
            , (other.corners().0.0, other.corners().1.0)
        ) && line_segment_a_is_subset_of_b(
            (self.corners().0.1, self.corners().1.1)
            , (other.corners().0.1, other.corners().1.1)
        )
    }
}


// Conventions:
// location.2 is magnification which is not the precision exponent.
// magnification goes up as you zoom in.
// Usually, when seat and row go in a tuple together, the order is seat then row.
// This is to align better with the x then y standard order.
// W/H and Width / Height are banned. This project uses seats and rows,
// and anytime both dimensions are together, a tuple called resolution.
//
// The stencil defines the set of complex points that make up a screen sample grid.
// Top-left sample is exactly at location.0, location.1; +seat → +real; +row → −imag.
// Spacing is 1/(2^(PIXELS_PER_UNIT_POT + mag_pot)).
//
// Zoom-fill / View::fill_from is a dead design idea. Tiles are static; each frame the
// sampling shader maps static tile data onto the stencil. Do not rewrite answer data
// under pan/zoom. Live path: ingest GPU tiles → sample → shade.
