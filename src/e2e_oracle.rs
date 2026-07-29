//! Known-good Mandelbrot oracles for e2e (requirements E2E Addendum).
//! Prove with unit/property tests, then headed scripts compare live behavior against these facts.
//!
//! r[impl cz.e2e.visual-oracle+1]

use crate::assemblies::structs::*;
use crate::assemblies::workgroup::structs::mandelbrotable::*;
use crate::assemblies::workgroup::workcore::mandelbrot::ActivePoint;
use crate::assemblies::workgroup::workcore::mandelbrot::PeriodicityDetector;
use crate::assemblies::workgroup::workcore::mandelbrot::worker_implementations::periodicity_detector::CpuPeriodicityDetector;
use crate::assemblies::workgroup::workcore::mandelbrot::worker_implementations::perturbation_cpu_worker::{
    iterate_perturbation_bout, point_to_answer, PerturbationCpuWorkerState,
};
use crate::assemblies::workgroup::workcore::mandelbrot::ZERO_ORBIT_ID;
use crate::constants::{HOME_POSITION, PIXELS_PER_UNIT_POT};
use crate::intexp::IntExp;
use crate::range::*;

/// Escape-class fingerprint for oracle compares (stable; ignores float escape_z).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EscapeClass {
    Inside,
    Outside { escape_time_r2: u64 },
}

impl From<&Answer> for EscapeClass {
    fn from(a: &Answer) -> Self {
        match a.result {
            MandelbrotResult::Inside { .. } => EscapeClass::Inside,
            MandelbrotResult::Outside { escape_time_r2, .. } => EscapeClass::Outside { escape_time_r2 },
        }
    }
}

fn fresh_point(c: (f64, f64)) -> ActivePoint<f64, CpuPeriodicityDetector> {
    ActivePoint {
        c,
        z: (f64::ZERO, f64::ZERO),
        derivative: (f64::ONE, f64::ZERO),
        real_squared: f64::ZERO,
        imag_squared: f64::ZERO,
        real_imag: f64::ZERO,
        iteration_count: 0,
        min_magnitude: f64::MAX,
        min_magnitude_time: 0,
        periodicity_detector: CpuPeriodicityDetector::init(
            0,
            (f64::ZERO, f64::ZERO),
            (f64::ONE, f64::ZERO),
        ),
        escaped: false,
        finished: false,
        orbit_id: ZERO_ORBIT_ID,
        seat_linear: 0,
    }
}

/// Iterate a single c until finished (perturbation + zero-orbit — live one-path).
pub fn oracle_answer_at(c: (f64, f64), max_bouts: u32) -> Answer {
    let mut point = fresh_point(c);
    let mut state = PerturbationCpuWorkerState::default();
    state.seat_orbit_ids = vec![ZERO_ORBIT_ID];
    state.iterations_per_bout = 1000;
    let epsilon = 1e-12f64.max(c.0.abs().max(c.1.abs()) * 1e-6);
    for _ in 0..max_bouts {
        if point.finished {
            break;
        }
        iterate_perturbation_bout(&mut state, &mut point, epsilon);
    }
    point_to_answer(&point)
}

pub fn oracle_class_at(c: (f64, f64)) -> EscapeClass {
    EscapeClass::from(&oracle_answer_at(c, 64))
}

/// Home display stencil: UL (-2,+2) at HOME zoom pot (imag flip from objective).
pub fn home_stencil(res: (usize, usize)) -> PointStencil {
    let (_re, _im, pot) = HOME_POSITION;
    PointStencil {
        homothety: (IntExp::from(-2), IntExp::from(2), pot),
        resolution: res,
        serial_number: 0,
        focus: None,
        hover: None,
            mag_velocity: 0.0
    }
}

/// Home viewport sample seats → escape classes.
pub fn home_sample_classes(res: (usize, usize), stride: usize) -> Vec<((usize, usize), EscapeClass)> {
    let stencil = home_stencil(res);
    let gen = stencil
        .get_c_generator::<f64>()
        .expect("home stencil must yield c generator");
    let mut out = Vec::new();
    let step = stride.max(1);
    let mut y = 0usize;
    while y < res.1 {
        let mut x = 0usize;
        while x < res.0 {
            let c = gen.get_c((x as u16, y as u16));
            out.push(((x, y), oracle_class_at(c)));
            x += step;
        }
        y += step;
    }
    out
}

/// FNV-1a style fingerprint of home sample classes (headed scripts can print/compare).
pub fn home_oracle_fingerprint(res: (usize, usize), stride: usize) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for ((x, y), class) in home_sample_classes(res, stride) {
        h ^= x as u64;
        h = h.wrapping_mul(0x100000001b3);
        h ^= y as u64;
        h = h.wrapping_mul(0x100000001b3);
        let tag = match class {
            EscapeClass::Inside => 1u64,
            EscapeClass::Outside { escape_time_r2 } => 2u64 ^ (escape_time_r2 << 8),
        };
        h ^= tag;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// After one center 2× zoom, zoom_pot increases by 1 (natural zoom).
pub fn center_zoom_pot_delta(before: i32, after: i32) -> i32 {
    after - before
}

/// Pixel spacing at home (documentation check for PIXELS_PER_UNIT_POT + home pot).
pub fn home_pixel_space_exp() -> i32 {
    let pot = HOME_POSITION.2;
    -(pot + PIXELS_PER_UNIT_POT)
}

#[cfg(test)]
mod oracle_proving_tests {
    use super::*;
    use proptest::prelude::*;

    // r[verify cz.e2e.visual-oracle+1]

    #[test]
    fn c_zero_is_inside() {
        assert!(matches!(oracle_class_at((0.0, 0.0)), EscapeClass::Inside));
    }

    #[test]
    fn c_two_is_outside_fast() {
        match oracle_class_at((2.0, 0.0)) {
            EscapeClass::Outside { escape_time_r2 } => assert!(escape_time_r2 < 10),
            EscapeClass::Inside => panic!("c=2 must escape"),
        }
    }

    #[test]
    fn home_framing_samples_inside_and_outside() {
        // Cover ~default home framing (~6 units across at pot -2 ⇒ ≥768 seats).
        let classes = home_sample_classes((768, 460), 64);
        let inside = classes
            .iter()
            .filter(|(_, c)| matches!(c, EscapeClass::Inside))
            .count();
        let outside = classes
            .iter()
            .filter(|(_, c)| matches!(c, EscapeClass::Outside { .. }))
            .count();
        assert!(
            inside >= 2 && outside >= 2,
            "home framing needs Inside+Outside, got inside={inside} outside={outside} / {}",
            classes.len()
        );
    }

    #[test]
    fn home_fingerprint_stable() {
        let a = home_oracle_fingerprint((32, 32), 8);
        let b = home_oracle_fingerprint((32, 32), 8);
        assert_eq!(a, b);
        assert_ne!(a, 0);
    }

    #[test]
    fn conjugate_symmetry_property() {
        // r[verify cz.math.mandelbrot-real-axis-symmetry+1] (oracle path)
        proptest::proptest!(|(re in -1.5f64..0.5, im in 0.0f64..1.2)| {
            let a = oracle_class_at((re, im));
            let b = oracle_class_at((re, -im));
            prop_assert_eq!(a, b);
        });
    }

    #[test]
    fn exterior_far_is_outside() {
        assert!(matches!(
            oracle_class_at((10.0, 10.0)),
            EscapeClass::Outside { .. }
        ));
    }

    #[test]
    fn home_position_constant_is_neg2() {
        assert_eq!(HOME_POSITION, (-2, -2, -2));
    }

    #[test]
    fn center_zoom_pot_delta_is_one() {
        assert_eq!(center_zoom_pot_delta(-2, -1), 1);
    }

    #[test]
    fn home_stencil_has_c_generator() {
        assert!(home_stencil((64, 64)).get_c_generator::<f64>().is_some());
    }
}
