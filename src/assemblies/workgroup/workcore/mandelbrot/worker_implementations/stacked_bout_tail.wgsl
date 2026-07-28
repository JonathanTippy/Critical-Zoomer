// Stacked-i32 bout tail (appended by gears::stacked_bout_wgsl).
// Same bind layout as perturbation_gpu_bout.wgsl; zero-orbit seats iterate in Stacked.

struct Uniforms {
    bailout_radius_squared: f32,
    bout_iterations: u32,
    orbit_len: u32,
    point_count: u32,
    glitch_threshold: f32,
    confirm_iterations: u32,
    _pad0: f32,
    _pad1: f32,
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

const FLAG_ACTIVE: u32 = 1u;
const FLAG_ESCAPED: u32 = 2u;
const FLAG_FINISHED: u32 = 4u;
const FLAG_GLITCH: u32 = 8u;
const FLAG_PERIODIC: u32 = 16u;

fn sie_from_f32(v: f32) -> Stacked {
    // Scale into fixed significand: treat value as v * 2^0 with coarse limb fill.
    if (abs(v) < 1e-30) {
        return sie_zero();
    }
    var bits = bitcast<u32>(v);
    let sign = (bits >> 31u) != 0u;
    var exp_bits = i32((bits >> 23u) & 0xffu) - 127;
    var mant = (bits & 0x7fffffu) | 0x800000u;
    // Place 24-bit mantissa in high limb region.
    var out = sie_zero();
    out.limbs[0] = i32(mant);
    out.exp = exp_bits - 23;
    if (sign) {
        out = sie_neg(out);
    }
    return out;
}

fn sie_to_f32(a: Stacked) -> f32 {
    if (sie_is_zero(a)) {
        return 0.0;
    }
    var x = sie_abs(a);
    var v = f32(x.limbs[0]);
    // Fold remaining limbs at 2^(32*k).
    for (var i = 1u; i < LIMBS; i = i + 1u) {
        v = v + f32(x.limbs[i]) * exp2(32.0 * f32(i));
    }
    v = v * exp2(f32(x.exp));
    if (sie_is_neg(a)) {
        v = -v;
    }
    return v;
}

fn orbit_at(n: u32) -> vec2<f32> {
    let len = uniforms.orbit_len;
    if len == 0u {
        return vec2<f32>(0.0, 0.0);
    }
    if len == 1u {
        return orbit[0];
    }
    let period = max(len - 1u, 1u);
    let loop_start = len - period;
    if n < loop_start {
        return orbit[n];
    }
    let offset = (n - loop_start) % period;
    return orbit[loop_start + offset];
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
    let zero_orbit = uniforms.orbit_len <= 1u;
    // Non-zero reference: fall back to f32 perturbation (stacked orbit upload is follow-up).
    if (!zero_orbit) {
        let c_re = p.dc_re;
        let c_im = p.dc_im;
        for (var step = 0u; step < uniforms.bout_iterations; step = step + 1u) {
            if (p.flags & FLAG_FINISHED) != 0u {
                break;
            }
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
                continue;
            }
            var d = vec2<f32>(p.d_re, p.d_im);
            d = vec2<f32>(
                2.0 * (z_full.x * d.x - z_full.y * d.y),
                2.0 * (z_full.x * d.y + z_full.y * d.x)
            );
            let dz2 = vec2<f32>(dz.x * dz.x - dz.y * dz.y, 2.0 * dz.x * dz.y);
            dz = vec2<f32>(
                2.0 * (z_ref.x * dz.x - z_ref.y * dz.y) + dz2.x + c_re,
                2.0 * (z_ref.x * dz.y + z_ref.y * dz.x) + dz2.y + c_im
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
        }
        points[i] = p;
        return;
    }

    // Zero-orbit absolute iterate in stacked precision.
    var z = StackedC(sie_from_f32(p.dz_re), sie_from_f32(p.dz_im));
    var d = StackedC(sie_from_f32(p.d_re), sie_from_f32(p.d_im));
    let c = StackedC(sie_from_f32(p.dc_re), sie_from_f32(p.dc_im));
    let bail = sie_from_f32(uniforms.bailout_radius_squared);
    for (var step = 0u; step < uniforms.bout_iterations; step = step + 1u) {
        if (p.flags & FLAG_FINISHED) != 0u {
            break;
        }
        // z := z^2 + c ; d := 2 z d
        let z2 = sie_c_mul(z, z);
        let two_zd = sie_c_mul(sie_c_scale2(z), d);
        z = sie_c_add(z2, c);
        d = two_zd;
        p.iteration_count = p.iteration_count + 1u;
        let rad = sie_norm2(z);
        let rad_f = sie_to_f32(rad);
        if rad_f < p.min_magnitude {
            p.min_magnitude = rad_f;
            p.min_magnitude_time = p.iteration_count;
        }
        if sie_cmp(rad, bail) > 0 {
            p.dz_re = sie_to_f32(z.re);
            p.dz_im = sie_to_f32(z.im);
            p.d_re = sie_to_f32(d.re);
            p.d_im = sie_to_f32(d.im);
            p.flags = p.flags | FLAG_ESCAPED | FLAG_FINISHED;
            break;
        }
    }
    if (p.flags & FLAG_FINISHED) == 0u {
        p.dz_re = sie_to_f32(z.re);
        p.dz_im = sie_to_f32(z.im);
        p.d_re = sie_to_f32(d.re);
        p.d_im = sie_to_f32(d.im);
    }
    points[i] = p;
}
