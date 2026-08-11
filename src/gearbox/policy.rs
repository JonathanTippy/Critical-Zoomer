//! View gear admission + PPS kernel selection policy.

use crate::assemblies::structs::KernelMode;
use crate::delta_gear::ComputeGear;

/// HUD / telemetry label for a compute gear.
#[inline]
pub fn hud_label(gear: ComputeGear) -> &'static str {
    gear.hud_label()
}

/// Default view gear from relative-admission (CGenerator), not a user setting.
///
/// When relative f64 grid is legal, prefer F64 host gear; otherwise FloatExp.
// r[impl cz.depth.compute-gear+1]
pub fn view_gear_from_relative_admission(relative_ok: bool) -> ComputeGear {
    if relative_ok {
        ComputeGear::F64
    } else {
        ComputeGear::FloatExp
    }
}

/// Shifts spent sampling each legal kernel before locking the PPS winner.
pub const PPS_PROBE_SHIFTS_PER_CANDIDATE: u8 = 3;

/// Re-run the PPS race this often — Naive GPU especially slows as fill progresses.
pub const PPS_REEVAL_INTERVAL: std::time::Duration = std::time::Duration::from_millis(100);

/// Legal production kernels for this view (honesty first, then PPS race).
///
/// Relative shells cannot run naive honestly → Perturbation only.
/// Absolute shells race Naive CPU, Naive GPU (if present), and Perturbation.
// r[impl cz.perf.pps-selected-kernel+1]
pub fn legal_kernels(coords_are_relative: bool, gpu_available: bool) -> Vec<KernelMode> {
    if coords_are_relative {
        return vec![KernelMode::Pert];
    }
    let mut out = vec![KernelMode::Naive];
    if gpu_available {
        out.push(KernelMode::NaiveGpu);
    }
    out.push(KernelMode::Pert);
    out
}

/// Pick the kernel with the highest measured PPS (`points / secs`).
/// Ties keep the earlier (legal-list order) candidate.
// r[impl cz.perf.pps-selected-kernel+1]
pub fn best_pps_kernel(samples: &[(KernelMode, f64)]) -> Option<KernelMode> {
    let mut best: Option<(KernelMode, f64)> = None;
    for &(mode, pps) in samples {
        match best {
            None => best = Some((mode, pps)),
            Some((_, best_pps)) if pps > best_pps => best = Some((mode, pps)),
            _ => {}
        }
    }
    best.map(|(m, _)| m)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relative_admission_prefers_f64() {
        assert_eq!(
            view_gear_from_relative_admission(true),
            ComputeGear::F64
        );
        assert_eq!(
            view_gear_from_relative_admission(false),
            ComputeGear::FloatExp
        );
    }

    #[test]
    fn legal_kernels_relative_is_pert_only() {
        assert_eq!(
            legal_kernels(true, true),
            vec![KernelMode::Pert]
        );
    }

    #[test]
    fn legal_kernels_absolute_races_all_when_gpu_up() {
        assert_eq!(
            legal_kernels(false, true),
            vec![KernelMode::Naive, KernelMode::NaiveGpu, KernelMode::Pert]
        );
        assert_eq!(
            legal_kernels(false, false),
            vec![KernelMode::Naive, KernelMode::Pert]
        );
    }

    #[test]
    fn best_pps_picks_highest_not_gpu_by_default() {
        let samples = [
            (KernelMode::Naive, 1.0e6),
            (KernelMode::NaiveGpu, 5.0e5),
            (KernelMode::Pert, 2.0e5),
        ];
        assert_eq!(best_pps_kernel(&samples), Some(KernelMode::Naive));
    }
}
