struct Uniforms {
    bailout_radius_squared: f32,
    bout_iterations: u32,
    orbit_len: u32,
    point_count: u32,
    glitch_threshold: f32,
    confirm_iterations: u32,
    // Per-tile seat→screen mapping (CGenerator is view-lifetime at binding 3).
    tile_origin_x: u32,
    tile_origin_y: u32,
    tile_edge: u32,
    use_c_generator: u32,
    tile_count: u32,
    seats_per_tile: u32,
    // Packed (ox,oy) pairs: origins[i/2] holds two tiles. Max 128 tiles.
    tile_origins: array<vec4<u32>, 64>,
}

/// View-lifetime CGenerator (uploaded once per PointStencil).
struct CGenerator {
    origin_re: f32,
    origin_im: f32,
    space: f32,
    half: f32,
}

struct GpuPertPoint {
    dc_re: f32,
    dc_im: f32,
    dz_re: f32,
    dz_im: f32,
    d_re: f32,
    d_im: f32,
    iteration_count: u32,
    min_magnitude: f32,
    min_magnitude_time: u32,
    flags: u32,
    checkpoint_re: f32,
    checkpoint_im: f32,
    steps_since_checkpoint: u32,
    next_checkpoint_iteration: u32,
    detected_period: u32,
    epsilon: f32,
}

@group(0) @binding(0) var<uniform> uniforms: Uniforms;
@group(0) @binding(1) var<storage, read_write> points: array<GpuPertPoint>;
@group(0) @binding(2) var<storage, read> orbit: array<vec2<f32>>;
@group(0) @binding(3) var<uniform> cgen: CGenerator;

const FLAG_ACTIVE: u32 = 1u;
const FLAG_ESCAPED: u32 = 2u;
const FLAG_FINISHED: u32 = 4u;
const FLAG_GLITCH: u32 = 8u;
const FLAG_PERIODIC: u32 = 16u;
const MAX_PERIOD_SEARCH: u32 = 64u;

/// Dense identity seats: point index i == local seat (lx + ly * edge).
/// Fused micro-batch: i = tile * seats_per_tile + local.
fn dc_from_cgen(i: u32) -> vec2<f32> {
    let edge = max(uniforms.tile_edge, 1u);
    let spt = max(uniforms.seats_per_tile, edge * edge);
    var ox = uniforms.tile_origin_x;
    var oy = uniforms.tile_origin_y;
    var local = i;
    if uniforms.tile_count > 1u {
        let tile = i / spt;
        local = i % spt;
        let t = min(tile, 127u);
        let packed = uniforms.tile_origins[t / 2u];
        if (t % 2u) == 0u {
            ox = packed.x;
            oy = packed.y;
        } else {
            ox = packed.z;
            oy = packed.w;
        }
    }
    let lx = local % edge;
    let ly = local / edge;
    let sx = ox + lx;
    let sy = oy + ly;
    let dc_re = cgen.origin_re + cgen.space * f32(sx) + cgen.half;
    let dc_im = cgen.origin_im - cgen.space * f32(sy) - cgen.half;
    return vec2<f32>(dc_re, dc_im);
}

/// Init points from CGenerator (no host 4096 δc pack). Dense identity layout only.
@compute @workgroup_size(64)
fn init_from_cgen(@builtin(global_invocation_id) gid: vec3<u32>) {
    let i = gid.x;
    if i >= uniforms.point_count {
        return;
    }
    let dc = dc_from_cgen(i);
    var p: GpuPertPoint;
    p.dc_re = dc.x;
    p.dc_im = dc.y;
    p.dz_re = 0.0;
    p.dz_im = 0.0;
    p.d_re = 1.0;
    p.d_im = 0.0;
    p.iteration_count = 0u;
    p.min_magnitude = 3.402823466e+38;
    p.min_magnitude_time = 0u;
    p.flags = FLAG_ACTIVE;
    p.checkpoint_re = 0.0;
    p.checkpoint_im = 0.0;
    p.steps_since_checkpoint = 0u;
    p.next_checkpoint_iteration = 1u;
    p.detected_period = 0u;
    let c_abs = max(abs(dc.x), abs(dc.y));
    p.epsilon = max(1e-12, c_abs * 1e-6);
    points[i] = p;
}

fn orbit_at(n: u32) -> vec2<f32> {
    let len = uniforms.orbit_len;
    if len == 0u {
        return vec2<f32>(0.0, 0.0);
    }
    if len == 1u {
        return orbit[0];
    }
    // Period is treated as len-1 (build_reference_orbit stores one pre-period sample).
    let period = max(len - 1u, 1u);
    let loop_start = len - period;
    if n < loop_start {
        return orbit[n];
    }
    let offset = (n - loop_start) % period;
    return orbit[loop_start + offset];
}

fn near(a_re: f32, a_im: f32, b_re: f32, b_im: f32, epsilon: f32) -> bool {
    return abs(a_re - b_re) < epsilon && abs(a_im - b_im) < epsilon;
}

fn advance_full(
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
        advance_full(&live_z_re, &live_z_im, &live_d_re, &live_d_im, c_re, c_im);
        advance_full(&twin_z_re, &twin_z_im, &twin_d_re, &twin_d_im, c_re, c_im);
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
        advance_full(&z_re, &z_im, &d_re, &d_im, c_re, c_im);
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
    let i = gid.x;
    if i >= uniforms.point_count {
        return;
    }
    var p = points[i];
    if (p.flags & FLAG_ACTIVE) == 0u || (p.flags & FLAG_FINISHED) != 0u {
        return;
    }
    // Period search on the full orbit needs an absolute C. That is exact when
    // the reference is the zero orbit (orbit_len == 1); otherwise leave period
    // to the CPU detector (B-PER-2: never invent a period).
    let zero_orbit = uniforms.orbit_len <= 1u;
    let c_re = p.dc_re;
    let c_im = p.dc_im;
    for (var step = 0u; step < uniforms.bout_iterations; step = step + 1u) {
        if (p.flags & FLAG_FINISHED) != 0u {
            break;
        }
        // Glitch rebind: drop onto the zero orbit in-place instead of bouncing
        // the seat back to the CPU (tile_worker.md glitch rebind-to-zero).
        if (p.flags & FLAG_GLITCH) != 0u {
            p.dz_re = 0.0;
            p.dz_im = 0.0;
            p.flags = (p.flags | FLAG_ACTIVE) & (~FLAG_GLITCH) & (~FLAG_FINISHED);
        }
        let n = p.iteration_count;
        let z_ref = orbit_at(n);
        var dz = vec2<f32>(p.dz_re, p.dz_im);
        let z_full = z_ref + dz;
        let z_ref_mag2 = dot(z_ref, z_ref);
        let z_full_mag2 = dot(z_full, z_full);
        if z_ref_mag2 > 1e-30 && z_full_mag2 < uniforms.glitch_threshold * z_ref_mag2 {
            p.flags = p.flags | FLAG_GLITCH;
            // Stay active: next iteration of this bout rebinds to zero.
            continue;
        }
        var d = vec2<f32>(p.d_re, p.d_im);
        d = vec2<f32>(
            2.0 * (z_full.x * d.x - z_full.y * d.y),
            2.0 * (z_full.x * d.y + z_full.y * d.x)
        );
        let dz2 = vec2<f32>(dz.x * dz.x - dz.y * dz.y, 2.0 * dz.x * dz.y);
        dz = vec2<f32>(
            2.0 * (z_ref.x * dz.x - z_ref.y * dz.y) + dz2.x + p.dc_re,
            2.0 * (z_ref.x * dz.y + z_ref.y * dz.x) + dz2.y + p.dc_im
        );
        p.iteration_count = p.iteration_count + 1u;
        let z_ref_next = orbit_at(p.iteration_count);
        let z_full_next = z_ref_next + dz;
        p.dz_re = dz.x;
        p.dz_im = dz.y;
        p.d_re = d.x;
        p.d_im = d.y;
        let rad = dot(z_full_next, z_full_next);
        if rad < p.min_magnitude {
            p.min_magnitude = rad;
            p.min_magnitude_time = p.iteration_count;
        }
        if rad > uniforms.bailout_radius_squared {
            p.dz_re = z_full_next.x;
            p.dz_im = z_full_next.y;
            p.flags = p.flags | FLAG_ESCAPED | FLAG_FINISHED;
            break;
        }
        if zero_orbit && p.detected_period == 0u {
            let z_re = z_full_next.x;
            let z_im = z_full_next.y;
            p.steps_since_checkpoint = p.steps_since_checkpoint + 1u;
            if (p.steps_since_checkpoint > 0u
                && near(z_re, z_im, p.checkpoint_re, p.checkpoint_im, p.epsilon)
                && confirm_twins(
                    c_re, c_im, z_re, z_im, p.d_re, p.d_im
                    , p.checkpoint_re, p.checkpoint_im, p.epsilon
                )) {
                let period = minimal_period(
                    c_re, c_im, z_re, z_im
                    , p.steps_since_checkpoint
                    , p.epsilon
                );
                // Certain periods only: leave 0 when the search is inconclusive
                // so the host never paints a false Inside period (B-PER-2).
                if (period != 0u) {
                    p.detected_period = period;
                    p.flags = p.flags | FLAG_PERIODIC | FLAG_FINISHED;
                    break;
                }
            }
            if (p.iteration_count == p.next_checkpoint_iteration) {
                p.checkpoint_re = z_re;
                p.checkpoint_im = z_im;
                p.steps_since_checkpoint = 0u;
                p.next_checkpoint_iteration = max(p.next_checkpoint_iteration * 2u, 1u);
            }
        }
    }
    points[i] = p;
}
