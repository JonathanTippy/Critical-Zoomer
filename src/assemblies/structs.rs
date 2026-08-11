use crate::utils::IntExp;
use rug::Integer;
use crate::constants::*;
use crate::delta_gear::ComputeGear;
use std::cmp::*;
use std::time::Instant;


#[derive(PartialEq, Clone, Debug)]
pub struct PointStencil {
    pub location: (IntExp, IntExp, i32) // real, imag, magnification POT
    , pub resolution: (usize, usize)
    , pub serial_number: u64
}

/// Worker → display telemetry for HUD (stack, mode, ref, gear + rate counters).
// r[impl cz.depth.gear-hud+2]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct ViewHud {
    pub stack: HostStack,
    pub mode: KernelMode,
    pub reference: ReferenceStatus,
    pub gear: ComputeGear,
    pub points_delta: u64,
    pub iterations_delta: u64,
    /// Cumulative full-frame packages dropped by escaper/colorer drain-to-newest.
    pub packages_dropped: u64,
    /// Colorer path actually used for this view (OG / GPU / GPU fell back to OG).
    pub color: ColorerHud,
    /// Escaper path actually used for this view.
    pub escape: EscaperHud,
    /// Stage frame pulses for HUD RateCounters (usually 0 or 1 per message).
    pub publisher_frames_delta: u64,
    pub escape_frames_delta: u64,
    pub color_frames_delta: u64,
    pub controller_frames_delta: u64,
}

/// Manual colorer implementation (settings gear; default OG).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ColorerMode {
    #[default]
    Og,
    Gpu,
}

impl ColorerMode {
    pub fn manual_gear_label(self) -> &'static str {
        match self {
            ColorerMode::Og => "OG",
            ColorerMode::Gpu => "GPU",
        }
    }
}

/// Manual escaper implementation (settings gear; default OG).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum EscaperMode {
    #[default]
    Og,
    Gpu,
}

impl EscaperMode {
    pub fn manual_gear_label(self) -> &'static str {
        match self {
            EscaperMode::Og => "OG",
            EscaperMode::Gpu => "GPU",
        }
    }
}

/// What the colorer stamped on the last painted frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ColorerHud {
    #[default]
    Og,
    Gpu,
    /// Manual GPU selected but device unavailable — painted with OG.
    GpuFallbackOg,
}

impl ColorerHud {
    pub fn hud_label(self) -> &'static str {
        match self {
            ColorerHud::Og => "OG",
            ColorerHud::Gpu => "GPU",
            ColorerHud::GpuFallbackOg => "GPU→OG",
        }
    }
}

/// What the escaper stamped on the last values frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum EscaperHud {
    #[default]
    Og,
    Gpu,
    GpuFallbackOg,
}

impl EscaperHud {
    pub fn hud_label(self) -> &'static str {
        match self {
            EscaperHud::Og => "OG",
            EscaperHud::Gpu => "GPU",
            EscaperHud::GpuFallbackOg => "GPU→OG",
        }
    }
}

/// Host numeric stack for the view shell (`f64` vs FloatExp).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum HostStack {
    #[default]
    F64,
    FloatExp,
}

impl HostStack {
    pub fn hud_label(self) -> &'static str {
        match self {
            HostStack::F64 => "f64",
            HostStack::FloatExp => "FE",
        }
    }
}

/// Reference floor inside the single perturbation kernel (zero-orbit vs published ref).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum KernelMode {
    #[default]
    Naive,
    NaiveGpu,
    Pert,
}

impl KernelMode {
    pub fn hud_label(self) -> &'static str {
        match self {
            KernelMode::Naive => "naive",
            KernelMode::NaiveGpu => "naive-gpu",
            KernelMode::Pert => "pert",
        }
    }

    /// Debug radio labels for manual gear (entire compute kernel).
    pub fn manual_gear_label(self) -> &'static str {
        match self {
            KernelMode::Naive => "Naive",
            KernelMode::NaiveGpu => "Naive GPU",
            KernelMode::Pert => "Perturbation",
        }
    }
}

/// Running reference pipeline status for the HUD.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ReferenceStatus {
    #[default]
    Wip,
    Complete,
}

impl ReferenceStatus {
    pub fn hud_label(self) -> &'static str {
        match self {
            ReferenceStatus::Wip => "wip",
            ReferenceStatus::Complete => "complete",
        }
    }
}

impl ViewHud {
    /// Ref column is NA when naive mode has no perturbation reference floor.
    pub fn ref_hud_label(self) -> &'static str {
        if self.mode == KernelMode::Naive || self.mode == KernelMode::NaiveGpu {
            "NA"
        } else {
            self.reference.hud_label()
        }
    }
}

#[derive(PartialEq, Clone, Debug)]
pub struct View<T> {
    pub stencil: PointStencil
    , pub data: Vec<(T)>
    , pub bitmap: Vec<(u8)>
    // value,
    // 7: exact
    // , 6: representative / estimate from parent pixel
    // , 5: result is final/done/complete
    , pub hud: ViewHud
}




pub const EXACT: u8 = 0b1000_0000;
pub const PROX: u8 = 0b0100_0000;
pub const DONE: u8 = 0b0010_0000;

#[derive(Copy, Clone)]

pub struct Answer {
    pub result: MandelbrotResult
    , pub min_magnitude_time: u64
    , pub min_magnitude: f64
}

impl Answer {
    pub const TESTVAL: Answer = Answer {
        result: MandelbrotResult::Outside {
            escape_time_r2: 0
            , escape_z: (0.0, 0.0)
            , escape_dc: (1.0, 0.0)
        }
        , min_magnitude_time: 0
        , min_magnitude: 0.0
    };
}

#[derive(Copy, Clone)]
pub enum MandelbrotResult {
    Outside {
        escape_time_r2: u64
        , escape_z: (f32, f32)
        , escape_dc: (f32, f32)
    }
    , Inside {
        period: u64
    }
}

#[cfg(test)]
mod mutant_kill {
    use super::*;

    /// Thought-killed pins for HUD labels / ref NA / bitmap flag bits.
    #[test]
    fn mutant_kill_structs_hud_and_flags() {
        assert_eq!(HostStack::F64.hud_label(), "f64");
        assert_eq!(HostStack::FloatExp.hud_label(), "FE");
        assert_ne!(HostStack::F64.hud_label(), HostStack::FloatExp.hud_label());
        assert_ne!(HostStack::F64.hud_label(), "");

        assert_eq!(KernelMode::Naive.hud_label(), "naive");
        assert_eq!(KernelMode::NaiveGpu.hud_label(), "naive-gpu");
        assert_eq!(KernelMode::Pert.hud_label(), "pert");
        assert_eq!(KernelMode::Naive.manual_gear_label(), "Naive");
        assert_eq!(KernelMode::NaiveGpu.manual_gear_label(), "Naive GPU");
        assert_eq!(KernelMode::Pert.manual_gear_label(), "Perturbation");
        assert_ne!(KernelMode::Naive.hud_label(), KernelMode::Pert.hud_label());

        assert_eq!(ReferenceStatus::Wip.hud_label(), "wip");
        assert_eq!(ReferenceStatus::Complete.hud_label(), "complete");

        let naive = ViewHud {
            mode: KernelMode::Naive,
            reference: ReferenceStatus::Complete,
            ..Default::default()
        };
        assert_eq!(naive.ref_hud_label(), "NA");
        let gpu = ViewHud {
            mode: KernelMode::NaiveGpu,
            reference: ReferenceStatus::Wip,
            ..Default::default()
        };
        assert_eq!(gpu.ref_hud_label(), "NA");
        let pert = ViewHud {
            mode: KernelMode::Pert,
            reference: ReferenceStatus::Complete,
            ..Default::default()
        };
        assert_eq!(pert.ref_hud_label(), "complete");
        let pert_wip = ViewHud {
            mode: KernelMode::Pert,
            reference: ReferenceStatus::Wip,
            ..Default::default()
        };
        assert_eq!(pert_wip.ref_hud_label(), "wip");
        // ||→&& on Naive|NaiveGpu would leave NaiveGpu showing "wip".
        assert_ne!(gpu.ref_hud_label(), "wip");

        assert_eq!(EXACT, 0b1000_0000);
        assert_eq!(PROX, 0b0100_0000);
        assert_eq!(DONE, 0b0010_0000);
        assert_ne!(EXACT, PROX);
        assert_ne!(EXACT | PROX, DONE);
        assert_eq!(EXACT | PROX | DONE, 0b1110_0000);
    }
}