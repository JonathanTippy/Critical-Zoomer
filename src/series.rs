//! Deferred: not on the live path until membership pins stay green
//! (`pin_exterior_not_marked_in_at_zoom_52`, `pin_not_blocky_delta_c_at_zoom_49`).
//!
//! Simple series approximation (Martin / Heiland-Allen): skip a safe prefix of
//! per-pixel delta iterations using coefficients that depend only on the reference.

use crate::floatexp::{ComplexFloatExp, FloatExp};
use crate::reference::ReferenceOrbit;

/// Coefficients `a_{k,n}` for k = 1..order at each stored reference index n.
/// `coeffs[n][k-1]` is a_{k,n}. Index 0 is unused (Z_0); pixel n aligns with
/// `ReferenceOrbit::get(n)`.
// r[impl cz.depth.series-approximation+1]
#[derive(Clone, Debug)]
pub struct SeriesApproximation {
    pub order: usize,
    /// Parallel to orbit iterates: coeffs[i] has length `order`.
    pub coeffs: Vec<Vec<ComplexFloatExp>>,
}

impl SeriesApproximation {
    /// Build coefficients from a finished reference orbit. Order is clamped to a
    /// small practical range; high orders at low zoom are a glitch risk.
    pub fn from_orbit(orbit: &ReferenceOrbit, order: usize) -> Option<Self> {
        let order = order.clamp(1, 16);
        let len = orbit.iterates.len();
        if len < 2 {
            return None;
        }
        let mut coeffs = Vec::with_capacity(len);
        // n = 0: a_1 = 0 conceptually before first iterate; store zeros.
        coeffs.push(vec![ComplexFloatExp::ZERO; order]);

        // At n=1 (Z_1 = c_ref): a_1 = 1, higher a_k = 0 for Mandelbrot start.
        let mut a = vec![ComplexFloatExp::ZERO; order];
        a[0] = ComplexFloatExp::new(FloatExp::ONE, FloatExp::ZERO);
        coeffs.push(a.clone());

        let two = ComplexFloatExp::new(FloatExp::TWO, FloatExp::ZERO);
        for n in 1..len.saturating_sub(1) {
            let z_n = orbit.iterates[n];
            let mut next = vec![ComplexFloatExp::ZERO; order];
            // a_1' = 2 Z a_1 + 1
            next[0] = z_n * a[0] * two + ComplexFloatExp::new(FloatExp::ONE, FloatExp::ZERO);
            for k in 2..=order {
                let mut sum = ComplexFloatExp::ZERO;
                for j in 1..k {
                    sum = sum + a[j - 1] * a[k - j - 1];
                }
                next[k - 1] = z_n * a[k - 1] * two + sum;
            }
            a = next;
            coeffs.push(a.clone());
        }
        Some(Self { order, coeffs })
    }

    /// Evaluate Σ a_k δc^k at series index `n` (must be in range).
    pub fn evaluate(&self, n: usize, dc: ComplexFloatExp) -> Option<ComplexFloatExp> {
        let row = self.coeffs.get(n)?;
        let mut acc = ComplexFloatExp::ZERO;
        let mut pow = dc;
        for a_k in row {
            acc = acc + (*a_k) * pow;
            pow = pow * dc;
        }
        Some(acc)
    }

    /// Largest n in 1..max_n where the next quadratic term stays absorbed
    /// (crude Heiland-Allen style bound using |a_order|·|δc|^order vs |δz|).
    pub fn safe_skip(&self, dc: ComplexFloatExp, max_n: usize) -> usize {
        if self.coeffs.len() < 2 {
            return 0;
        }
        let max_n = max_n.min(self.coeffs.len().saturating_sub(1)).max(1);
        let dc_norm = dc.norm_squared();
        let mut best = 1usize;
        for n in 1..=max_n {
            let Some(dz) = self.evaluate(n, dc) else {
                break;
            };
            let dz_n = dz.norm_squared();
            // Truncation probe: last coefficient contribution.
            let last = self.coeffs[n].last().copied().unwrap_or(ComplexFloatExp::ZERO);
            let mut pow = FloatExp::ONE;
            for _ in 0..self.order {
                pow = pow * dc_norm;
            }
            let tail = last.norm_squared() * pow;
            // Require tail << |δz|² (or small absolute).
            if dz_n > FloatExp::ZERO && tail > dz_n * FloatExp::from(1e-6) {
                break;
            }
            if dz_n > FloatExp::from(1e6) {
                break;
            }
            best = n;
        }
        best
    }
}
