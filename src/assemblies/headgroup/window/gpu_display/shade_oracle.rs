//! Known-good cpu mirror of the shading shader, for tests only.
//!
//! This is what used to be the shadergroup: the escaper and the colorer, cleaned up and
//! reduced to the pure math, so it can act as the oracle the wgsl is measured against.
//! It carries no state, talks to no channels, and is never linked into the app.
//!
//! Everything here is written in f32 in the same order as `shade.wgsl` so the two agree
//! to within the slack a gpu is allowed on transcendentals.

use std::f32::consts::E;
use std::f32::consts::TAU;

use super::GpuInstruction;
use super::ShadeUniforms;

pub const KIND_MISSING: f32 = 0.0;
pub const KIND_OUTSIDE: f32 = 1.0;
pub const KIND_INSIDE: f32 = 2.0;

pub const OP_ESCAPE_TIME: u32 = 0;
pub const OP_SMALL_TIME: u32 = 1;
pub const OP_SMALLNESS: u32 = 2;
pub const OP_IN_FILAMENT: u32 = 3;
pub const OP_OUT_FILAMENT: u32 = 4;
pub const OP_NODES: u32 = 5;
pub const OP_STE: u32 = 6;

pub const SHADE_MODULAR: u32 = 0;
pub const SHADE_SINUS: u32 = 1;

pub const NORM_NONE: u32 = 0;
pub const NORM_LN: u32 = 1;
pub const NORM_LNLN: u32 = 2;
pub const NORM_RECIP: u32 = 3;
pub const NORM_RECIP_LN: u32 = 4;

/// An answer as the sampling shader hands it over: still at r=2, not yet bailed out.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RawAnswer {
    pub kind: f32
    , pub escape_or_period: f32
    , pub small_time: f32
    , pub smallness: f32
    , pub zx: f32
    , pub zy: f32
}

impl RawAnswer {
    /// No tile covers this seat.
    pub fn missing() -> Self {
        Self {
            kind: KIND_MISSING
            , escape_or_period: 0.0
            , small_time: 0.0
            , smallness: 0.0
            , zx: 0.0
            , zy: 0.0
        }
    }

    pub fn outside(escape_time: f32, small_time: f32, smallness: f32, z: (f32, f32)) -> Self {
        Self {
            kind: KIND_OUTSIDE
            , escape_or_period: escape_time
            , small_time
            , smallness
            , zx: z.0
            , zy: z.1
        }
    }

    pub fn inside(period: f32, small_time: f32, smallness: f32) -> Self {
        Self {
            kind: KIND_INSIDE
            , escape_or_period: period
            , small_time
            , smallness
            , zx: 0.0
            , zy: 0.0
        }
    }
}

/// An answer after the escape phase.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Finished {
    pub kind: f32
    , pub big_time: f32
    , pub small_time: f32
    , pub smallness: f32
    , pub loop_period: f32
}

impl Finished {
    pub fn is_outside(&self) -> bool {
        self.kind > 0.5 && self.kind < 1.5
    }

    pub fn is_inside(&self) -> bool {
        self.kind > 1.5
    }
}

/// The patch of seats a tile covers. Anything outside it reads as missing, which is what
/// the sampling shader does when no tile owns the seat.
#[derive(Clone, Debug)]
pub struct RawGrid {
    pub size: (i32, i32)
    , cells: Vec<RawAnswer>
}

impl RawGrid {
    pub fn new(size: (i32, i32)) -> Self {
        Self {
            size
            , cells: vec![RawAnswer::missing(); (size.0 * size.1) as usize]
        }
    }

    pub fn set(&mut self, seat: (i32, i32), raw: RawAnswer) {
        if let Some(index) = self.index(seat) {
            self.cells[index] = raw;
        }
    }

    pub fn fill(&mut self, raw: RawAnswer) {
        for cell in self.cells.iter_mut() {
            *cell = raw;
        }
    }

    pub fn get(&self, seat: (i32, i32)) -> RawAnswer {
        match self.index(seat) {
            Some(index) => self.cells[index]
            , None => RawAnswer::missing()
        }
    }

    pub fn cells(&self) -> &[RawAnswer] {
        &self.cells
    }

    fn index(&self, seat: (i32, i32)) -> Option<usize> {
        if seat.0 < 0 || seat.1 < 0 || seat.0 >= self.size.0 || seat.1 >= self.size.1 {
            return None;
        }
        Some((seat.1 * self.size.0 + seat.0) as usize)
    }
}

pub fn normalize_value(n: f32, method: u32) -> f32 {
    match method {
        NORM_LN => n.max(1.0).ln()
        , NORM_LNLN => n.max(E).ln().ln()
        , NORM_RECIP => 1.0 / n.max(1.0e-6)
        , NORM_RECIP_LN => 1.0 / n.max(1.0).ln().max(1.0e-6)
        , _ => n
    }
}

pub fn shade_value(n: f32, period: f32, phase: f32, shading: u32) -> f32 {
    let period_recip = 1.0 / period.max(1.0e-6);
    if shading == SHADE_SINUS {
        return (1.0 - ((n + phase) * TAU * period_recip).cos()) * 0.5;
    }
    let scaled = (n + phase) * period_recip;
    scaled - scaled.floor()
}

/// Shift every channel by the same amount, capped so the shift can neither blow out the
/// brightest channel nor crush the darkest one.
pub fn modify_color(color: [f32; 3], brightness: f32, range: f32) -> [f32; 3] {
    let mut delta = ((brightness * 255.0) - 127.0) * range;
    let cmax = color[0].max(color[1]).max(color[2]);
    let cmin = color[0].min(color[1]).min(color[2]);
    if cmin + delta < 0.0 {
        delta = -cmin;
    }
    if cmax + delta > 255.0 {
        delta = 255.0 - cmax;
    }
    [
        (color[0] + delta).clamp(0.0, 255.0)
        , (color[1] + delta).clamp(0.0, 255.0)
        , (color[2] + delta).clamp(0.0, 255.0)
    ]
}

pub fn layer_colors(bottom: [f32; 3], top: [f32; 3], opacity: f32) -> [f32; 3] {
    let top_share = opacity;
    let bottom_share = 255.0 - top_share;
    [
        ((bottom[0] * bottom_share + top[0] * top_share) / 256.0).floor()
        , ((bottom[1] * bottom_share + top[1] * top_share) / 256.0).floor()
        , ((bottom[2] * bottom_share + top[2] * top_share) / 256.0).floor()
    ]
}

/// Escape phase: carry an outside answer on from r=2 to the settings bailout radius.
pub fn bailout_escape(raw: RawAnswer, seat: (i32, i32), uniforms: &ShadeUniforms) -> Finished {
    if raw.kind < 0.5 {
        return Finished {
            kind: KIND_OUTSIDE
            , big_time: 1.0
            , small_time: 0.0
            , smallness: 1.0e30
            , loop_period: 0.0
        };
    }
    if raw.kind > 1.5 {
        return Finished {
            kind: KIND_INSIDE
            , big_time: 0.0
            , small_time: raw.small_time
            , smallness: raw.smallness
            , loop_period: raw.escape_or_period
        };
    }
    let c = (
        uniforms.origin_re + seat.0 as f32 * uniforms.space
        , uniforms.origin_im - seat.1 as f32 * uniforms.space
    );
    let mut z = (raw.zx, raw.zy);
    let mut iterations = raw.escape_or_period;
    let r2 = uniforms.bailout_radius * uniforms.bailout_radius;
    let mut extra = 0u32;
    loop {
        let mag2 = z.0 * z.0 + z.1 * z.1;
        if mag2 > r2 {
            break;
        }
        if extra >= uniforms.bailout_max_extra {
            break;
        }
        z = (z.0 * z.0 - z.1 * z.1 + c.0, 2.0 * z.0 * z.1 + c.1);
        iterations += 1.0;
        extra += 1;
    }
    Finished {
        kind: KIND_OUTSIDE
        , big_time: iterations
        , small_time: raw.small_time
        , smallness: raw.smallness
        , loop_period: 0.0
    }
}

pub fn finished_at(grid: &RawGrid, uniforms: &ShadeUniforms, seat: (i32, i32)) -> Finished {
    bailout_escape(grid.get(seat), seat, uniforms)
}

/// A negative reading means "this seat has nothing to say about this comparison".
pub fn opt_escape_time(f: Finished) -> f32 {
    if f.is_outside() {
        return f.big_time;
    }
    -1.0
}

pub fn opt_period(f: Finished) -> f32 {
    if f.is_inside() && f.loop_period > 0.5 {
        return f.loop_period;
    }
    -1.0
}

pub fn opt_smallness(f: Finished) -> f32 {
    if f.kind < 0.5 {
        return -1.0;
    }
    f.smallness
}

/// Small time is read before the escape phase, which cannot change it, and a zero small
/// time is not a ridge so it never takes part.
pub fn raw_small_time(grid: &RawGrid, seat: (i32, i32)) -> f32 {
    let raw = grid.get(seat);
    if raw.kind < 0.5 {
        return -1.0;
    }
    if raw.small_time < 0.5 {
        return -1.0;
    }
    raw.small_time
}

pub fn is_increased(v: f32, up: f32, down: f32, left: f32, right: f32) -> bool {
    if v < 0.0 {
        return false;
    }
    (up >= 0.0 && up < v)
        || (down >= 0.0 && down < v)
        || (left >= 0.0 && left < v)
        || (right >= 0.0 && right < v)
}

pub fn slope_sign_changed(v: f32, up: f32, down: f32, left: f32, right: f32) -> bool {
    // D-SHADE-1 / A-SHADE-INFIL: discrete hard inversion (π/2-class) of escape-time slope.
    if v < 0.0 {
        return false;
    }
    if up >= 0.0 && down >= 0.0 && down < v && v > up {
        return true;
    }
    if left >= 0.0 && right >= 0.0 && left < v && v > right {
        return true;
    }
    false
}

pub fn is_local_minimum(v: f32, up: f32, down: f32, left: f32, right: f32) -> bool {
    // D-SHADE-2 / A-SHADE-NODE: local smallness minimum (node seed).
    v >= 0.0
        && up >= 0.0
        && down >= 0.0
        && left >= 0.0
        && right >= 0.0
        && down > v
        && v < up
        && left > v
        && v < right
}

fn neighborhood<T: Copy>(seat: (i32, i32), step: i32, read: impl Fn((i32, i32)) -> T) -> [T; 5] {
    [
        read(seat)
        , read((seat.0, seat.1 - step))
        , read((seat.0, seat.1 + step))
        , read((seat.0 - step, seat.1))
        , read((seat.0 + step, seat.1))
    ]
}

pub fn is_in_filament(grid: &RawGrid, uniforms: &ShadeUniforms, seat: (i32, i32)) -> bool {
    let v = neighborhood(seat, 1, |s| opt_escape_time(finished_at(grid, uniforms, s)));
    slope_sign_changed(v[0], v[1], v[2], v[3], v[4])
}

pub fn is_out_filament(grid: &RawGrid, uniforms: &ShadeUniforms, seat: (i32, i32)) -> bool {
    let v = neighborhood(seat, 1, |s| opt_period(finished_at(grid, uniforms, s)));
    is_increased(v[0], v[1], v[2], v[3], v[4])
}

pub fn is_node(
    grid: &RawGrid
    , uniforms: &ShadeUniforms
    , seat: (i32, i32)
    , thickness: i32
) -> bool {
    let v = neighborhood(seat, thickness, |s| opt_smallness(finished_at(grid, uniforms, s)));
    is_local_minimum(v[0], v[1], v[2], v[3], v[4])
}

pub fn is_ste(grid: &RawGrid, seat: (i32, i32)) -> bool {
    let v = neighborhood(seat, 1, |s| raw_small_time(grid, s));
    is_increased(v[0], v[1], v[2], v[3], v[4])
}

/// The colour the shader should produce for one seat.
pub fn shade_seat(
    uniforms: &ShadeUniforms
    , instructions: &[GpuInstruction]
    , grid: &RawGrid
    , seat: (i32, i32)
) -> [u8; 3] {
    if uniforms.zoom_match == 0 {
        return quantize([0.05, 0.05, 0.08]);
    }
    let finished = finished_at(grid, uniforms, seat);
    let mut rgb = [0.0f32; 3];
    // Oracle paints from the instruction slice it was given. The GPU uniform's
    // instruction_count is only authoritative for the shader upload path.
    let count = instructions.len();
    for inst in &instructions[..count] {
        let base = [inst.color_r, inst.color_g, inst.color_b];
        let side_opacity = if finished.is_inside() {
            inst.opacity_inside
        } else {
            inst.opacity_outside
        };
        match inst.opcode {
            OP_ESCAPE_TIME => {
                if finished.is_outside() && inst.opacity_outside > 0.0 {
                    let n = normalize_value(finished.big_time, inst.normalizing);
                    rgb = paint(rgb, base, n, inst, inst.opacity_outside);
                }
            }
            OP_SMALL_TIME => {
                if side_opacity > 0.0 {
                    let n = normalize_value(finished.small_time, inst.normalizing);
                    rgb = paint(rgb, base, n, inst, side_opacity);
                }
            }
            OP_SMALLNESS => {
                if side_opacity > 0.0 {
                    let n = normalize_value(finished.smallness, inst.normalizing);
                    rgb = paint(rgb, base, n, inst, side_opacity);
                }
            }
            OP_IN_FILAMENT => {
                if finished.is_outside()
                    && side_opacity > 0.0
                    && is_in_filament(grid, uniforms, seat)
                {
                    rgb = layer_colors(rgb, base, side_opacity);
                }
            }
            OP_OUT_FILAMENT => {
                if finished.is_inside()
                    && side_opacity > 0.0
                    && is_out_filament(grid, uniforms, seat)
                {
                    rgb = layer_colors(rgb, base, side_opacity);
                }
            }
            OP_NODES => {
                if side_opacity > 0.0
                    && is_node(grid, uniforms, seat, inst.thickness as i32)
                {
                    rgb = layer_colors(rgb, base, side_opacity);
                }
            }
            OP_STE => {
                if side_opacity > 0.0 && is_ste(grid, seat) {
                    rgb = layer_colors(rgb, base, side_opacity);
                }
            }
            _ => {}
        }
    }
    quantize([rgb[0] / 255.0, rgb[1] / 255.0, rgb[2] / 255.0])
}

fn paint(
    rgb: [f32; 3]
    , base: [f32; 3]
    , n: f32
    , inst: &GpuInstruction
    , opacity: f32
) -> [f32; 3] {
    let brightness = shade_value(n, inst.period, inst.phase, inst.shading);
    let top = modify_color(base, brightness, inst.range);
    layer_colors(rgb, top, opacity)
}

/// The whole viewport, row major, as the render target would hold it.
pub fn shade_frame(
    uniforms: &ShadeUniforms
    , instructions: &[GpuInstruction]
    , grid: &RawGrid
) -> Vec<[u8; 3]> {
    let width = uniforms.viewport_size[0] as i32;
    let height = uniforms.viewport_size[1] as i32;
    let mut out = Vec::with_capacity((width.max(0) * height.max(0)) as usize);
    for y in 0..height {
        for x in 0..width {
            out.push(shade_seat(uniforms, instructions, grid, (x, y)));
        }
    }
    out
}

fn quantize(rgb: [f32; 3]) -> [u8; 3] {
    [
        (rgb[0].clamp(0.0, 1.0) * 255.0).round() as u8
        , (rgb[1].clamp(0.0, 1.0) * 255.0).round() as u8
        , (rgb[2].clamp(0.0, 1.0) * 255.0).round() as u8
    ]
}
