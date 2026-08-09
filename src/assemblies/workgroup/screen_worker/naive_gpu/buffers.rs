//! Host↔GPU seat / finish packing for F32 and F64 pipelines.

use crate::assemblies::workgroup::screen_worker::workshift::Point;
use bytemuck::{Pod, Zeroable};

pub const FLAG_ACTIVE: u32 = 1;
pub const FLAG_ESCAPES: u32 = 2;
pub const FLAG_REPEATS: u32 = 4;

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct SeatF32 {
    pub c_x: f32,
    pub c_y: f32,
    pub z_x: f32,
    pub z_y: f32,
    pub dc_x: f32,
    pub dc_y: f32,
    pub real_squared: f32,
    pub imag_squared: f32,
    pub real_imag: f32,
    pub iterations: u32,
    pub loop_zx: f32,
    pub loop_zy: f32,
    pub loop_iter: u32,
    pub smallness: f32,
    pub small_time: u32,
    pub flags: u32,
    pub seat_index: u32,
    pub _pad: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct FinishF32 {
    pub seat_index: u32,
    pub flags: u32,
    pub iterations: u32,
    pub small_time: u32,
    pub smallness: f32,
    pub iter_delta: u32,
    pub z_x: f32,
    pub z_y: f32,
    pub dc_x: f32,
    pub dc_y: f32,
    pub c_x: f32,
    pub c_y: f32,
    pub loop_zx: f32,
    pub loop_zy: f32,
    pub loop_iter: u32,
    pub _pad: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct ParamsF32 {
    pub r_squared: f32,
    pub epsilon: f32,
    pub cap: u32,
    pub wip_count: u32,
    pub generation: u32,
    pub _p0: u32,
    pub _p1: u32,
    pub _p2: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct SeatF64 {
    pub c_x: f64,
    pub c_y: f64,
    pub z_x: f64,
    pub z_y: f64,
    pub dc_x: f64,
    pub dc_y: f64,
    pub real_squared: f64,
    pub imag_squared: f64,
    pub real_imag: f64,
    pub smallness: f64,
    pub iterations: u32,
    pub loop_iter: u32,
    pub loop_zx: f64,
    pub loop_zy: f64,
    pub small_time: u32,
    pub flags: u32,
    pub seat_index: u32,
    pub _pad: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct FinishF64 {
    pub seat_index: u32,
    pub flags: u32,
    pub iterations: u32,
    pub small_time: u32,
    pub smallness: f64,
    pub iter_delta: u32,
    pub loop_iter: u32,
    pub z_x: f64,
    pub z_y: f64,
    pub dc_x: f64,
    pub dc_y: f64,
    pub c_x: f64,
    pub c_y: f64,
    pub loop_zx: f64,
    pub loop_zy: f64,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct ParamsF64 {
    pub r_squared: f64,
    pub epsilon: f64,
    pub cap: u32,
    pub wip_count: u32,
    pub generation: u32,
    pub _p0: u32,
}

impl SeatF32 {
    pub fn from_point(index: u32, p: &Point<f64>) -> Self {
        let loop_z = p.loop_detection_point.0;
        Self {
            c_x: p.c.0 as f32,
            c_y: p.c.1 as f32,
            z_x: p.z.0 as f32,
            z_y: p.z.1 as f32,
            dc_x: p.dc.0 as f32,
            dc_y: p.dc.1 as f32,
            real_squared: p.real_squared as f32,
            imag_squared: p.imag_squared as f32,
            real_imag: p.real_imag as f32,
            iterations: p.iterations,
            loop_zx: loop_z.0 as f32,
            loop_zy: loop_z.1 as f32,
            loop_iter: p.loop_detection_point.1,
            smallness: p.smallness_squared as f32,
            small_time: p.small_time,
            flags: FLAG_ACTIVE,
            seat_index: index,
            _pad: 0,
        }
    }
}

impl SeatF64 {
    pub fn from_point(index: u32, p: &Point<f64>) -> Self {
        let loop_z = p.loop_detection_point.0;
        Self {
            c_x: p.c.0,
            c_y: p.c.1,
            z_x: p.z.0,
            z_y: p.z.1,
            dc_x: p.dc.0,
            dc_y: p.dc.1,
            real_squared: p.real_squared,
            imag_squared: p.imag_squared,
            real_imag: p.real_imag,
            smallness: p.smallness_squared,
            iterations: p.iterations,
            loop_iter: p.loop_detection_point.1,
            loop_zx: loop_z.0,
            loop_zy: loop_z.1,
            small_time: p.small_time,
            flags: FLAG_ACTIVE,
            seat_index: index,
            _pad: 0,
        }
    }
}

#[cfg(test)]
mod layout_tests {
    use super::*;
    use std::mem::size_of;

    #[test]
    fn f32_strides_match_device_constants() {
        assert_eq!(size_of::<SeatF32>(), 72);
        assert_eq!(size_of::<FinishF32>(), 64);
        assert_eq!(size_of::<ParamsF32>(), 32);
    }

    #[test]
    fn f64_strides_match_device_constants() {
        assert_eq!(size_of::<SeatF64>(), 120);
        assert_eq!(size_of::<FinishF64>(), 96);
        assert_eq!(size_of::<ParamsF64>(), 32);
    }
}
