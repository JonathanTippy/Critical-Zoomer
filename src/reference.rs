use rug::Float;
use std::time::{Duration, Instant};

use crate::floatexp::ComplexFloatExp;
use crate::utils::IntExp;

fn intexp_to_float(value: &IntExp, precision: u32) -> Float {
    let mut out = Float::with_val(precision, &value.val);
    if value.exp >= 0 {
        out <<= value.exp;
    } else {
        out >>= -value.exp;
    }
    out
}

fn iterate(z: &(Float, Float), c: &(Float, Float), precision: u32) -> (Float, Float) {
    let re = Float::with_val(
        precision,
        Float::with_val(precision, &z.0 * &z.0) - Float::with_val(precision, &z.1 * &z.1) + &c.0,
    );
    let im = Float::with_val(
        precision,
        Float::with_val(precision, &z.0 * &z.1) * 2 + &c.1,
    );
    (re, im)
}

fn stored(z: &(Float, Float)) -> ComplexFloatExp {
    ComplexFloatExp::new(
        crate::floatexp::FloatExp::from_rug(&z.0),
        crate::floatexp::FloatExp::from_rug(&z.1),
    )
}

enum CycleDetector {
    Seeking {
        tortoise: (Float, Float),
        power: u32,
        lam: u32,
    },
    AdvanceHare {
        period: u32,
        hare: (Float, Float),
        remaining: u32,
    },
    FindPreperiod {
        period: u32,
        tortoise: (Float, Float),
        hare: (Float, Float),
        preperiod: u32,
    },
    Done,
}

/// A high-precision-computed, low-precision-stored reference orbit.
///
/// `state` is the sole retained high-precision iterate, making extension
/// resumable without keeping a high-precision history.
pub struct ReferenceOrbit {
    pub c: (Float, Float),
    pub iterates: Vec<ComplexFloatExp>,
    pub state: (Float, Float),
    pub precision_bits: u32,
    pub period: Option<u32>,
    pub preperiod: u32,
    pub escaped: bool,
    cycle_detector: CycleDetector,
}

impl ReferenceOrbit {
    pub fn start(c: &(IntExp, IntExp), precision_bits: u32) -> Self {
        let precision_bits = precision_bits.max(53);
        let c = (
            intexp_to_float(&c.0, precision_bits),
            intexp_to_float(&c.1, precision_bits),
        );
        let state = (
            Float::with_val(precision_bits, 0),
            Float::with_val(precision_bits, 0),
        );
        Self {
            c,
            iterates: vec![stored(&state)],
            state,
            precision_bits,
            period: None,
            preperiod: 0,
            escaped: false,
            cycle_detector: CycleDetector::Seeking {
                tortoise: (
                    Float::with_val(precision_bits, 0),
                    Float::with_val(precision_bits, 0),
                ),
                power: 1,
                lam: 0,
            },
        }
    }

    // r[impl cz.depth.reference-low-storage+1]
    pub fn compute(c: &(IntExp, IntExp), precision_bits: u32, max_iterations: u32) -> Self {
        let mut out = Self::start(c, precision_bits);
        out.extend(max_iterations);
        out
    }

    pub fn extend(&mut self, additional_iterations: u32) {
        self.extend_inner(additional_iterations, None);
    }

    /// Extend for at most `additional_iterations`, yielding once the wall-clock
    /// budget expires. The retained tail makes the next call resume exactly
    /// where this one stopped.
    // r[impl cz.depth.reference-bout-law+1]
    pub fn extend_for(
        &mut self,
        additional_iterations: u32,
        budget: Duration,
    ) -> u32 {
        self.extend_inner(additional_iterations, Some((Instant::now(), budget)))
    }

    fn extend_inner(
        &mut self,
        additional_iterations: u32,
        deadline: Option<(Instant, Duration)>,
    ) -> u32 {
        if self.period.is_some() || self.escaped {
            return 0;
        }

        let mut completed = 0;
        for _ in 0..additional_iterations {
            if let Some((start, budget)) = deadline {
                if start.elapsed() >= budget {
                    break;
                }
            }
            if !matches!(&self.cycle_detector, CycleDetector::Seeking { .. }) {
                let finished = self.resolve_cycle_step();
                completed += 1;
                if finished {
                    return completed;
                }
                continue;
            }

            let next = iterate(&self.state, &self.c, self.precision_bits);
            let repeated = self.observe_cycle(&next);
            self.state = (next.0.clone(), next.1.clone());
            if repeated {
                completed += 1;
                continue;
            }
            self.iterates.push(stored(&next));
            completed += 1;

            let norm = Float::with_val(
                self.precision_bits,
                Float::with_val(self.precision_bits, &self.state.0 * &self.state.0)
                    + Float::with_val(self.precision_bits, &self.state.1 * &self.state.1),
            );
            if norm > 4 {
                self.escaped = true;
                return completed;
            }
        }
        completed
    }

    /// Advance the constant-memory exact cycle detector after one orbit step.
    /// Returns true once Brent has found a candidate period; preperiod
    /// resolution then continues in later bounded work units.
    fn observe_cycle(&mut self, next: &(Float, Float)) -> bool {
        let CycleDetector::Seeking {
            tortoise,
            power,
            lam,
        } = &mut self.cycle_detector
        else {
            return true;
        };

        *lam = lam.saturating_add(1);
        if tortoise.0 == next.0 && tortoise.1 == next.1 {
            let period = *lam;
            self.cycle_detector = CycleDetector::AdvanceHare {
                period,
                hare: (
                    Float::with_val(self.precision_bits, 0),
                    Float::with_val(self.precision_bits, 0),
                ),
                remaining: period,
            };
            return true;
        }
        if *lam == *power {
            *tortoise = (next.0.clone(), next.1.clone());
            *power = power.saturating_mul(2);
            *lam = 0;
        }
        false
    }

    /// Performs at most one constant-memory cycle-resolution work unit.
    fn resolve_cycle_step(&mut self) -> bool {
        match &mut self.cycle_detector {
            CycleDetector::Seeking { .. } | CycleDetector::Done => false,
            CycleDetector::AdvanceHare {
                period,
                hare,
                remaining,
            } => {
                if *remaining > 0 {
                    *hare = iterate(hare, &self.c, self.precision_bits);
                    *remaining -= 1;
                }
                if *remaining == 0 {
                    let tortoise = (
                        Float::with_val(self.precision_bits, 0),
                        Float::with_val(self.precision_bits, 0),
                    );
                    if tortoise.0 == hare.0 && tortoise.1 == hare.1 {
                        self.period = Some(*period);
                        self.preperiod = 0;
                        self.cycle_detector = CycleDetector::Done;
                        return true;
                    }
                    self.cycle_detector = CycleDetector::FindPreperiod {
                        period: *period,
                        tortoise,
                        hare: (hare.0.clone(), hare.1.clone()),
                        preperiod: 0,
                    };
                }
                false
            }
            CycleDetector::FindPreperiod {
                period,
                tortoise,
                hare,
                preperiod,
            } => {
                *tortoise = iterate(tortoise, &self.c, self.precision_bits);
                *hare = iterate(hare, &self.c, self.precision_bits);
                *preperiod = preperiod.saturating_add(1);
                if tortoise.0 == hare.0 && tortoise.1 == hare.1 {
                    self.period = Some(*period);
                    self.preperiod = *preperiod;
                    self.cycle_detector = CycleDetector::Done;
                    return true;
                }
                false
            }
        }
    }

    /// Returns any stored cycle index at arbitrary n, or None when a finite
    /// non-periodic reference has not been extended that far.
    pub fn get(&self, n: u32) -> Option<ComplexFloatExp> {
        if let Some(period) = self.period {
            let index = if n < self.preperiod {
                n
            } else {
                self.preperiod + (n - self.preperiod) % period
            };
            return self.iterates.get(index as usize).copied();
        }
        self.iterates.get(n as usize).copied()
    }

    /// The floor reference: Z_n = 0 for all n, period 1.
    ///
    /// Delta iteration against this orbit is ordinary Mandelbrot arithmetic
    /// in floatexp (`δz' = δz² + δc`). Same code path as any other reference.
    // r[impl cz.ref.zero-orbit-same-path+1]
    pub fn zero_orbit() -> Self {
        let mut orbit = Self::start(&(IntExp::ZERO, IntExp::ZERO), 53);
        orbit.period = Some(1);
        orbit.preperiod = 0;
        debug_assert_eq!(orbit.get(0), Some(ComplexFloatExp::ZERO));
        debug_assert_eq!(orbit.get(1), Some(ComplexFloatExp::ZERO));
        orbit
    }
}

pub fn bits_for_zoom(zoom_pot: i64, pixels_per_unit_pot: i32) -> u32 {
    zoom_pot
        .saturating_add(pixels_per_unit_pot as i64)
        .saturating_add(32)
        .max(53)
        .min(u32::MAX as i64) as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stored_orbit_matches_full_precision_rounding() {
        let c = (IntExp::from(-1), IntExp::from(0));
        let orbit = ReferenceOrbit::compute(&c, 256, 8);
        assert_eq!(orbit.get(0), Some(ComplexFloatExp::ZERO));
        assert_eq!(
            orbit.get(1),
            Some(ComplexFloatExp::new((-1.0).into(), 0.0.into()))
        );
    }

    #[test]
    // r[verify cz.depth.reference-low-storage+1]
    fn extending_matches_one_shot() {
        let c = (IntExp::from(3).shift(-4), IntExp::from(1).shift(-3));
        let mut split = ReferenceOrbit::compute(&c, 192, 7);
        split.extend(11);
        let whole = ReferenceOrbit::compute(&c, 192, 18);
        assert_eq!(split.iterates, whole.iterates);
        assert_eq!(split.escaped, whole.escaped);
    }

    #[test]
    // r[verify cz.depth.reference-bout-law+1]
    fn bout_sliced_extension_matches_one_shot() {
        let c = (IntExp::from(3).shift(-4), IntExp::from(1).shift(-3));
        let mut split = ReferenceOrbit::start(&c, 192);
        let mut remaining = 18;
        while remaining > 0 {
            let advanced = split.extend_for(remaining.min(3), Duration::from_secs(1));
            assert!(advanced > 0);
            remaining -= advanced;
        }
        let whole = ReferenceOrbit::compute(&c, 192, 18);
        assert_eq!(split.iterates, whole.iterates);
        assert_eq!(split.escaped, whole.escaped);
    }

    #[test]
    fn zero_budget_does_no_work() {
        let c = (IntExp::from(-1), IntExp::ZERO);
        let mut orbit = ReferenceOrbit::start(&c, 128);
        assert_eq!(orbit.extend_for(100, Duration::ZERO), 0);
        assert_eq!(orbit.iterates.len(), 1);
    }

    #[test]
    // r[verify cz.depth.reference-bout-law+1]
    fn one_step_bouts_preserve_period_and_preperiod_detection() {
        for c in [-1, -2] {
            let coordinate = (IntExp::from(c), IntExp::ZERO);
            let whole = ReferenceOrbit::compute(&coordinate, 128, 32);
            let mut sliced = ReferenceOrbit::start(&coordinate, 128);
            for _ in 0..32 {
                sliced.extend_for(1, Duration::from_secs(1));
                if sliced.period.is_some() {
                    break;
                }
            }
            assert_eq!(sliced.period, whole.period);
            assert_eq!(sliced.preperiod, whole.preperiod);
            assert_eq!(sliced.iterates, whole.iterates);
        }
    }

    #[test]
    // r[verify cz.depth.reference-low-storage+1]
    fn periodic_and_preperiodic_orbits_index_forever() {
        let p2 = ReferenceOrbit::compute(&(IntExp::from(-1), IntExp::ZERO), 128, 8);
        assert_eq!(p2.period, Some(2));
        assert_eq!(p2.preperiod, 0);
        assert_eq!(p2.get(1000), p2.get(0));
        assert_eq!(p2.get(1001), p2.get(1));

        let misiurewicz = ReferenceOrbit::compute(&(IntExp::from(-2), IntExp::ZERO), 128, 8);
        assert_eq!(misiurewicz.period, Some(1));
        assert_eq!(misiurewicz.preperiod, 2);
        assert_eq!(misiurewicz.get(10_000), misiurewicz.get(2));
    }

    #[test]
    fn escaping_reference_is_finite_and_honest() {
        let orbit = ReferenceOrbit::compute(&(IntExp::from(2), IntExp::ZERO), 128, 20);
        assert!(orbit.escaped);
        assert_eq!(orbit.period, None);
        assert!(orbit.get(10_000).is_none());
    }

    #[test]
    fn precision_grows_with_depth() {
        assert_eq!(bits_for_zoom(0, 8), 53);
        assert!(bits_for_zoom(1500, 8) >= 1540);
    }

    /// Thought-killed pins for dense `reference.rs` caught mutants
    /// (`iterate` arithmetic, `get` period wrap, cycle-step compares, extend counters).
    #[test]
    fn iterate_mandelbrot_step_is_z2_plus_c() {
        let prec = 128u32;
        let z = (
            Float::with_val(prec, 1),
            Float::with_val(prec, 2),
        );
        let c = (
            Float::with_val(prec, 3),
            Float::with_val(prec, 4),
        );
        let (re, im) = iterate(&z, &c, prec);
        // (1+2i)² + (3+4i) = -3+4i + 3+4i = 0+8i
        assert_eq!(re, Float::with_val(prec, 0));
        assert_eq!(im, Float::with_val(prec, 8));
        // Kill *→+ / *→/ on the imag 2·z0·z1 term: wrong ops cannot yield 8.
        assert_ne!(im, Float::with_val(prec, 1 * 2 + 2)); // *→+ on 2*
        assert_ne!(im, Float::with_val(prec, 1 + 2 + 4));
        // Kill *→+ / *→/ on re: z0²−z1²+c0.
        assert_ne!(re, Float::with_val(prec, 1 + 1 - (2 + 2) + 3)); // *→+
        assert_ne!(re, Float::with_val(prec, 1 / 1 - 2 / 2 + 3)); // *→/
        // Kill +→- / +→* on c addends.
        assert_ne!(re, Float::with_val(prec, 1 - 4 - 3));
        assert_ne!(im, Float::with_val(prec, (1 * 2 * 2) * 4));
    }

    #[test]
    fn mutant_kill_reference_iterate_cycle_get_intexp() {
        iterate_mandelbrot_step_is_z2_plus_c();
        get_wraps_periodic_indices_not_linear();
        observe_cycle_detects_period_two_at_minus_one();
        zero_orbit_floor_and_bits_clamp();
        extend_stops_when_period_or_escaped_and_counts_steps();

        // intexp_to_float: positive exp left-shifts, negative right-shifts.
        let hi = intexp_to_float(&IntExp { val: rug::Integer::from(3), exp: 2 }, 64);
        assert_eq!(hi, Float::with_val(64, 12));
        let lo = intexp_to_float(&IntExp { val: rug::Integer::from(8), exp: -2 }, 64);
        assert_eq!(lo, Float::with_val(64, 2));
        assert_ne!(hi, Float::with_val(64, 3 >> 2)); // <<=→>>=
        assert_ne!(lo, Float::with_val(64, 8 << 2)); // >>=→<<=
        // exp==0 keeps mantissa (kill >=→< flipping the branch).
        let z = intexp_to_float(&IntExp { val: rug::Integer::from(7), exp: 0 }, 64);
        assert_eq!(z, Float::with_val(64, 7));

        // extend_inner counters: open orbit advances exactly N steps (not +=→-=/*=).
        let mut open =
            ReferenceOrbit::start(&(IntExp::from(3).shift(-4), IntExp::from(1).shift(-3)), 128);
        let before = open.iterates.len();
        let n = open.extend_for(7, Duration::from_secs(1));
        assert_eq!(n, 7);
        assert_eq!(open.iterates.len(), before + 7);

        // start() clamps precision to ≥53 (max mutant → wrong tiny precision).
        let tiny_prec = ReferenceOrbit::start(&(IntExp::ZERO, IntExp::ZERO), 1);
        assert!(tiny_prec.precision_bits >= 53);

        // bits_for_zoom: zoom+ppu+32 with floor 53 (not replace→1 / drop saturating_add).
        assert_eq!(bits_for_zoom(0, 8), 53);
        assert_eq!(bits_for_zoom(20, 8), 60); // 20+8+32
        assert_ne!(bits_for_zoom(20, 8), 1);
        assert_ne!(bits_for_zoom(20, 8), 28); // missing +32
        assert_ne!(bits_for_zoom(20, 8), 20); // missing both adds
    }

    #[test]
    fn get_wraps_periodic_indices_not_linear() {
        let p2 = ReferenceOrbit::compute(&(IntExp::from(-1), IntExp::ZERO), 128, 8);
        assert_eq!(p2.period, Some(2));
        assert_eq!(p2.preperiod, 0);
        // n < preperiod uses n; else preperiod + (n-preperiod)%period
        assert_eq!(p2.get(0), p2.get(2));
        assert_eq!(p2.get(1), p2.get(3));
        assert_ne!(p2.get(0), p2.get(1));
        // %→+ / %→/ would break large n:
        assert_eq!(p2.get(10_000), p2.get(0));
        assert_eq!(p2.get(10_001), p2.get(1));
        // %→/ would map 5→2 and wrongly equate get(1) with get(0).
        assert_ne!(p2.get(5), p2.get(5 / 2));
        // Large even/odd wrap (kills %→+ which would drift with n).
        assert_eq!(p2.get(4), p2.get(0));
        assert_eq!(p2.get(6), p2.get(0));
        assert_eq!(p2.get(7), p2.get(1));
        assert_ne!(p2.get(1_000_001), p2.get(1_000_000));

        let m = ReferenceOrbit::compute(&(IntExp::from(-2), IntExp::ZERO), 128, 8);
        assert_eq!(m.period, Some(1));
        assert_eq!(m.preperiod, 2);
        // For n>=2: index = 2 + (n-2)%1 = 2
        assert_eq!(m.get(2), m.get(99));
        assert_eq!(m.get(2), m.get(10_000));
        // Preperiod seats are distinct indices 0 and 1.
        assert_ne!(m.get(0), m.get(1));
        assert_eq!(m.get(1).unwrap().re.to_f64(), -2.0);
    }

    #[test]
    fn extend_stops_when_period_or_escaped_and_counts_steps() {
        let mut p2 = ReferenceOrbit::compute(&(IntExp::from(-1), IntExp::ZERO), 128, 32);
        assert!(p2.period.is_some());
        let len_after = p2.iterates.len();
        p2.extend(50);
        assert_eq!(p2.iterates.len(), len_after);
        assert_eq!(p2.extend_for(50, Duration::from_secs(1)), 0);

        let mut esc = ReferenceOrbit::compute(&(IntExp::from(2), IntExp::ZERO), 128, 20);
        assert!(esc.escaped);
        let esc_len = esc.iterates.len();
        esc.extend(10);
        assert_eq!(esc.iterates.len(), esc_len);
        assert_eq!(esc.extend_for(10, Duration::from_secs(1)), 0);

        let mut open =
            ReferenceOrbit::start(&(IntExp::from(3).shift(-4), IntExp::from(1).shift(-3)), 128);
        let n = open.extend_for(5, Duration::from_secs(1));
        assert_eq!(n, 5);
        assert!(open.iterates.len() >= 6);
        // Escape threshold uses |z|² > 4 (not >= / == / <).
        let far = ReferenceOrbit::compute(&(IntExp::from(10), IntExp::ZERO), 64, 5);
        assert!(far.escaped);
    }

    #[test]
    fn zero_orbit_floor_and_bits_clamp() {
        let z = ReferenceOrbit::zero_orbit();
        assert_eq!(z.period, Some(1));
        assert_eq!(z.preperiod, 0);
        assert_eq!(z.get(0), Some(ComplexFloatExp::ZERO));
        assert_eq!(z.get(1), Some(ComplexFloatExp::ZERO));
        assert_eq!(z.get(999), Some(ComplexFloatExp::ZERO));

        assert_eq!(bits_for_zoom(-100, 8), 53); // floor at 53
        assert!(bits_for_zoom(i64::MAX / 4, 8) <= u32::MAX);
        assert_ne!(bits_for_zoom(100, 8), bits_for_zoom(100, 9));
        assert_eq!(bits_for_zoom(100, 8), 140); // 100+8+32
    }

    #[test]
    fn observe_cycle_detects_period_two_at_minus_one() {
        // Full path already asserts period==2; pin that further extend is a no-op
        // and that resolve_cycle_step equality uses == not != on both components.
        let orbit = ReferenceOrbit::compute(&(IntExp::from(-1), IntExp::ZERO), 256, 64);
        assert_eq!(orbit.period, Some(2));
        assert_eq!(orbit.preperiod, 0);
        let z0 = orbit.get(0).unwrap();
        let z1 = orbit.get(1).unwrap();
        let z2 = orbit.get(2).unwrap();
        assert_eq!(z0, z2);
        assert_ne!(z0, z1);
        // Stored iterate at n=1 is c for Mandelbrot start from 0.
        assert_eq!(z1.re.to_f64(), -1.0);
        assert_eq!(z1.im.to_f64(), 0.0);
    }
}
