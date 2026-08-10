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