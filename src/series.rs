//! Series approximation (Martin / Heiland-Allen): skip a safe prefix of
//! per-pixel delta iterations using coefficients that depend only on the
//! reference.
//!
//! Production contract (`r[cz.depth.series-approximation+1]`, developer
//! 2026-08-11): always-on seat-init probe O(log N) / nearly free when skip is
//! useless; one series step per reference iterate fused into the reference
//! loop; flat coeff storage; airtight hot-path habits.

use crate::floatexp::{ComplexFloatExp, FloatExp};

/// Order policy used at publish / finish time (deep-biased, modest).
#[inline(always)]
pub fn series_order_for(orbit_len: usize) -> usize {
    if orbit_len < 16 {
        2
    } else if orbit_len < 256 {
        4
    } else {
        8
    }
}

/// Coefficients `a_{k,n}` for k = 1..order at each stored reference index n.
/// Flat layout: row `n` lives at `coeffs[n * order .. (n + 1) * order]`.
/// Index 0 is unused (Z_0); pixel n aligns with `ReferenceOrbit::get(n)`.
// r[impl cz.depth.series-approximation+1]
#[derive(Clone, Debug)]
pub struct SeriesApproximation {
    pub order: usize,
    /// `rows * order` coefficients, row-major.
    pub coeffs: Vec<ComplexFloatExp>,
    pub rows: usize,
}

impl SeriesApproximation {
    #[inline(always)]
    fn row(&self, n: usize) -> Option<&[ComplexFloatExp]> {
        if n >= self.rows || self.order == 0 {
            return None;
        }
        let start = n * self.order;
        Some(&self.coeffs[start..start + self.order])
    }

    /// Build coefficients from stored reference iterates (test / replay helper).
    /// Production builds via [`SeriesBuilder`] fused into orbit extension.
    pub fn from_iterates(iterates: &[ComplexFloatExp], order: usize) -> Option<Self> {
        let order = order.clamp(1, 16);
        let len = iterates.len();
        if len < 2 {
            return None;
        }
        let mut builder = SeriesBuilder::new(order);
        // Row 0 already present. Seed at Z_1, then step with Z_n for n = 1..len-2
        // to produce rows 2..len-1 (same recurrence as the prior sketch).
        builder.seed_at_c();
        for n in 1..len.saturating_sub(1) {
            builder.step(iterates[n]);
        }
        builder.finish_with_order(order)
    }

    /// Convenience wrapper: build from a finished [`crate::reference::ReferenceOrbit`].
    pub fn from_orbit(orbit: &crate::reference::ReferenceOrbit, order: usize) -> Option<Self> {
        Self::from_iterates(&orbit.iterates, order)
    }

    /// Evaluate Σ a_k δc^k at series index `n` (must be in range).
    pub fn evaluate(&self, n: usize, dc: ComplexFloatExp) -> Option<ComplexFloatExp> {
        let row = self.row(n)?;
        let mut acc = ComplexFloatExp::ZERO;
        let mut pow = dc;
        for a_k in row {
            acc = acc + (*a_k) * pow;
            pow = pow * dc;
        }
        Some(acc)
    }

    /// Largest n in 1..max_n that stays absorbed (Heiland-Allen-style tail bound).
    /// Binary search — O(log max_n) evaluations.
    pub fn safe_skip(&self, dc: ComplexFloatExp, max_n: usize) -> usize {
        self.safe_skip_counted(dc, max_n).0
    }

    /// Like [`Self::safe_skip`], also returning how many `evaluate` calls ran.
    pub fn safe_skip_counted(&self, dc: ComplexFloatExp, max_n: usize) -> (usize, u32) {
        if self.rows < 2 || self.order == 0 {
            return (0, 0);
        }
        let max_n = max_n.min(self.rows.saturating_sub(1)).max(1);
        let dc_norm = dc.norm_squared();
        let mut evals = 0u32;

        // Series is only useful for small δc. Large δc → free no-op (one access
        // not even required): avoids inflating escape_time on far exterior seats
        // and keeps shallow/home overhead in the noise.
        if dc_norm > FloatExp::from(1e-4) {
            return (1, 0);
        }

        let mut is_safe = |n: usize, evals: &mut u32| -> bool {
            let Some(dz) = self.evaluate(n, dc) else {
                return false;
            };
            *evals = evals.saturating_add(1);
            let dz_n = dz.norm_squared();
            if dz_n > FloatExp::from(1e6) {
                return false;
            }
            let Some(row) = self.row(n) else {
                return false;
            };
            let last = row.last().copied().unwrap_or(ComplexFloatExp::ZERO);
            let mut pow = FloatExp::ONE;
            for _ in 0..self.order {
                pow = pow * dc_norm;
            }
            let tail = last.norm_squared() * pow;
            !(dz_n > FloatExp::ZERO && tail > dz_n * FloatExp::from(1e-6))
        };

        // Probe n=1 once: easy / large-δc cases exit after a single access.
        if !is_safe(1, &mut evals) {
            return (1, evals);
        }
        if max_n == 1 {
            return (1, evals);
        }
        if is_safe(max_n, &mut evals) {
            return (max_n, evals);
        }

        // Largest safe n in [1, max_n): binary search.
        let mut lo = 1usize;
        let mut hi = max_n.saturating_sub(1);
        let mut best = 1usize;
        while lo <= hi {
            let mid = lo + (hi - lo) / 2;
            if is_safe(mid, &mut evals) {
                best = mid;
                lo = mid.saturating_add(1);
            } else if mid == 0 {
                break;
            } else {
                hi = mid - 1;
            }
        }
        (best, evals)
    }
}

/// Incremental series coeff builder — one step per reference iterate.
#[derive(Clone, Debug)]
pub struct SeriesBuilder {
    order: usize,
    /// Flat rows accumulated so far (starts with row 0 = zeros).
    coeffs: Vec<ComplexFloatExp>,
    rows: usize,
    /// Current coefficient vector a_k (length `order`) for the last row.
    current: Vec<ComplexFloatExp>,
}

impl SeriesBuilder {
    pub fn new(order: usize) -> Self {
        let order = order.clamp(1, 16);
        let mut coeffs = vec![ComplexFloatExp::ZERO; order];
        Self {
            order,
            coeffs,
            rows: 1,
            current: vec![ComplexFloatExp::ZERO; order],
        }
    }

    /// After Z_1 = c_ref is stored: a_1 = 1, higher a_k = 0.
    pub fn seed_at_c(&mut self) {
        self.current.fill(ComplexFloatExp::ZERO);
        self.current[0] = ComplexFloatExp::new(FloatExp::ONE, FloatExp::ZERO);
        self.push_current_row();
    }

    /// Advance from row n → n+1 using reference iterate Z_n.
    pub fn step(&mut self, z_n: ComplexFloatExp) {
        let two = ComplexFloatExp::new(FloatExp::TWO, FloatExp::ZERO);
        let one = ComplexFloatExp::new(FloatExp::ONE, FloatExp::ZERO);
        let a = &self.current;
        let mut next = vec![ComplexFloatExp::ZERO; self.order];
        // a_1' = 2 Z a_1 + 1
        next[0] = z_n * a[0] * two + one;
        for k in 2..=self.order {
            let mut sum = ComplexFloatExp::ZERO;
            for j in 1..k {
                sum = sum + a[j - 1] * a[k - j - 1];
            }
            next[k - 1] = z_n * a[k - 1] * two + sum;
        }
        self.current = next;
        self.push_current_row();
    }

    #[inline(always)]
    fn push_current_row(&mut self) {
        self.coeffs.extend_from_slice(&self.current);
        self.rows += 1;
    }

    /// Finish, truncating per-row order to `series_order_for(rows)` (or `order`).
    pub fn finish(self) -> Option<SeriesApproximation> {
        let order = series_order_for(self.rows).min(self.order).max(1);
        self.finish_with_order(order)
    }

    pub fn finish_with_order(self, order: usize) -> Option<SeriesApproximation> {
        let order = order.clamp(1, self.order);
        if self.rows < 2 {
            return None;
        }
        if order == self.order {
            return Some(SeriesApproximation {
                order: self.order,
                coeffs: self.coeffs,
                rows: self.rows,
            });
        }
        // Truncate each row to the first `order` coefficients.
        let mut coeffs = Vec::with_capacity(self.rows * order);
        for n in 0..self.rows {
            let start = n * self.order;
            coeffs.extend_from_slice(&self.coeffs[start..start + order]);
        }
        Some(SeriesApproximation {
            order,
            coeffs,
            rows: self.rows,
        })
    }

    pub fn rows(&self) -> usize {
        self.rows
    }

    pub fn order(&self) -> usize {
        self.order
    }
}

#[cfg(test)]
mod mutant_kill {
    //! Thought-killed pins for `series.rs` (builder recurrence, evaluate,
    //! logarithmic safe_skip).
    use super::*;
    use crate::reference::ReferenceOrbit;
    use crate::utils::IntExp;

    fn short_escape_orbit() -> ReferenceOrbit {
        ReferenceOrbit::compute(&(IntExp::from(1).shift(-2), IntExp::ZERO), 128, 24)
    }

    fn from_orbit(orbit: &ReferenceOrbit, order: usize) -> Option<SeriesApproximation> {
        SeriesApproximation::from_iterates(&orbit.iterates, order)
    }

    #[test]
    fn from_orbit_rejects_too_short_and_clamps_order() {
        let tiny = ReferenceOrbit::start(&(IntExp::ZERO, IntExp::ZERO), 53);
        assert!(from_orbit(&tiny, 4).is_none());

        let orbit = short_escape_orbit();
        assert!(orbit.iterates.len() >= 2);
        let s = from_orbit(&orbit, 100).expect("series");
        assert_eq!(s.order, 16); // clamp high
        let s1 = from_orbit(&orbit, 0).expect("series");
        assert_eq!(s1.order, 1); // clamp low
        assert!(from_orbit(&orbit, 2).is_some());
    }

    #[test]
    fn from_orbit_seeds_a1_and_recurs_2z_a_plus_1() {
        let orbit = short_escape_orbit();
        let s = from_orbit(&orbit, 2).expect("series");
        assert_eq!(s.order, 2);
        assert_eq!(s.row(0).unwrap()[0], ComplexFloatExp::ZERO);
        assert_eq!(
            s.row(1).unwrap()[0],
            ComplexFloatExp::new(FloatExp::ONE, FloatExp::ZERO)
        );
        assert_eq!(s.row(1).unwrap()[1], ComplexFloatExp::ZERO);

        if s.rows > 2 {
            let z1 = orbit.iterates[1];
            let a1 = s.row(1).unwrap()[0];
            let two = ComplexFloatExp::new(FloatExp::TWO, FloatExp::ZERO);
            let expect_a1 = z1 * a1 * two + ComplexFloatExp::new(FloatExp::ONE, FloatExp::ZERO);
            assert_eq!(s.row(2).unwrap()[0], expect_a1);
            let wrong = z1 * a1 + two + ComplexFloatExp::new(FloatExp::ONE, FloatExp::ZERO);
            assert_ne!(s.row(2).unwrap()[0], wrong);
            let wrong_sub = z1 * a1 * two - ComplexFloatExp::new(FloatExp::ONE, FloatExp::ZERO);
            let wrong_mul = z1 * a1 * two * ComplexFloatExp::new(FloatExp::ONE, FloatExp::ZERO);
            assert_ne!(s.row(2).unwrap()[0], wrong_sub);
            assert_ne!(s.row(2).unwrap()[0], wrong_mul);
            let expect_a2 = a1 * a1;
            assert_eq!(s.row(2).unwrap()[1], expect_a2);
            assert_ne!(s.row(2).unwrap()[1], a1 + a1);
            assert_ne!(s.row(2).unwrap()[1], a1 - a1);
        }
    }

    #[test]
    fn evaluate_is_power_series_not_constant_none() {
        let orbit = short_escape_orbit();
        let s = from_orbit(&orbit, 3).expect("series");
        let dc = ComplexFloatExp::new(FloatExp::from(1e-3), FloatExp::ZERO);
        assert!(s.evaluate(s.rows, dc).is_none());
        let v = s.evaluate(1, dc).expect("eval");
        assert!((v.re.to_f64() - 1e-3).abs() < 1e-9, "got {}", v.re.to_f64());
        assert_ne!(v, ComplexFloatExp::ZERO);
        let v2 = s
            .evaluate(1, ComplexFloatExp::new(FloatExp::from(2e-3), FloatExp::ZERO))
            .unwrap();
        assert!(v2.re.to_f64().abs() > v.re.to_f64().abs());
        if s.rows > 2 {
            let v3 = s.evaluate(2, dc).expect("eval2");
            assert_ne!(v3, s.row(2).unwrap()[0] + dc);
            assert_ne!(v3, s.row(2).unwrap()[0] * dc + s.row(2).unwrap()[0]);
        }
    }

    #[test]
    fn safe_skip_not_constant_and_respects_bounds() {
        let empty = SeriesApproximation {
            order: 1,
            coeffs: vec![],
            rows: 0,
        };
        assert_eq!(empty.safe_skip(ComplexFloatExp::ZERO, 10), 0);
        let one = SeriesApproximation {
            order: 1,
            coeffs: vec![ComplexFloatExp::ZERO],
            rows: 1,
        };
        assert_eq!(one.safe_skip(ComplexFloatExp::ZERO, 10), 0);

        let orbit = short_escape_orbit();
        let s = from_orbit(&orbit, 4).expect("series");
        let dc = ComplexFloatExp::new(FloatExp::from(1e-8), FloatExp::ZERO);
        let n = s.safe_skip(dc, 1000);
        assert!(n >= 1);
        assert!(n < s.rows);
        let huge = ComplexFloatExp::new(FloatExp::from(1e3), FloatExp::ZERO);
        let n_huge = s.safe_skip(huge, 1000);
        assert!(n_huge <= n);
        assert_eq!(s.safe_skip(dc, 1), 1);
        let n_over = s.safe_skip(dc, usize::MAX);
        assert!(n_over < s.rows);
        assert!(n_over >= 1);
        assert_eq!(s.safe_skip(dc, 0), 1);
        let mid = ComplexFloatExp::new(FloatExp::from(1e-2), FloatExp::ZERO);
        let n_mid = s.safe_skip(mid, 1000);
        assert!(n >= n_mid, "tiny dc skip {n} should be ≥ mid {n_mid}");
    }

    #[test]
    // r[verify cz.depth.series-approximation+1]
    fn series_safe_skip_eval_count_is_logarithmic() {
        // Long synthetic row count via a long escaping exterior orbit.
        let orbit = ReferenceOrbit::compute(&(IntExp::from(1).shift(-3), IntExp::ZERO), 128, 4096);
        let s = from_orbit(&orbit, 4).expect("series");
        assert!(s.rows > 64, "need a long series for the log pin; rows={}", s.rows);
        let dc = ComplexFloatExp::new(FloatExp::from(1e-12), FloatExp::ZERO);
        let max_n = s.rows.saturating_sub(1);
        let (skip, evals) = s.safe_skip_counted(dc, max_n);
        assert!(skip >= 1);
        // Binary search: ≤ 2 + ceil(log2(max_n)) with a small constant pad.
        let log_budget = 2 + (max_n as f64).log2().ceil() as u32 + 4;
        assert!(
            evals <= log_budget,
            "evals={evals} > log budget {log_budget} for max_n={max_n} skip={skip}"
        );
        // Must beat a linear scan of every index.
        assert!(evals < (max_n as u32) / 4, "evals={evals} not clearly sublinear vs {max_n}");
    }

    #[test]
    // r[verify cz.depth.series-approximation+1]
    fn series_shallow_probe_stays_nearly_free() {
        let orbit = ReferenceOrbit::compute(&(IntExp::from(1).shift(-3), IntExp::ZERO), 128, 2048);
        let s = from_orbit(&orbit, 4).expect("series");
        let max_n = s.rows.saturating_sub(1);
        // Large δc: useless skip — must exit with zero/near-zero evals.
        let huge = ComplexFloatExp::new(FloatExp::from(1e2), FloatExp::ZERO);
        let (skip_huge, evals_huge) = s.safe_skip_counted(huge, max_n);
        assert!(skip_huge <= 1);
        assert!(
            evals_huge <= 1,
            "shallow/useless probe must stay nearly free; evals={evals_huge}"
        );
        // Home-like modest δc on a short max_n=1 probe: single access path.
        let homeish = ComplexFloatExp::new(FloatExp::from(1e-2), FloatExp::ZERO);
        let (_s1, e1) = s.safe_skip_counted(homeish, 1);
        assert_eq!(e1, 1);
    }

    #[test]
    // r[verify cz.depth.series-approximation+1]
    fn series_deep_skip_is_material_on_long_orbit() {
        let orbit = ReferenceOrbit::compute(&(IntExp::from(1).shift(-3), IntExp::ZERO), 128, 4096);
        let s = from_orbit(&orbit, 4).expect("series");
        let dc = ComplexFloatExp::new(FloatExp::from(1e-14), FloatExp::ZERO);
        let skip = s.safe_skip(dc, s.rows.saturating_sub(1));
        assert!(
            skip > 16,
            "deep tiny δc must admit a material skip; got {skip} rows={}",
            s.rows
        );
        // Remaining series value must exist (skip does not invent a final answer).
        assert!(s.evaluate(skip, dc).is_some());
    }

    #[test]
    fn builder_matches_from_orbit_fused_steps() {
        let orbit = short_escape_orbit();
        let via_from = from_orbit(&orbit, 4).expect("from");
        let mut b = SeriesBuilder::new(4);
        b.seed_at_c();
        for n in 1..orbit.iterates.len().saturating_sub(1) {
            b.step(orbit.iterates[n]);
        }
        let via_builder = b.finish_with_order(4).expect("builder");
        assert_eq!(via_from.order, via_builder.order);
        assert_eq!(via_from.rows, via_builder.rows);
        assert_eq!(via_from.coeffs, via_builder.coeffs);
    }

    #[test]
    fn mutant_kill_series_from_orbit_evaluate_safe_skip() {
        from_orbit_rejects_too_short_and_clamps_order();
        from_orbit_seeds_a1_and_recurs_2z_a_plus_1();
        evaluate_is_power_series_not_constant_none();
        safe_skip_not_constant_and_respects_bounds();
        series_safe_skip_eval_count_is_logarithmic();
        series_shallow_probe_stays_nearly_free();
        series_deep_skip_is_material_on_long_orbit();
        builder_matches_from_orbit_fused_steps();
    }
}
