use rug::Float;

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
}

impl ReferenceOrbit {
    // r[impl cz.depth.reference-low-storage+1]
    pub fn compute(c: &(IntExp, IntExp), precision_bits: u32, max_iterations: u32) -> Self {
        let precision_bits = precision_bits.max(53);
        let c = (
            intexp_to_float(&c.0, precision_bits),
            intexp_to_float(&c.1, precision_bits),
        );
        let state = (
            Float::with_val(precision_bits, 0),
            Float::with_val(precision_bits, 0),
        );
        let mut out = Self {
            c,
            iterates: vec![stored(&state)],
            state,
            precision_bits,
            period: None,
            preperiod: 0,
            escaped: false,
        };
        out.extend(max_iterations);
        out
    }

    pub fn extend(&mut self, additional_iterations: u32) {
        if self.period.is_some() || self.escaped {
            return;
        }

        // Full-precision history is temporary and only needed to prove an
        // exact preperiod/cycle during this bout. Stored history remains low.
        let mut exact = Vec::with_capacity(additional_iterations as usize + 1);
        exact.push((self.state.0.clone(), self.state.1.clone()));

        for _ in 0..additional_iterations {
            let next = iterate(&self.state, &self.c, self.precision_bits);
            let absolute_index = self.iterates.len() as u32;

            if let Some(local_first) = exact.iter().position(|z| z.0 == next.0 && z.1 == next.1) {
                let exact_start_index = absolute_index - exact.len() as u32 + local_first as u32;
                self.preperiod = exact_start_index;
                self.period = Some(absolute_index - exact_start_index);
                self.state = next;
                // The repeated endpoint is not stored; indexing wraps to the
                // first identical value.
                return;
            }

            self.iterates.push(stored(&next));
            self.state = (next.0.clone(), next.1.clone());
            exact.push(next);

            let norm = Float::with_val(
                self.precision_bits,
                Float::with_val(self.precision_bits, &self.state.0 * &self.state.0)
                    + Float::with_val(self.precision_bits, &self.state.1 * &self.state.1),
            );
            if norm > 4 {
                self.escaped = true;
                return;
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
}
