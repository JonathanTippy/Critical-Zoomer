// read delivery.md for project context
//! Precision gears for tile work (docs/design/tile_worker.md).
//!
//! Smaller types are preferred for speed. GPU is preferred to CPU when the gear
//! exists on both. The C-generator asks for a gear that can distinguish every
//! stencil point; the worker climbs this ladder until one fits.
// r[impl cz.seamless.gpu-preferred+1]

/// How a tile is iterated: which numeric type, and whether GPU may run it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Gear {
    /// f32 perturbation — fastest, preferred; CPU and GPU.
    F32,
    /// f64 perturbation — CPU only (GPU stays on f32 / stacked-i32).
    F64,
    /// Stacked i32 limbs with a shared exponent. `limbs` is 1..=8.
    StackedI32 { limbs: u8 },
    /// Adaptive rug float — heap, least preferred, CPU only.
    AdaptiveRug,
    // DESIGN HOLE (auth `[i32;N]+exp` CPU-only array gear): blocked until the
    // developer specifies N and reconciles the auth "12 stack" count vs the
    // enumerated 11-type list. Do not invent N here.
}

impl Gear {
    pub const MIN_LIMBS: u8 = 1;
    pub const MAX_LIMBS: u8 = 8;

    /// Stack gears in preference order, ending at the adaptive heap gear.
    pub fn ladder() -> [Gear; 11] {
        [
            Gear::F32,
            Gear::F64,
            Gear::StackedI32 { limbs: 1 },
            Gear::StackedI32 { limbs: 2 },
            Gear::StackedI32 { limbs: 3 },
            Gear::StackedI32 { limbs: 4 },
            Gear::StackedI32 { limbs: 5 },
            Gear::StackedI32 { limbs: 6 },
            Gear::StackedI32 { limbs: 7 },
            Gear::StackedI32 { limbs: 8 },
            Gear::AdaptiveRug,
        ]
    }

    pub fn runs_on_gpu(self) -> bool {
        match self {
            Gear::F32 => true,
            Gear::F64 => false,
            Gear::StackedI32 { limbs } => (Self::MIN_LIMBS..=Self::MAX_LIMBS).contains(&limbs),
            Gear::AdaptiveRug => false,
        }
    }

    pub fn runs_on_cpu(self) -> bool {
        true
    }

    /// Approximate significand bits this gear can hold.
    pub fn significand_bits(self) -> u32 {
        match self {
            Gear::F32 => 24,
            Gear::F64 => 53,
            Gear::StackedI32 { limbs } => u32::from(limbs) * 32,
            Gear::AdaptiveRug => u32::MAX / 4,
        }
    }

    /// Pick the smallest gear that can distinguish `required_bits` of Δc, and
    /// prefer a GPU-capable gear when a device exists.
    pub fn select(required_bits: u32, gpu_available: bool) -> Gear {
        let candidates = [
            Gear::F32,
            Gear::F64,
            Gear::StackedI32 { limbs: 1 },
            Gear::StackedI32 { limbs: 2 },
            Gear::StackedI32 { limbs: 3 },
            Gear::StackedI32 { limbs: 4 },
            Gear::StackedI32 { limbs: 5 },
            Gear::StackedI32 { limbs: 6 },
            Gear::StackedI32 { limbs: 7 },
            Gear::StackedI32 { limbs: 8 },
            Gear::AdaptiveRug,
        ];
        // Prefer GPU gears that fit, then any CPU gear that fits.
        if gpu_available {
            for gear in candidates {
                if gear.runs_on_gpu() && gear.significand_bits() >= required_bits {
                    return gear;
                }
            }
        }
        for gear in candidates {
            if gear.significand_bits() >= required_bits {
                return gear;
            }
        }
        Gear::AdaptiveRug
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // r[verify cz.seamless.gpu-preferred+1]
    #[test]
    fn prefers_f32_on_gpu_when_bits_fit() {
        assert_eq!(Gear::select(20, true), Gear::F32);
    }

    #[test]
    fn climbs_to_stacked_when_f32_is_too_narrow() {
        assert_eq!(
            Gear::select(40, true),
            Gear::StackedI32 { limbs: 2 }
            , "40 bits need two i32 limbs on the GPU ladder (f64 is cpu-only)"
        );
    }

    #[test]
    fn without_gpu_f64_is_preferred_over_stacked_for_mid_precision() {
        assert_eq!(Gear::select(40, false), Gear::F64);
    }

    #[test]
    fn adaptive_rug_is_the_last_resort() {
        assert_eq!(Gear::select(u32::MAX / 2, true), Gear::AdaptiveRug);
    }

    #[test]
    fn stacked_gears_cover_one_through_eight_limbs() {
        for limbs in 1..=8 {
            assert!(Gear::StackedI32 { limbs }.runs_on_gpu());
            assert_eq!(
                Gear::StackedI32 { limbs }.significand_bits(),
                u32::from(limbs) * 32
            );
        }
    }
}
