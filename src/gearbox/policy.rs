//! View gear admission helpers (engine switching policy).

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
}
