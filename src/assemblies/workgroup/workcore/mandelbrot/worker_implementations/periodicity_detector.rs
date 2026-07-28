use crate::assemblies::workgroup::structs::mandelbrotable::*;
use crate::assemblies::workgroup::workcore::mandelbrot::*;
use crate::constants::PERIOD_CONFIRMATION_ITERATIONS;

pub struct StandardPeriodicityDetector<T: Mandelbrotable> {
    pub(crate) checkpoint_z: (T, T)
    , pub(crate) steps_since_checkpoint: u64
    , pub(crate) next_checkpoint_iteration: u64
    , pub(crate) detected_period: Option<u64>
}

impl<T: Mandelbrotable> PeriodicityDetector<T> for StandardPeriodicityDetector<T> {
    fn init(_iteration_count: u64, z: (T, T), _derivative: (T, T)) -> Self {
        StandardPeriodicityDetector {
            checkpoint_z: z
            , steps_since_checkpoint: 0
            , next_checkpoint_iteration: 1
            , detected_period: None
        }
    }

    fn check_periodicity(
        &mut self
        , c: (T, T)
        , z: (T, T)
        , derivative: (T, T)
        , iteration_count: u64
        , epsilon: T
    ) -> Option<u64> {
        if let Some(period) = self.detected_period {
            return Some(period);
        }

        self.steps_since_checkpoint = self.steps_since_checkpoint.saturating_add(1);

        if self.steps_since_checkpoint > 0
            && near(z, self.checkpoint_z, epsilon)
            && confirm_twins_off_to_side(
                c
                , z
                , derivative
                , self.checkpoint_z
                , epsilon
            )
        {
            if let Some(period) = minimal_period(c, z, self.steps_since_checkpoint, epsilon) {
                self.detected_period = Some(period);
                return Some(period);
            }
        }

        if iteration_count == self.next_checkpoint_iteration {
            self.checkpoint_z = z;
            self.steps_since_checkpoint = 0;
            self.next_checkpoint_iteration = self.next_checkpoint_iteration.saturating_mul(2);
        }

        None
    }

    fn is_periodic(&self) -> bool {
        self.detected_period.is_some()
    }

    fn detected_period(&self) -> Option<u64> {
        self.detected_period
    }
}

fn confirm_twins_off_to_side<T: Mandelbrotable>(
    c: (T, T)
    , z: (T, T)
    , derivative: (T, T)
    , checkpoint_z: (T, T)
    , epsilon: T
) -> bool {
    let mut live_z = z;
    let mut live_derivative = derivative;
    let mut twin_z = checkpoint_z;
    let mut twin_derivative = derivative;
    for _ in 0..PERIOD_CONFIRMATION_ITERATIONS {
        advance_orbit_step(&mut live_z, &mut live_derivative, c);
        advance_orbit_step(&mut twin_z, &mut twin_derivative, c);
        if !near(live_z, twin_z, epsilon) || !near(live_derivative, twin_derivative, epsilon) {
            return false;
        }
    }
    true
}

fn minimal_period<T: Mandelbrotable>(
    c: (T, T)
    , z0: (T, T)
    , candidate: u64
    , epsilon: T
) -> Option<u64> {
    if candidate == 0 {
        return None;
    }
    if candidate == 1 {
        return Some(1);
    }
    let reduce_eps = epsilon.to_f64().max(1e-3);
    for period in 1..=candidate {
        if closes_after(c, z0, period, reduce_eps) {
            return Some(period);
        }
    }
    None
}

fn closes_after<T: Mandelbrotable>(
    c: (T, T)
    , z0: (T, T)
    , period: u64
    , epsilon: f64
) -> bool {
    let mut z = z0;
    let mut derivative = (T::ONE, T::ZERO);
    for _ in 0..period {
        advance_orbit_step(&mut z, &mut derivative, c);
    }
    (z.0.to_f64() - z0.0.to_f64()).abs() < epsilon
        && (z.1.to_f64() - z0.1.to_f64()).abs() < epsilon
}

fn near<T: Mandelbrotable>(a: (T, T), b: (T, T), epsilon: T) -> bool {
    component_near(a.0, b.0, epsilon) && component_near(a.1, b.1, epsilon)
}

fn component_near<T: Mandelbrotable>(a: T, b: T, epsilon: T) -> bool {
    let d = if a > b { a - b } else { b - a };
    d < epsilon
}

fn advance_orbit_step<T: Mandelbrotable>(
    z: &mut (T, T)
    , derivative: &mut (T, T)
    , c: (T, T)
) {
    let z_re = z.0;
    let z_im = z.1;
    let d_re = derivative.0;
    let d_im = derivative.1;
    let new_d_re = T::TWO * (z_re * d_re - z_im * d_im);
    let new_d_im = T::TWO * (z_re * d_im + z_im * d_re);
    *derivative = (new_d_re, new_d_im);
    *z = (
        z_re * z_re - z_im * z_im + c.0
        , T::TWO * z_re * z_im + c.1
    );
}

pub type CpuPeriodicityDetector = StandardPeriodicityDetector<f64>;
pub type GpuPeriodicityDetector = StandardPeriodicityDetector<f32>;

pub fn detect_period_for_c(c: (f64, f64), max_iters: u64) -> Option<u64> {
    let mut z = (0.0, 0.0);
    let mut d = (1.0, 0.0);
    let mut detector = CpuPeriodicityDetector::init(0, z, d);
    let epsilon = period_epsilon(c);
    let mut iteration_count = 0u64;
    for _ in 0..max_iters {
        let old_z = z;
        let rad = old_z.0 * old_z.0 + old_z.1 * old_z.1;
        if iteration_count > 0 && rad > 4.0 {
            return None;
        }
        d = (
            2.0 * (old_z.0 * d.0 - old_z.1 * d.1)
            , 2.0 * (old_z.0 * d.1 + old_z.1 * d.0)
        );
        z = (
            old_z.0 * old_z.0 - old_z.1 * old_z.1 + c.0
            , 2.0 * old_z.0 * old_z.1 + c.1
        );
        iteration_count += 1;
        if let Some(period) = detector.check_periodicity(c, z, d, iteration_count, epsilon) {
            return Some(period);
        }
        let rad = z.0 * z.0 + z.1 * z.1;
        if rad > 4.0 {
            return None;
        }
    }
    None
}

pub fn period_epsilon(c: (f64, f64)) -> f64 {
    1e-12f64.max(c.0.abs().max(c.1.abs()) * 1e-6)
}

pub fn cardioid_c_from_mu(mu: (f64, f64)) -> (f64, f64) {
    let (a, b) = mu;
    let half_a = 0.5 * a;
    let half_b = 0.5 * b;
    let one_minus_half = (1.0 - half_a, -half_b);
    (
        half_a * one_minus_half.0 - half_b * one_minus_half.1
        , half_a * one_minus_half.1 + half_b * one_minus_half.0
    )
}

pub fn in_period_two_bulb(c: (f64, f64)) -> bool {
    let dx = c.0 + 1.0;
    let dy = c.1;
    dx * dx + dy * dy < 0.25 * 0.25
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_exact_periods(
        label: &str
        , expected: u64
        , samples: &[(f64, f64)]
        , max_iters: u64
    ) {
        let mut failures: Vec<String> = Vec::new();
        for &(re, im) in samples {
            let c = (re, im);
            match detect_period_for_c(c, max_iters) {
                Some(p) if p == expected => {}
                Some(p) => {
                    failures.push(format!(
                        "{label}: c=({re:.17}, {im:.17}) expected period {expected}, got {p}"
                    ));
                }
                None => {
                    failures.push(format!(
                        "{label}: c=({re:.17}, {im:.17}) expected period {expected}, got None (escaped or unresolved within {max_iters})"
                    ));
                }
            }
        }
        if !failures.is_empty() {
            let n = failures.len();
            let head: Vec<&str> = failures.iter().take(40).map(|s| s.as_str()).collect();
            panic!(
                "{n} period failures for {label} (showing up to 40):\n{}",
                head.join("\n")
            );
        }
    }

    fn sample_main_cardioid(grid: usize) -> Vec<(f64, f64)> {
        let mut out = Vec::new();
        for iy in 0..grid {
            for ix in 0..grid {
                let u = -0.999 + 1.998 * (ix as f64) / (grid as f64 - 1.0);
                let v = -0.999 + 1.998 * (iy as f64) / (grid as f64 - 1.0);
                if u * u + v * v >= 0.999 * 0.999 {
                    continue;
                }
                out.push(cardioid_c_from_mu((u, v)));
            }
        }
        out
    }

    fn sample_period_two_bulb(grid: usize) -> Vec<(f64, f64)> {
        let mut out = Vec::new();
        let radius = 0.249;
        for iy in 0..grid {
            for ix in 0..grid {
                let x = -1.0 - radius + 2.0 * radius * (ix as f64) / (grid as f64 - 1.0);
                let y = -radius + 2.0 * radius * (iy as f64) / (grid as f64 - 1.0);
                let c = (x, y);
                if !in_period_two_bulb(c) {
                    continue;
                }
                if (c.0 + 0.75).abs() < 1e-3 && c.1.abs() < 1e-3 {
                    continue;
                }
                out.push(c);
            }
        }
        out
    }

    fn sample_cardioid_boundary_ring(radii: usize, angles: usize) -> Vec<(f64, f64)> {
        let mut out = Vec::new();
        for ir in 0..radii {
            let r = 0.90 + 0.099 * (ir as f64) / ((radii - 1) as f64);
            for it in 0..angles {
                let theta = std::f64::consts::TAU * (it as f64) / (angles as f64);
                let u = r * theta.cos();
                let v = r * theta.sin();
                out.push(cardioid_c_from_mu((u, v)));
            }
        }
        out
    }

    fn sample_bulb_boundary_ring(radii: usize, angles: usize) -> Vec<(f64, f64)> {
        let mut out = Vec::new();
        for ir in 0..radii {
            let r = 0.20 + 0.049 * (ir as f64) / ((radii - 1) as f64);
            for it in 0..angles {
                let theta = std::f64::consts::TAU * (it as f64) / (angles as f64);
                let c = (-1.0 + r * theta.cos(), r * theta.sin());
                if !in_period_two_bulb(c) {
                    continue;
                }
                if (c.0 + 0.75).abs() < 1e-3 && c.1.abs() < 1e-3 {
                    continue;
                }
                out.push(c);
            }
        }
        out
    }

    #[test]
    fn entire_main_cardioid_is_exactly_period_one() {
        let samples = sample_main_cardioid(41);
        assert!(samples.len() > 100, "expected a dense cardioid sample, got {}", samples.len());
        assert_exact_periods("main cardioid", 1, &samples, 500_000);
    }

    #[test]
    fn entire_main_bulb_is_exactly_period_two() {
        let samples = sample_period_two_bulb(41);
        assert!(samples.len() > 100, "expected a dense period-2 bulb sample, got {}", samples.len());
        assert_exact_periods("period-2 bulb", 2, &samples, 500_000);
    }

    #[test]
    fn cardioid_and_bulb_boundary_rings_exact() {
        assert_exact_periods(
            "cardioid boundary ring"
            , 1
            , &sample_cardioid_boundary_ring(20, 72)
            , 500_000
        );
        assert_exact_periods(
            "period-2 bulb boundary ring"
            , 2
            , &sample_bulb_boundary_ring(20, 72)
            , 500_000
        );
    }

    #[test]
    fn cardioid_interior_periods_spatially_uniform() {
        let grid = 48;
        let mut periods = vec![None; grid * grid];
        for iy in 0..grid {
            for ix in 0..grid {
                let u = -0.98 + 1.96 * (ix as f64) / (grid as f64 - 1.0);
                let v = -0.98 + 1.96 * (iy as f64) / (grid as f64 - 1.0);
                if u * u + v * v >= 0.98 * 0.98 {
                    continue;
                }
                let c = cardioid_c_from_mu((u, v));
                periods[iy * grid + ix] = detect_period_for_c(c, 500_000);
            }
        }
        let mut failures = Vec::new();
        for iy in 1..grid - 1 {
            for ix in 1..grid - 1 {
                let here = periods[iy * grid + ix];
                if here != Some(1) {
                    continue;
                }
                for (dx, dy) in [(0isize, -1), (0, 1), (-1, 0), (1, 0)] {
                    let nx = (ix as isize + dx) as usize;
                    let ny = (iy as isize + dy) as usize;
                    match periods[ny * grid + nx] {
                        Some(1) | None => {}
                        Some(p) => failures.push(format!(
                            "cardioid neighbor period clash at mu-grid ({ix},{iy}): center=1 neighbor={p}"
                        )),
                    }
                }
            }
        }
        if !failures.is_empty() {
            panic!(
                "{} spatial period clashes (bands) in cardioid:\n{}",
                failures.len(),
                failures.iter().take(40).cloned().collect::<Vec<_>>().join("\n")
            );
        }
    }

    #[test]
    fn cardioid_and_bulb_nuclei_exact() {
        assert_exact_periods("cardioid nucleus", 1, &[(0.0, 0.0)], 50_000);
        assert_exact_periods("period-2 nucleus", 2, &[(-1.0, 0.0)], 50_000);
    }

    #[test]
    fn greater_magnification_local_samples_exact() {
        let mut samples = Vec::new();
        let cardioid_nucleus = (0.0, 0.0);
        let bulb_nucleus = (-1.0, 0.0);
        for &scale in &[1e-2, 1e-4, 1e-6, 1e-8] {
            for &(dx, dy) in &[
                (0.0, 0.0)
                , (0.3, 0.0)
                , (-0.3, 0.0)
                , (0.0, 0.3)
                , (0.0, -0.3)
                , (0.2, 0.2)
                , (-0.2, -0.2)
            ] {
                let c1 = (
                    cardioid_nucleus.0 + dx * scale
                    , cardioid_nucleus.1 + dy * scale
                );
                if c1.0 * c1.0 + c1.1 * c1.1 < 0.2 {
                    samples.push((1u64, c1));
                }
                let c2 = (
                    bulb_nucleus.0 + dx * scale * 0.2
                    , bulb_nucleus.1 + dy * scale * 0.2
                );
                if in_period_two_bulb(c2) {
                    samples.push((2u64, c2));
                }
            }
        }
        let mut failures = Vec::new();
        for (expected, c) in samples {
            match detect_period_for_c(c, 200_000) {
                Some(p) if p == expected => {}
                Some(p) => failures.push(format!(
                    "mag sample c=({:.17},{:.17}) expected {expected}, got {p}",
                    c.0, c.1
                )),
                None => failures.push(format!(
                    "mag sample c=({:.17},{:.17}) expected {expected}, got None",
                    c.0, c.1
                )),
            }
        }
        if !failures.is_empty() {
            panic!(
                "{} magnification-period failures:\n{}",
                failures.len(),
                failures.iter().take(40).cloned().collect::<Vec<_>>().join("\n")
            );
        }
    }

    #[test]
    fn exterior_point_rejects_period() {
        let period = detect_period_for_c((0.5, 0.5), 10_000);
        assert!(period.is_none(), "exterior should escape without period, got {period:?}");
    }

    #[test]
    fn near_parabolic_bulb_edge_is_exactly_period_two() {
        let samples = [
            (-1.21484375, 0.12777777777777777)
            , (-1.21484375, -0.12777777777777777)
            , (-0.78515625, 0.12777777777777777)
            , (-0.78515625, -0.12777777777777777)
            , (-1.0, 0.249)
            , (-1.0, -0.249)
        ];
        for c in samples {
            assert!(
                in_period_two_bulb(c)
                , "fixture must lie in period-2 bulb: {c:?}"
            );
        }
        assert_exact_periods("near-parabolic bulb edge", 2, &samples, 1_000_000);
    }

    fn in_main_cardioid(c: (f64, f64)) -> bool {
        let disc_re = 1.0 - 4.0 * c.0;
        let disc_im = -4.0 * c.1;
        let disc_abs = (disc_re * disc_re + disc_im * disc_im).sqrt();
        let root_re = ((disc_abs + disc_re) * 0.5).sqrt();
        let root_im = if disc_im >= 0.0 {
            ((disc_abs - disc_re) * 0.5).sqrt()
        } else {
            -((disc_abs - disc_re) * 0.5).sqrt()
        };
        for sign in [1.0, -1.0] {
            let z_re = 0.5 * (1.0 + sign * root_re);
            let z_im = 0.5 * (sign * root_im);
            if (2.0 * z_re).hypot(2.0 * z_im) < 1.0 {
                return true;
            }
        }
        false
    }

    #[test]
    fn membership_screen_cardioid_and_bulb_exact() {
        let width = 160;
        let height = 90;
        let mut failures = Vec::new();
        for iy in 0..height {
            for ix in 0..width {
                let x = -2.0 + 2.5 * ((ix as f64) + 0.5) / (width as f64);
                let y = 1.0 - 2.0 * ((iy as f64) + 0.5) / (height as f64);
                let c = (x, y);
                if in_main_cardioid(c) {
                    match detect_period_for_c(c, 500_000) {
                        Some(1) => {}
                        other => failures.push(format!(
                            "cardioid member c=({x:.17},{y:.17}) expected 1 got {other:?}"
                        )),
                    }
                } else if in_period_two_bulb(c)
                    && !((c.0 + 0.75).abs() < 1e-3 && c.1.abs() < 1e-3)
                {
                    match detect_period_for_c(c, 1_000_000) {
                        Some(2) => {}
                        other => failures.push(format!(
                            "bulb member c=({x:.17},{y:.17}) expected 2 got {other:?}"
                        )),
                    }
                }
            }
        }
        if !failures.is_empty() {
            panic!(
                "{} membership-screen period failures:\n{}",
                failures.len(),
                failures.iter().take(40).cloned().collect::<Vec<_>>().join("\n")
            );
        }
    }
}

#[cfg(test)]
mod twin_tests {
    use super::*;

    #[test]
    fn twin_confirm_accepts_matching_period_one_orbit() {
        // At c=0, z stays 0; checkpoint and live twin stay equal through N steps.
        let c = (0.0, 0.0);
        let z = (0.0, 0.0);
        let d = (1.0, 0.0);
        assert!(confirm_twins_off_to_side(c, z, d, z, 1e-12));
    }

    #[test]
    fn twin_confirm_rejects_divergent_checkpoints() {
        let c = (-0.5, 0.0);
        let z = (0.1, 0.0);
        let checkpoint = (0.9, 0.0);
        let d = (1.0, 0.0);
        assert!(!confirm_twins_off_to_side(c, z, d, checkpoint, 1e-12));
    }

    #[test]
    fn twin_confirm_rejects_nearby_but_diverging_orbit() {
        // Spatially close checkpoint that will diverge under iteration.
        let c = (-0.75, 0.1);
        let z = (0.2, 0.1);
        let d = (1.0, 0.0);
        let checkpoint = (0.21, 0.11);
        assert!(!confirm_twins_off_to_side(c, z, d, checkpoint, 1e-14));
    }

    #[test]
    fn detect_period_respects_twin_for_cardioid_nucleus() {
        let c = cardioid_c_from_mu((0.0, 0.0));
        assert_eq!(detect_period_for_c(c, 100_000), Some(1));
    }

    // D-PER-1: twin-test N is the named constant.
    #[test]
    fn twin_confirmation_uses_named_iteration_count() {
        assert_eq!(PERIOD_CONFIRMATION_ITERATIONS, 20);
    }
}
