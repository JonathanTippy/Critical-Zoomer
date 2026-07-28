//! Publisher compute: calibrated → answer with biased-nearest clamp and NORES.
//! docs/design/tile_publisher.md
// r[impl cz.int.publisher-nores-bias+1]
// r[impl cz.range.guess-biased-nearest+1]

const TILE_EDGE: u32 = 64u;
const KIND_AGNOSTIC: u32 = 0u;
const KIND_OUTSIDE: u32 = 1u;
const KIND_INSIDE: u32 = 2u;
const KIND_EMPTY: u32 = 3u;

struct GPUCalibratedAnswer {
    kind: u32,
    period_lo: u32,
    period_hi: u32,
    escape_lo: u32,
    escape_hi: u32,
    escape_z_re_lo: f32,
    escape_z_re_hi: f32,
    escape_z_im_lo: f32,
    escape_z_im_hi: f32,
    min_mag_time_lo: u32,
    min_mag_time_hi: u32,
    min_mag_lo: f32,
    min_mag_hi: f32,
    _pad0: u32,
    _pad1: u32,
    _pad2: u32,
}

struct GPUPackedAnswer {
    // 1 Outside, 2 Inside (NORES is Outside with escape=1, z=±inf, min_mag=inf)
    kind: u32,
    escape_or_period: u32,
    min_mag_time: u32,
    min_mag: f32,
    zx: f32,
    zy: f32,
    _pad0: u32,
    _pad1: u32,
}

@group(0) @binding(0) var<storage, read> calibrated: array<GPUCalibratedAnswer>;
@group(0) @binding(1) var<storage, read> bias: array<GPUPackedAnswer>;
@group(0) @binding(2) var<storage, read> bias_valid: array<u32>;
@group(0) @binding(3) var<storage, read_write> out_answers: array<GPUPackedAnswer>;

fn clamp_biased(lo: f32, hi: f32, b: f32) -> f32 {
    if b < lo {
        return lo;
    }
    if b > hi {
        return hi;
    }
    return b;
}

fn clamp_u32(lo: u32, hi: u32, b: u32) -> u32 {
    if b < lo {
        return lo;
    }
    if b > hi {
        return hi;
    }
    return b;
}

fn nores() -> GPUPackedAnswer {
    var a: GPUPackedAnswer;
    a.kind = KIND_OUTSIDE;
    a.escape_or_period = 1u;
    a.min_mag_time = 0u;
    a.min_mag = 1e30; // ∞ stand-in for shader; host maps to f64::INFINITY
    a.zx = -1e30;
    a.zy = 1e30;
    a._pad0 = 0u;
    a._pad1 = 0u;
    return a;
}

fn collapse_outside(c: GPUCalibratedAnswer, use_bias: bool, b: GPUPackedAnswer) -> GPUPackedAnswer {
    var a: GPUPackedAnswer;
    a.kind = KIND_OUTSIDE;
    if use_bias && b.kind == KIND_OUTSIDE {
        a.escape_or_period = clamp_u32(c.escape_lo, c.escape_hi, b.escape_or_period);
        a.zx = clamp_biased(c.escape_z_re_lo, c.escape_z_re_hi, b.zx);
        a.zy = clamp_biased(c.escape_z_im_lo, c.escape_z_im_hi, b.zy);
        a.min_mag_time = clamp_u32(c.min_mag_time_lo, c.min_mag_time_hi, b.min_mag_time);
        a.min_mag = clamp_biased(c.min_mag_lo, c.min_mag_hi, b.min_mag);
    } else {
        a.escape_or_period = c.escape_lo;
        a.zx = c.escape_z_re_lo;
        a.zy = c.escape_z_im_lo;
        a.min_mag_time = c.min_mag_time_lo;
        a.min_mag = c.min_mag_lo;
    }
    a._pad0 = 0u;
    a._pad1 = 0u;
    return a;
}

fn collapse_inside(c: GPUCalibratedAnswer, use_bias: bool, b: GPUPackedAnswer) -> GPUPackedAnswer {
    var a: GPUPackedAnswer;
    a.kind = KIND_INSIDE;
    if use_bias && b.kind == KIND_INSIDE {
        a.escape_or_period = clamp_u32(c.period_lo, c.period_hi, b.escape_or_period);
        a.min_mag_time = clamp_u32(c.min_mag_time_lo, c.min_mag_time_hi, b.min_mag_time);
        a.min_mag = clamp_biased(c.min_mag_lo, c.min_mag_hi, b.min_mag);
    } else {
        a.escape_or_period = c.period_lo;
        a.min_mag_time = c.min_mag_time_lo;
        a.min_mag = c.min_mag_lo;
    }
    a.zx = 0.0;
    a.zy = 0.0;
    a._pad0 = 0u;
    a._pad1 = 0u;
    return a;
}

@compute @workgroup_size(8, 8)
fn publish(@builtin(global_invocation_id) gid: vec3<u32>) {
    if gid.x >= TILE_EDGE || gid.y >= TILE_EDGE {
        return;
    }
    let idx = gid.y * TILE_EDGE + gid.x;
    let c = calibrated[idx];
    if c.kind == KIND_EMPTY {
        out_answers[idx] = nores();
        return;
    }
    let has_bias = bias_valid[idx] != 0u;
    let b = bias[idx];
    if c.kind == KIND_AGNOSTIC {
        if has_bias {
            // Biased guess toward proximate; prefer proximate kind when ranges allow.
            if b.kind == KIND_OUTSIDE {
                out_answers[idx] = collapse_outside(c, true, b);
            } else {
                out_answers[idx] = collapse_inside(c, true, b);
            }
        } else {
            out_answers[idx] = nores();
        }
        return;
    }
    if c.kind == KIND_OUTSIDE {
        out_answers[idx] = collapse_outside(c, has_bias, b);
        return;
    }
    // Inside
    out_answers[idx] = collapse_inside(c, has_bias, b);
}
