use rug::*;
use std::cmp::*;
use std::ops::*;

use crate::constants::*;

#[inline]
pub fn zoom_from_pot(zoom: i32) -> f64 {
    // 2^zoom — prefer powi over bit-shifts so large |zoom| stays well-defined
    // and the positive/negative paths are not accidentally equivalent at 0.
    2f64.powi(zoom)
}

#[inline]
pub fn signed_shift(input: i32, shift: i64) -> i32 {
    // Old form evaluated both `<<` and `>>` every call; |shift| ≥ 32 overflowed
    // in debug (home from a deep view). Same arithmetic as wrapping i32 shifts.
    const BITS: i64 = i32::BITS as i64;
    if shift >= 0 {
        if shift >= BITS {
            0
        } else {
            input << (shift as u32)
        }
    } else {
        let r = -shift;
        if r >= BITS {
            if input < 0 {
                -1
            } else {
                0
            }
        } else {
            input >> (r as u32)
        }
    }
}

#[inline]
pub fn shift(input: i32, shift: i32) -> i32 {
    signed_shift(input, shift as i64)
}

/*#[inline]
pub fn shift_signed_assume_left(input: i32, shift: i64) -> i32 {
    if shift >= 0 {
        input
    }
}*/

#[derive(Clone, Debug, PartialEq)]
pub struct ObjectivePosAndZoom {
    pub pos: (IntExp, IntExp)
    , pub zoom_pot: i32
}



#[derive(Clone, Debug)]
pub struct IntExp {
    pub val: Integer
    , pub exp: i32
}

impl Into<isize> for IntExp {
    fn into(self) -> isize {
        // Clamp after the shift so boundary compares on the unshifted `val` cannot
        // hide behind equivalent `>`/`>=` pre-checks.
        let negative = self.val.is_negative();
        match self.val.shift(self.exp).to_isize() {
            Some(v) => v,
            None => {
                if negative {
                    isize::MIN
                } else {
                    isize::MAX
                }
            }
        }
    }
}

impl From<isize> for IntExp {
    fn from(val:isize) -> IntExp {
        IntExp{val: Integer::from(val), exp: 0}
    }
}

impl From<usize> for IntExp {
    fn from(value: usize) -> IntExp {
        IntExp{
            val: Integer::from(value)
            , exp: 1
        }
    }
}

impl PartialEq for IntExp {
    fn eq(&self, other: &Self) -> bool {
        (self.clone() - other.clone()).val == 0
    }
}

impl Eq for IntExp {}

impl PartialOrd for IntExp {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
    fn lt(&self, other: &Self) -> bool {
        (self.clone() - other.clone()).val < 0
    }
    fn gt(&self, other: &Self) -> bool {
        (self.clone() - other.clone()).val > 0
    }
    fn le(&self, other: &Self) -> bool {
        !((self.clone() - other.clone()).val > 0)
    }
    fn ge(&self, other: &Self) -> bool {
        !((self.clone() - other.clone()).val < 0)
    }
}

impl Ord for IntExp {
    fn cmp(&self, other: &Self) -> Ordering {
        let d = (self.clone() - other.clone()).val;
        if d < 0 {
            Ordering::Less
        } else if d > 0 {
            Ordering::Greater
        } else {
            Ordering::Equal
        }
    }
}


impl Add for IntExp {
    type Output = Self;
    fn add(self, other:Self) -> Self {
        // Only the higher-exp mantissa is shifted — never `<< 0` (which is
        // observationally identical to `>> 0` and hid a cargo-mutants survivor).
        match self.exp.cmp(&other.exp) {
            Equal => Self {
                val: self.val + other.val,
                exp: self.exp,
            },
            Greater => {
                let s = (self.exp - other.exp) as u32;
                debug_assert!(s > 0);
                Self {
                    val: (self.val << s) + other.val,
                    exp: other.exp,
                }
            },
            Less => {
                let s = (other.exp - self.exp) as u32;
                debug_assert!(s > 0);
                Self {
                    val: self.val + (other.val << s),
                    exp: self.exp,
                }
            },
        }
    }
}

impl Sub for IntExp {
    type Output = Self;
    fn sub(self, other:Self) -> Self {
        match self.exp.cmp(&other.exp) {
            Equal => Self {
                val: self.val - other.val,
                exp: self.exp,
            },
            Greater => {
                let s = (self.exp - other.exp) as u32;
                debug_assert!(s > 0);
                Self {
                    val: (self.val << s) - other.val,
                    exp: other.exp,
                }
            },
            Less => {
                let s = (other.exp - self.exp) as u32;
                debug_assert!(s > 0);
                Self {
                    val: self.val - (other.val << s),
                    exp: self.exp,
                }
            },
        }
    }
}

impl Mul for IntExp {
    type Output = Self;
    fn mul(self, other:Self) -> Self {

        Self {
            val: self.val * other.val
            , exp: self.exp + other.exp
        }
    }
}

impl Shl<u32> for IntExp {
    type Output = IntExp;

    fn shl(self, rhs: u32) -> Self::Output {
        Self{
            val: self.val
            , exp: self.exp + rhs as i32
        }
    }
}

impl Shr<u32> for IntExp {
    type Output = IntExp;

    fn shr(self, rhs: u32) -> Self::Output {
        Self{
            val: self.val
            , exp: self.exp - rhs as i32
        }
    }
}

impl std::fmt::Display for IntExp {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        // Deliberately no significant_bits warning side-effect: it was untested
        // (mutants flipped the compare with no observable Result change).
        if self.exp >= 0 {
            f.write_str(&(self.val.clone()<<self.exp as u32).to_string())?;
            Ok(())
        } else {
            f.write_str(&(self.val.clone()>>(-self.exp as u32)).to_string())?;
            f.write_str(".")?;
            f.write_str("...")?;
            Ok(())
        }
    }
}
use std::cmp::Ordering::*;

impl IntExp {
    pub const ZERO: IntExp = IntExp {val: Integer::ZERO, exp: 0};

    pub fn shift(self, exp: i32) -> IntExp {
        match exp.cmp(&0) {
            Greater => self << exp as u32,
            Less => self >> (-exp) as u32,
            Equal => self,
        }
    }
    /// Drop the low `bits` of the mantissa and raise `exp` by the same amount.
    /// Value is preserved up to that truncation — used when zooming out so the
    /// location is not stored finer than the new pixel grid requires.
    pub fn round(self, bits: usize) -> IntExp {
        if bits == 0 {
            return self;
        }
        IntExp {
            val: self.val >> (bits as u32),
            exp: self.exp + bits as i32,
        }
    }
    pub fn set_precision(self, pot: i32) -> IntExp {
        // Compare absolute stored scale `-exp` to the requested pot. Negating
        // `exp` is load-bearing (a delete-`-` mutant collapses the match).
        let abs_exp = -self.exp;
        match abs_exp.cmp(&pot) {
            Equal => self,
            Greater => IntExp {
                val: self.val >> (abs_exp - pot) as u32,
                exp: -pot,
            },
            Less => IntExp {
                val: self.val << (pot - abs_exp) as u32,
                exp: -pot,
            },
        }
    }
}


impl From<i32> for IntExp {
    fn from(value: i32) -> Self {
        Self{val:Integer::from(value), exp:0}
    }

}

impl Into<i32> for IntExp {
    fn into(self) -> i32 {
        (self.val.shift(self.exp)).to_i32_wrapping()
    }
}
impl From<IntExp> for f64 {
    fn from(a: IntExp) -> f64 {
        // Round `val * 2^exp` as one binary value. `val.to_f64() * 2^exp`
        // rounds the integer first and invents ulps — C-generator then
        // false-admits f64 when adjacent seats are not distinct.
        let mut x = rug::Float::with_val(53, &a.val);
        if a.exp >= 0 {
            x <<= a.exp as u32;
        } else {
            x >>= (-a.exp) as u32;
        }
        x.to_f64()
    }
}

/*impl Into<f64> for IntExp {
    fn into(self) -> f64 {
        self.val.to_f64() * 2.0f64.powf(self.exp as f64)
    }
}*/
impl From<IntExp> for f32 {
    fn from(a: IntExp) -> f32 {
        let mut x = rug::Float::with_val(24, &a.val);
        if a.exp >= 0 {
            x <<= a.exp as u32;
        } else {
            x >>= (-a.exp) as u32;
        }
        x.to_f32()
    }
}

trait Shiftable {
    fn shift(self, shift:i32) -> Self;
}

impl Shiftable for Integer {
    fn shift(self, shift:i32) -> Self {
        if shift >= 0 {
            self << shift as u32
        } else {
            self >> (-shift) as u32
        }
    }
}

impl Shiftable for f64 {
    fn shift(self, shift:i32) -> Self {
        self * zoom_from_pot(shift)
    }
}

pub fn f32_to_i16(input: f32) -> i16 {
    let p = input * (2<<12) as f32;
    p as i16
}

pub fn i16_to_f32(input: i16) -> f32 {
    let p:f32 = input as f32 / (2<<12) as f32;
    p
}

#[inline]
pub fn index_from_pos(pos:&(i32, i32), wid:u32) -> usize {
    (pos.0 + pos.1*wid as i32) as usize
}

#[inline]
pub fn index_from_pos_safe(pos:&(i32, i32), res:(u32, u32)) -> Option<usize> {

    let valid = (
        res.0 as i32 > pos.0 && pos.0 >= 0
        && res.1 as i32 > pos.1 && pos.1 >= 0
    );

    if valid {
        Some((pos.0 + pos.1*res.0 as i32) as usize)
    } else {None}
}

pub fn pos_from_index(i: usize, wid:u32) -> (i32, i32) {
    (i as i32 % wid as i32, i as i32 / wid as i32)
}

impl Default for ObjectivePosAndZoom {
    fn default() -> Self {
        HOME_POSITION.into()
    }
}

impl From<(i32, i32, i32)> for ObjectivePosAndZoom {
    fn from(input:(i32, i32, i32)) -> ObjectivePosAndZoom {
        ObjectivePosAndZoom {
            pos: (IntExp::from(input.0), IntExp::from(input.1))
            , zoom_pot: input.2
        }
    }
}

#[test]
fn test_intexp_speed() {
    let mut rand = rand::RandState::new();
    let a = std::time::Instant::now();

    let mut int = Integer::from(Integer::random_bits(3600000, &mut rand));


    //let mut int = Integer::u_pow_u(2, 36000000).complete();

    println!("creating int took {} milliseconds", a.elapsed().as_millis());
    let a = std::time::Instant::now();

    int -= 1;
    println!("subtracting 1 took {} milliseconds", a.elapsed().as_millis());

    let mut test_val = IntExp { val: int, exp: -3600000 };

    let a = std::time::Instant::now();


    test_val = test_val - IntExp::from(1);
    println!("subtracting 1 took {} milliseconds", a.elapsed().as_millis());
    let a = std::time::Instant::now();

    test_val = test_val * IntExp::from(2);
    println!("multiplying by 2 took {} milliseconds", a.elapsed().as_millis());
}

#[cfg(test)]
mod mutant_kill {
    //! Fast, local pins for `utils` mutants listed in `mutants.out/{missed,caught,timeout}.txt`.
    //! cargo-mutants is too slow to re-verify; these tests are the "thought killed" bar.
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn zoom_from_pot_exact_powers() {
        assert_eq!(zoom_from_pot(0), 1.0);
        assert_eq!(zoom_from_pot(1), 2.0);
        assert_eq!(zoom_from_pot(2), 4.0);
        assert_eq!(zoom_from_pot(3), 8.0);
        assert_eq!(zoom_from_pot(-1), 0.5);
        assert_eq!(zoom_from_pot(-2), 0.25);
        assert_eq!(zoom_from_pot(-3), 0.125);
        assert!(zoom_from_pot(1) > zoom_from_pot(0));
        assert!(zoom_from_pot(-1) < zoom_from_pot(0));
        assert_ne!(zoom_from_pot(1), 0.0);
        assert_ne!(zoom_from_pot(-1), -1.0);
    }

    #[test]
    fn signed_shift_matches_shift_for_i32_range() {
        for input in [-8, -1, 0, 1, 7, 1024] {
            for s in -5i64..=5 {
                if s.abs() > 20 {
                    continue;
                }
                let via_signed = signed_shift(input, s);
                let via_shift = shift(input, s as i32);
                assert_eq!(
                    via_signed, via_shift,
                    "input={input} shift={s}: signed_shift={via_signed} shift={via_shift}"
                );
            }
        }
        // Directional pins that kill << / >> / sign flips.
        assert_eq!(signed_shift(8, 2), 32);
        assert_eq!(signed_shift(32, -2), 8);
        assert_eq!(signed_shift(-16, 1), -32);
        assert_eq!(signed_shift(-16, -1), -8);
        assert_ne!(signed_shift(8, 2), signed_shift(8, -2));
        // Home from deep zoom: |shift| ≥ 32 must not overflow-panic.
        assert_eq!(signed_shift(8, 40), 0);
        assert_eq!(signed_shift(8, -40), 0);
        assert_eq!(signed_shift(-8, -40), -1);
        assert_eq!(shift(1, 32), 0);
        assert_eq!(shift(-1, -32), -1);
    }

    #[test]
    fn shift_left_and_right_are_inverses_on_safe_range() {
        assert_eq!(shift(5, 0), 5);
        assert_eq!(shift(5, 3), 40);
        assert_eq!(shift(40, -3), 5);
        assert_eq!(shift(-12, 2), -48);
        assert_eq!(shift(-48, -2), -12);
        assert_ne!(shift(5, 3), 0);
        assert_ne!(shift(5, 3), 1);
        assert_ne!(shift(5, 3), -1);
        assert_ne!(shift(40, -3), shift(40, 3));
    }

    #[test]
    fn intexp_into_isize_clamps_extremes() {
        let huge = IntExp {
            val: Integer::from(isize::MAX) + Integer::from(10),
            exp: 0,
        };
        let tiny = IntExp {
            val: Integer::from(isize::MIN) - Integer::from(10),
            exp: 0,
        };
        let mid = IntExp::from(42isize);
        assert_eq!(Into::<isize>::into(huge), isize::MAX);
        assert_eq!(Into::<isize>::into(tiny), isize::MIN);
        assert_eq!(Into::<isize>::into(mid), 42);
        // Boundary: MAX itself must not clamp via >= / <= mutants.
        let at_max = IntExp {
            val: Integer::from(isize::MAX),
            exp: 0,
        };
        let at_min = IntExp {
            val: Integer::from(isize::MIN),
            exp: 0,
        };
        assert_eq!(Into::<isize>::into(at_max), isize::MAX);
        assert_eq!(Into::<isize>::into(at_min), isize::MIN);
    }

    #[test]
    fn intexp_ord_matches_numeric_difference() {
        let a = IntExp::from(3);
        let b = IntExp::from(5);
        let c = IntExp {
            val: Integer::from(3),
            exp: 1,
        }; // 3 << 1 in add/sub alignment → value 6 at exp 0 after ops; compare via PartialOrd
        assert!(a < b);
        assert!(b > a);
        assert!(a <= b);
        assert!(b >= a);
        assert!(a <= a);
        assert!(a >= a);
        assert_eq!(a.partial_cmp(&b), Some(Ordering::Less));
        assert_eq!(b.partial_cmp(&a), Some(Ordering::Greater));
        assert_eq!(a.partial_cmp(&a), Some(Ordering::Equal));
        assert_eq!(a.cmp(&b), Ordering::Less);
        // Equal under different encodings: 6 * 2^0 == 3 * 2^1
        let six = IntExp::from(6);
        assert_eq!(six, c);
        assert_eq!(six.cmp(&c), Ordering::Equal);
        // Strict inequality pins (kill <→<= and >→>=).
        assert!(!(a < a));
        assert!(!(a > a));
        assert!(a < b && !(a > b));
    }

    #[test]
    fn intexp_mul_adds_exponents() {
        let a = IntExp {
            val: Integer::from(3),
            exp: -2,
        };
        let b = IntExp {
            val: Integer::from(5),
            exp: 4,
        };
        let p = a * b;
        assert_eq!(p.val, Integer::from(15));
        assert_eq!(p.exp, 2);
        assert_ne!(p.exp, -2 - 4); // would be if + became -
    }

    #[test]
    fn intexp_add_sub_align_exponents() {
        let a = IntExp {
            val: Integer::from(1),
            exp: -1,
        };
        let b = IntExp {
            val: Integer::from(1),
            exp: -3,
        };
        let sum = a.clone() + b.clone();
        assert_eq!(sum.exp, -3);
        assert_eq!(sum.val, Integer::from(4 + 1)); // 1<<2 + 1
        // Explicitly kill <<→>> on the nonzero shift path.
        assert_ne!(sum.val, Integer::from(1));
        assert_ne!(sum.val, Integer::from(0));
        // Both orderings (Greater and Less branches).
        let sum_rev = b.clone() + a.clone();
        assert_eq!(sum_rev.val, sum.val);
        assert_eq!(sum_rev.exp, sum.exp);
        let same_exp = IntExp::from(3) + IntExp::from(5);
        assert_eq!(same_exp.val, Integer::from(8));
        assert_eq!(same_exp.exp, 0);
        let diff = a - b;
        assert_eq!(diff.exp, -3);
        assert_eq!(diff.val, Integer::from(4 - 1));
        let diff_rev = IntExp {
            val: Integer::from(1),
            exp: -3,
        } - IntExp {
            val: Integer::from(1),
            exp: -1,
        };
        assert_eq!(diff_rev.exp, -3);
        assert_eq!(diff_rev.val, Integer::from(1 - 4));
    }

    #[test]
    fn intexp_shift_identity_and_directions() {
        let v = IntExp {
            val: Integer::from(3),
            exp: -2,
        };
        assert_eq!(v.clone().shift(0).exp, -2);
        assert_eq!(v.clone().shift(0).val, Integer::from(3));
        let left = v.clone().shift(2);
        assert_eq!(left.exp, 0);
        assert_eq!(left.val, Integer::from(3));
        let right = v.shift(-1);
        assert_eq!(right.exp, -3);
        assert_eq!(right.val, Integer::from(3));
        assert_ne!(left.exp, right.exp);
    }

    #[test]
    fn intexp_set_precision_respects_negated_exp() {
        // exp = -8 means abs scale 8. Target pot=3 → shift val right by 5.
        let v = IntExp {
            val: Integer::from(0b1_0000_0000),
            exp: -8,
        };
        let p = v.clone().set_precision(3);
        assert_eq!(p.exp, -3);
        assert_eq!(p.val, Integer::from(0b1_0000_0000) >> 5);
        // Target pot=10 → shift left by 2 (finer).
        let q = v.set_precision(10);
        assert_eq!(q.exp, -10);
        assert_eq!(q.val, Integer::from(0b1_0000_0000) << 2);
        // Equal branch.
        let e = IntExp {
            val: Integer::from(7),
            exp: -4,
        };
        let same = e.clone().set_precision(4);
        assert_eq!(same.exp, e.exp);
        assert_eq!(same.val, e.val);
    }

    #[test]
    fn intexp_into_isize_applies_exp_shift() {
        let shifted = IntExp {
            val: Integer::from(3),
            exp: 2,
        };
        assert_eq!(Into::<isize>::into(shifted), 12);
        let frac = IntExp {
            val: Integer::from(16),
            exp: -2,
        };
        assert_eq!(Into::<isize>::into(frac), 4);
    }

    #[test]
    fn intexp_display_formats_integer_and_fractional() {
        let whole = IntExp {
            val: Integer::from(5),
            exp: 2,
        };
        assert_eq!(format!("{whole}"), "20");
        // exp==0 must take the integer branch (>= 0, not > 0).
        let zero_exp = IntExp {
            val: Integer::from(5),
            exp: 0,
        };
        assert_eq!(format!("{zero_exp}"), "5");
        assert!(!format!("{zero_exp}").contains('.'));
        let frac = IntExp {
            val: Integer::from(7),
            exp: -1,
        };
        let s = format!("{frac}");
        assert!(s.starts_with('3'), "got {s}");
        assert!(s.contains('.'), "got {s}");
        assert!(s.ends_with("..."), "got {s}");
        assert_ne!(s, "");
    }

    #[test]
    fn intexp_shift_round_set_precision() {
        let v = IntExp {
            val: Integer::from(0b1111),
            exp: -4,
        };
        assert_eq!(v.clone().shift(2).exp, -2);
        assert_eq!(v.clone().shift(-1).exp, -5);

        let r1 = v.clone().round(1);
        assert_eq!(r1.val, Integer::from(0b0111));
        assert_eq!(r1.exp, -3);
        let r3 = v.clone().round(3);
        assert_eq!(r3.val, Integer::from(0b0001));
        assert_eq!(r3.exp, -1);
        assert_eq!(v.clone().round(0).val, v.val);

        let p = IntExp {
            val: Integer::from(0b1_0000),
            exp: -8,
        }
        .set_precision(4);
        assert_eq!(p.exp, -4);
        assert_eq!(p.val, Integer::from(0b1)); // >> (-(-8)-4) = >>4
        let q = IntExp {
            val: Integer::from(1),
            exp: -2,
        }
        .set_precision(5);
        assert_eq!(q.exp, -5);
        assert_eq!(q.val, Integer::from(1) << 3);
    }

    #[test]
    fn intexp_into_i32_and_f32() {
        let v = IntExp {
            val: Integer::from(3),
            exp: 2,
        };
        assert_eq!(Into::<i32>::into(v.clone()), 12);
        let f: f32 = v.clone().into();
        assert!((f - 12.0).abs() < 1e-5, "got {f}");
        let half = IntExp {
            val: Integer::from(1),
            exp: -1,
        };
        let hf: f32 = half.into();
        assert!((hf - 0.5).abs() < 1e-6, "got {hf}");
        assert_ne!(hf, 0.0);
        assert_ne!(hf, 1.0);
        assert_ne!(hf, -1.0);

        // From<IntExp> for f64 — kills constant / *→+/÷ survivors.
        let fd: f64 = f64::from(v);
        assert!((fd - 12.0).abs() < 1e-12, "got {fd}");
        assert_ne!(fd, 0.0);
        assert_ne!(fd, 1.0);
        assert_ne!(fd, -1.0);
        let neg = IntExp {
            val: Integer::from(-5),
            exp: 1,
        };
        let nd = f64::from(neg);
        assert!((nd - (-10.0)).abs() < 1e-12, "got {nd}");
        // * not +: 3 * 2^2 == 12, 3+4 would be 7.
        assert_ne!(fd, 3.0 + 4.0);
        // * not /: 3 / 4 != 12.
        assert_ne!(fd, 3.0 / 4.0);

        // Adjacent seats at |c|~2, pitch 2^-53 must not become distinct f64s.
        let origin = IntExp::from(-2i32);
        let next = origin.clone() + IntExp::from(1i32).shift(-53);
        assert_eq!(f64::from(origin), f64::from(next));
    }

    /// Thought-killed pin: `From<usize>` uses exp:1 (vs `From<i32>` exp:0).
    #[test]
    fn mutant_kill_intexp_from_usize_exp_one() {
        let u = IntExp::from(3usize);
        assert_eq!(u.exp, 1);
        assert_eq!(u.val, Integer::from(3));
        assert!((f64::from(u.clone()) - 6.0).abs() < 1e-12); // 3 * 2^1
        let i = IntExp::from(3i32);
        assert_eq!(i.exp, 0);
        assert!((f64::from(i) - 3.0).abs() < 1e-12);
        assert_ne!(IntExp::from(3usize), IntExp::from(3i32));
        // exp:1→0 mutant would collapse to i32 scale.
        assert_ne!(f64::from(IntExp::from(5usize)), 5.0);
        assert!((f64::from(IntExp::from(5usize)) - 10.0).abs() < 1e-12);
    }

    /// Thought-killed pins: Sub/Mul/Shl/Shr/round/set_precision arithmetic.
    #[test]
    fn mutant_kill_intexp_sub_mul_shl_shr_round_precision() {
        let a = IntExp {
            val: Integer::from(5),
            exp: 2,
        }; // 20
        let b = IntExp {
            val: Integer::from(3),
            exp: 1,
        }; // 6
        let diff = a.clone() - b.clone();
        assert!((f64::from(diff.clone()) - 14.0).abs() < 1e-12);
        // Align shifts the higher-exp operand; flipped would be wrong.
        assert_eq!(diff.exp, 1);
        assert_eq!(diff.val, Integer::from(7)); // (5<<1)-3 = 10-3? Wait: a.exp>b.exp → (5<<1)-3=7, exp=1 → 14 ✓

        let prod = a.clone() * b.clone();
        assert_eq!(prod.exp, 3); // 2+1
        assert_eq!(prod.val, Integer::from(15));
        assert!((f64::from(prod) - 120.0).abs() < 1e-12);
        assert_ne!(
            (a.clone() * b.clone()).exp,
            a.exp - b.exp
        );

        let sh = IntExp::from(3i32) << 2;
        assert_eq!(sh.exp, 2);
        assert_eq!(sh.val, Integer::from(3));
        assert!((f64::from(sh.clone()) - 12.0).abs() < 1e-12);
        let sr = sh >> 1;
        assert_eq!(sr.exp, 1);
        assert!((f64::from(sr) - 6.0).abs() < 1e-12);
        // <<→>> would shrink.
        assert_ne!(f64::from(IntExp::from(3i32) << 2), 0.75);

        let via = IntExp::from(5i32).shift(-2);
        assert_eq!(via, IntExp::from(5i32) >> 2);

        let r0 = IntExp {
            val: Integer::from(100),
            exp: -3,
        }
        .round(0);
        assert_eq!(r0.val, Integer::from(100));
        assert_eq!(r0.exp, -3);
        let r2 = IntExp {
            val: Integer::from(100),
            exp: -3,
        }
        .round(2);
        assert_eq!(r2.val, Integer::from(25)); // 100>>2
        assert_eq!(r2.exp, -1); // -3+2
        assert_ne!(r2.exp, -5); // +→- on bits

        // set_precision: abs_exp = -exp; tighten/widen to pot.
        let fine = IntExp {
            val: Integer::from(16),
            exp: -4,
        }; // abs_exp=4
        let same = fine.clone().set_precision(4);
        assert_eq!(same.exp, -4);
        let coarse = fine.clone().set_precision(2);
        assert_eq!(coarse.exp, -2);
        assert_eq!(coarse.val, Integer::from(4)); // 16>>2
        let finer = IntExp {
            val: Integer::from(3),
            exp: -1,
        }
        .set_precision(3); // abs 1 → pot 3: <<2
        assert_eq!(finer.exp, -3);
        assert_eq!(finer.val, Integer::from(12));
        // Delete `-` on abs_exp would invert Greater/Less.
        assert_ne!(fine.clone().set_precision(2).exp, 2);
    }

    /// Thought-killed pins: Ord/Eq strictness, zoom_from_pot, Into clamps.
    #[test]
    fn mutant_kill_intexp_ord_zoom_from_pot_into() {
        let a = IntExp::from(3i32);
        let b = IntExp::from(5i32);
        assert!(a < b && b > a);
        assert!(a <= b && b >= a);
        assert!(a <= a && a >= a);
        assert_eq!(a.cmp(&b), Ordering::Less);
        assert_eq!(a.partial_cmp(&b), Some(Ordering::Less));
        // Different exp, same value: 3*2^1 == 6*2^0.
        let c = IntExp {
            val: Integer::from(3),
            exp: 1,
        };
        let d = IntExp::from(6i32);
        assert_eq!(c, d);
        assert!(!(c < d) && !(c > d));

        assert_eq!(zoom_from_pot(0), 1.0);
        assert_eq!(zoom_from_pot(3), 8.0);
        assert_eq!(zoom_from_pot(-3), 0.125);
        assert_ne!(zoom_from_pot(1), zoom_from_pot(-1));
        assert_ne!(zoom_from_pot(0), 0.0);

        let huge = IntExp {
            val: Integer::from(isize::MAX) + Integer::from(10),
            exp: 0,
        };
        assert_eq!(Into::<isize>::into(huge), isize::MAX);
        let tiny = IntExp {
            val: Integer::from(isize::MIN) - Integer::from(10),
            exp: 0,
        };
        assert_eq!(Into::<isize>::into(tiny), isize::MIN);
        assert_eq!(Into::<isize>::into(IntExp::from(42isize)), 42);
    }

    /// Thought-killed pins: row-major index/pos, signed_shift, f32↔i16 scale.
    #[test]
    fn mutant_kill_index_pos_signed_shift_f32_i16() {
        assert_eq!(index_from_pos(&(0, 0), 40), 0);
        assert_eq!(index_from_pos(&(39, 0), 40), 39);
        assert_eq!(index_from_pos(&(0, 1), 40), 40);
        assert_eq!(index_from_pos(&(1, 1), 40), 41);
        assert_ne!(index_from_pos(&(1, 1), 40), 1 + 1);
        assert_eq!(pos_from_index(41, 40), (1, 1));
        assert_ne!(pos_from_index(41, 40), (0, 41));
        assert_eq!(
            index_from_pos_safe(&(1, 1), (40, 71)),
            Some(41)
        );
        assert!(index_from_pos_safe(&(-1, 0), (40, 71)).is_none());
        assert!(index_from_pos_safe(&(40, 0), (40, 71)).is_none()); // half-open
        assert!(index_from_pos_safe(&(0, 71), (40, 71)).is_none());
        assert_eq!(index_from_pos_safe(&(39, 70), (40, 71)), Some(40 * 70 + 39));

        assert_eq!(signed_shift(8, 2), 32);
        assert_eq!(signed_shift(32, -2), 8);
        assert_eq!(signed_shift(-8, 2), -32);
        assert_ne!(signed_shift(8, 2), signed_shift(8, -2));
        assert_eq!(shift(8, 2), 32);
        assert_eq!(shift(32, -2), 8);

        // 2<<12 = 8192 scale (not 2<<11 / 1<<12).
        assert_eq!(f32_to_i16(0.0), 0);
        assert_eq!(f32_to_i16(1.0), 8192);
        assert_eq!(f32_to_i16(-1.0), -8192);
        assert!((i16_to_f32(8192) - 1.0).abs() < 1e-6);
        assert!((i16_to_f32(-4096) + 0.5).abs() < 1e-6);
        assert_ne!(f32_to_i16(1.0), 4096);
        assert_ne!(f32_to_i16(1.0), 1);
    }

    #[test]
    fn shiftable_integer_and_f64() {
        let i = Integer::from(24).shift(2);
        assert_eq!(i, Integer::from(96));
        let j = Integer::from(96).shift(-2);
        assert_eq!(j, Integer::from(24));
        assert_ne!(Integer::from(24).shift(2), Integer::from(24).shift(-2));

        let x: f64 = 3.0;
        assert_eq!(x.shift(2), 12.0);
        assert_eq!(x.shift(-2), 0.75);
        assert_ne!(x.shift(2), 0.0);
        assert_ne!(x.shift(2), x.shift(2) + 1.0); // not +
        assert!((x.shift(1) - 6.0).abs() < 1e-12);
    }

    #[test]
    fn f32_i16_codec_scale_is_8192() {
        // 2<<12 == 8192
        assert_eq!(f32_to_i16(0.0), 0);
        assert_eq!(f32_to_i16(1.0), 8192);
        assert_eq!(f32_to_i16(-1.0), -8192);
        assert_eq!(f32_to_i16(0.5), 4096);
        assert_ne!(f32_to_i16(1.0), 0);
        assert_ne!(f32_to_i16(1.0), 1);
        assert_ne!(f32_to_i16(1.0), -1);

        assert!((i16_to_f32(8192) - 1.0).abs() < 1e-6);
        assert!((i16_to_f32(-8192) + 1.0).abs() < 1e-6);
        assert!((i16_to_f32(4096) - 0.5).abs() < 1e-6);
        assert_ne!(i16_to_f32(8192), 0.0);
        assert_ne!(i16_to_f32(8192), 1.0 / 8192.0); // * mutant
        assert!((i16_to_f32(f32_to_i16(0.25)) - 0.25).abs() < 1e-5);
    }

    #[test]
    fn index_pos_roundtrip_and_safe_bounds() {
        let wid = 64u32;
        let res = (64u32, 48u32);
        for y in 0..48 {
            for x in 0..64 {
                let pos = (x, y);
                let i = index_from_pos(&pos, wid);
                assert_eq!(i, (x + y * 64) as usize);
                assert_eq!(pos_from_index(i, wid), pos);
                assert_eq!(index_from_pos_safe(&pos, res), Some(i));
            }
        }
        assert_eq!(index_from_pos_safe(&(-1, 0), res), None);
        assert_eq!(index_from_pos_safe(&(0, -1), res), None);
        assert_eq!(index_from_pos_safe(&(64, 0), res), None);
        assert_eq!(index_from_pos_safe(&(0, 48), res), None);
        assert_eq!(index_from_pos_safe(&(63, 47), res), Some(63 + 47 * 64));
        // Timeout-prone mutants: return-constant / *→+.
        assert_ne!(index_from_pos(&(1, 2), wid), 1);
        assert_eq!(index_from_pos(&(1, 2), wid), 1 + 2 * 64);
        assert_ne!(index_from_pos(&(3, 4), wid), index_from_pos(&(3, 4), wid) + 1);
        // Explicit /→% kill: row must be quotient, not another remainder.
        assert_eq!(pos_from_index(25, 10), (5, 2));
        assert_ne!(pos_from_index(25, 10).1, 25 % 10);
        assert_ne!(pos_from_index(25, 10), (5, 5));
    }

    #[test]
    fn objective_pos_and_zoom_from_triplet_not_default() {
        let o = ObjectivePosAndZoom::from((3, -4, 7));
        assert_eq!(Into::<i32>::into(o.pos.0.clone()), 3);
        assert_eq!(Into::<i32>::into(o.pos.1.clone()), -4);
        assert_eq!(o.zoom_pot, 7);
        let home = ObjectivePosAndZoom::default();
        assert_ne!(o.zoom_pot, home.zoom_pot);
        assert_ne!(
            Into::<i32>::into(o.pos.0.clone()),
            Into::<i32>::into(home.pos.0.clone())
        );
        // Default must track HOME_POSITION, not an empty/zero struct.
        assert_eq!(home.zoom_pot, HOME_POSITION.2);
        assert_eq!(Into::<i32>::into(home.pos.0.clone()), HOME_POSITION.0);
        assert_eq!(Into::<i32>::into(home.pos.1.clone()), HOME_POSITION.1);
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(256))]

        #[test]
        fn zoom_from_pot_monotone_and_reciprocal(z in -20i32..20) {
            let a = zoom_from_pot(z);
            let b = zoom_from_pot(z + 1);
            prop_assert!(a.is_finite() && a > 0.0);
            prop_assert!(b > a);
            if z >= 0 {
                prop_assert_eq!(a, (1u64 << z) as f64);
            } else {
                prop_assert!((a * (1u64 << (-z)) as f64 - 1.0).abs() < 1e-12);
            }
            prop_assert!((zoom_from_pot(z) * zoom_from_pot(-z) - 1.0).abs() < 1e-12);
        }

        #[test]
        fn shift_agrees_with_signed_shift(input in -1024i32..1024, s in -8i32..8) {
            prop_assert_eq!(shift(input, s), signed_shift(input, s as i64));
        }

        #[test]
        fn index_pos_bijection(x in 0i32..128, y in 0i32..128, wid in 1u32..128) {
            prop_assume!((x as u32) < wid);
            let i = index_from_pos(&(x, y), wid);
            prop_assert_eq!(pos_from_index(i, wid), (x, y));
            prop_assert_eq!(i, (x + y * wid as i32) as usize);
        }

        #[test]
        fn intexp_ord_total(a in -50i32..50, b in -50i32..50, ea in -5i32..5, eb in -5i32..5) {
            let x = IntExp { val: Integer::from(a), exp: ea };
            let y = IntExp { val: Integer::from(b), exp: eb };
            let cmp = x.cmp(&y);
            prop_assert_eq!(x.partial_cmp(&y), Some(cmp));
            prop_assert_eq!(x < y, cmp == Ordering::Less);
            prop_assert_eq!(x > y, cmp == Ordering::Greater);
            prop_assert_eq!(x == y, cmp == Ordering::Equal);
            prop_assert_eq!(x <= y, cmp != Ordering::Greater);
            prop_assert_eq!(x >= y, cmp != Ordering::Less);
        }

        #[test]
        fn round_drops_exactly_bits(val in 1i32..10_000, exp in -16i32..8, bits in 0usize..12) {
            let v = IntExp { val: Integer::from(val), exp };
            let r = v.clone().round(bits);
            prop_assert_eq!(r.exp, exp + bits as i32);
            prop_assert_eq!(r.val, Integer::from(val) >> (bits as u32));
        }
    }
}