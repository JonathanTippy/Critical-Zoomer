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

/// Worker → display telemetry for HUD (stack, path, gear + rate counters).
// r[impl cz.depth.gear-hud+1]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct ViewHud {
    pub stack: HostStack,
    pub path: ComputePath,
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

/// Reference floor in use (interim until naive|pert kernel HUD).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ComputePath {
    #[default]
    Zero,
    Ref,
    Glitch,
}

impl ComputePath {
    pub fn hud_label(self) -> &'static str {
        match self {
            ComputePath::Zero => "zero",
            ComputePath::Ref => "ref",
            ComputePath::Glitch => "glitch",
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