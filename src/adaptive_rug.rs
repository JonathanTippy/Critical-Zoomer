//! Rug `Float` helpers for AdaptiveRug / reference-orbit paths.
//!
//! Screenspace AdaptiveRug gear remains `FloatExp` (Copy Mandelbrotable) until a
//! Clone-based bout lands. Reference seek/build use `rug::Float` directly.

use rug::Float;

use crate::intexp::IntExp;

/// Default working precision when IntExp does not demand more.
pub const ADAPTIVE_RUG_DEFAULT_BITS: u32 = 256;

/// Convert IntExp → rug Float at adequate precision (no f64 squash).
pub fn intexp_to_adaptive_rug(value: &IntExp) -> Float {
    if value.val == 0 {
        return Float::with_val(ADAPTIVE_RUG_DEFAULT_BITS, 0);
    }
    let need = value
        .val
        .significant_bits()
        .saturating_add(20)
        .max(ADAPTIVE_RUG_DEFAULT_BITS);
    let mut f = Float::with_val(need, &value.val);
    if value.exp >= 0 {
        f <<= value.exp as u32;
    } else {
        f >>= (-value.exp) as u32;
    }
    f
}

#[cfg(test)]
mod adaptive_rug_tests {
    use super::*;

    #[test]
    fn adaptive_rug_from_intexp_oversized_near_one() {
        let bits = 200u32;
        let val = rug::Integer::from(1) << (bits - 1);
        let ie = IntExp {
            val
            , exp: -((bits as i32) - 1)
        };
        let got = intexp_to_adaptive_rug(&ie).to_f64();
        assert!((got - 1.0).abs() < 1e-12, "got {got}");
    }

    #[test]
    fn adaptive_rug_add_mul_basic() {
        let a = Float::with_val(ADAPTIVE_RUG_DEFAULT_BITS, 2);
        let b = Float::with_val(ADAPTIVE_RUG_DEFAULT_BITS, 3);
        let sum = Float::with_val(ADAPTIVE_RUG_DEFAULT_BITS, &a + &b);
        let prod = Float::with_val(ADAPTIVE_RUG_DEFAULT_BITS, &a * &b);
        assert!((sum.to_f64() - 5.0).abs() < 1e-12);
        assert!((prod.to_f64() - 6.0).abs() < 1e-12);
    }

    #[test]
    fn adaptive_rug_zero_intexp() {
        assert_eq!(intexp_to_adaptive_rug(&IntExp::ZERO).to_f64(), 0.0);
    }
}
