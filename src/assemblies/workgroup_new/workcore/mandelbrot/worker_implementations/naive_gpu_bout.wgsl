struct Uniforms {
    bailout_radius_squared: f32,
    bout_iterations: u32,
    confirm_iterations: u32,
    point_count: u32,
}

struct GpuPoint {
    c_re: f32,
    c_im: f32,
    z_re: f32,
    z_im: f32,
    d_re: f32,
    d_im: f32,
    checkpoint_re: f32,
    checkpoint_im: f32,
    min_magnitude: f32,
    epsilon: f32,
    iteration_count: u32,
    min_magnitude_time: u32,
    steps_since_checkpoint: u32,
    next_checkpoint_iteration: u32,
    detected_period: u32,
    flags: u32,
    local_x: u32,
    local_y: u32,
}

@group(0) @binding(0) var<uniform> uniforms: Uniforms;
@group(0) @binding(1) var<storage, read_write> points: array<GpuPoint>;

const FLAG_ACTIVE: u32 = 1u;
const FLAG_ESCAPED: u32 = 2u;
const FLAG_FINISHED: u32 = 4u;
const MAX_PERIOD_SEARCH: u32 = 4096u;

fn near(ax: f32, ay: f32, bx: f32, by: f32, epsilon: f32) -> bool {
    return abs(ax - bx) < epsilon && abs(ay - by) < epsilon;
}

fn advance(
    z_re: ptr<function, f32>
    , z_im: ptr<function, f32>
    , d_re: ptr<function, f32>
    , d_im: ptr<function, f32>
    , c_re: f32
    , c_im: f32
) {
    let old_re = *z_re;
    let old_im = *z_im;
    let old_d_re = *d_re;
    let old_d_im = *d_im;
    *d_re = 2.0 * (old_re * old_d_re - old_im * old_d_im);
    *d_im = 2.0 * (old_re * old_d_im + old_im * old_d_re);
    *z_re = old_re * old_re - old_im * old_im + c_re;
    *z_im = 2.0 * old_re * old_im + c_im;
}

fn confirm_twins(
    c_re: f32
    , c_im: f32
    , z_re: f32
    , z_im: f32
    , d_re: f32
    , d_im: f32
    , checkpoint_re: f32
    , checkpoint_im: f32
    , epsilon: f32
) -> bool {
    var live_z_re = z_re;
    var live_z_im = z_im;
    var live_d_re = d_re;
    var live_d_im = d_im;
    var twin_z_re = checkpoint_re;
    var twin_z_im = checkpoint_im;
    var twin_d_re = d_re;
    var twin_d_im = d_im;
    for (var i = 0u; i < uniforms.confirm_iterations; i = i + 1u) {
        advance(&live_z_re, &live_z_im, &live_d_re, &live_d_im, c_re, c_im);
        advance(&twin_z_re, &twin_z_im, &twin_d_re, &twin_d_im, c_re, c_im);
        if (!near(live_z_re, live_z_im, twin_z_re, twin_z_im, epsilon)
            || !near(live_d_re, live_d_im, twin_d_re, twin_d_im, epsilon)) {
            return false;
        }
    }
    return true;
}

fn closes_after(
    c_re: f32
    , c_im: f32
    , z0_re: f32
    , z0_im: f32
    , period: u32
    , epsilon: f32
) -> bool {
    var z_re = z0_re;
    var z_im = z0_im;
    var d_re = 1.0;
    var d_im = 0.0;
    for (var i = 0u; i < period; i = i + 1u) {
        advance(&z_re, &z_im, &d_re, &d_im, c_re, c_im);
    }
    let reduce = max(epsilon, 1.0e-3);
    return abs(z_re - z0_re) < reduce && abs(z_im - z0_im) < reduce;
}

fn minimal_period(
    c_re: f32
    , c_im: f32
    , z_re: f32
    , z_im: f32
    , candidate: u32
    , epsilon: f32
) -> u32 {
    if (candidate == 0u) {
        return 0u;
    }
    if (candidate == 1u) {
        return 1u;
    }
    let limit = min(candidate, MAX_PERIOD_SEARCH);
    for (var period = 1u; period <= limit; period = period + 1u) {
        if (closes_after(c_re, c_im, z_re, z_im, period, epsilon)) {
            return period;
        }
    }
    return 0u;
}

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let idx = gid.x;
    if (idx >= uniforms.point_count) {
        return;
    }
    var p = points[idx];
    if ((p.flags & FLAG_ACTIVE) == 0u || (p.flags & FLAG_FINISHED) != 0u) {
        return;
    }
    var z_re = p.z_re;
    var z_im = p.z_im;
    var d_re = p.d_re;
    var d_im = p.d_im;
    let c_re = p.c_re;
    let c_im = p.c_im;
    let bailout = uniforms.bailout_radius_squared;
    for (var step = 0u; step < uniforms.bout_iterations; step = step + 1u) {
        if ((p.flags & FLAG_FINISHED) != 0u) {
            break;
        }
        advance(&z_re, &z_im, &d_re, &d_im, c_re, c_im);
        p.iteration_count = p.iteration_count + 1u;
        let rad = z_re * z_re + z_im * z_im;
        if (rad < p.min_magnitude) {
            p.min_magnitude = rad;
            p.min_magnitude_time = p.iteration_count;
        }
        if (rad > bailout) {
            p.z_re = z_re;
            p.z_im = z_im;
            p.d_re = d_re;
            p.d_im = d_im;
            p.flags = p.flags | FLAG_ESCAPED | FLAG_FINISHED;
            break;
        }
        if (p.detected_period == 0u) {
            p.steps_since_checkpoint = p.steps_since_checkpoint + 1u;
            if (p.steps_since_checkpoint > 0u
                && near(z_re, z_im, p.checkpoint_re, p.checkpoint_im, p.epsilon)
                && confirm_twins(
                    c_re
                    , c_im
                    , z_re
                    , z_im
                    , d_re
                    , d_im
                    , p.checkpoint_re
                    , p.checkpoint_im
                    , p.epsilon
                )) {
                let period = minimal_period(
                    c_re
                    , c_im
                    , z_re
                    , z_im
                    , p.steps_since_checkpoint
                    , p.epsilon
                );
                if (period != 0u) {
                    p.detected_period = period;
                    p.z_re = z_re;
                    p.z_im = z_im;
                    p.d_re = d_re;
                    p.d_im = d_im;
                    p.flags = p.flags | FLAG_FINISHED;
                    break;
                }
            }
            if (p.iteration_count == p.next_checkpoint_iteration) {
                p.checkpoint_re = z_re;
                p.checkpoint_im = z_im;
                p.steps_since_checkpoint = 0u;
                p.next_checkpoint_iteration = p.next_checkpoint_iteration * 2u;
            }
        }
    }
    if ((p.flags & FLAG_FINISHED) == 0u) {
        p.z_re = z_re;
        p.z_im = z_im;
        p.d_re = d_re;
        p.d_im = d_im;
    }
    points[idx] = p;
}
