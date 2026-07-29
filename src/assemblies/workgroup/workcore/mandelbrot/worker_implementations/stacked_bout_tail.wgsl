// Stacked-i32 bout tail (appended by gears::stacked_bout_wgsl).
// Same bind layout as perturbation_gpu_bout.wgsl.
// δz / derivative iterate in stacked limbs for both zero and non-zero orbits.
// Non-zero Z_ref is projected from the f32 orbit mirror via sie_from_f32
// (CPU parity uses the same projection until native stacked orbit buffers land).

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
    if (abs(v) < 1e-30) {
        return sie_zero();
    }
    var bits = bitcast<u32>(v);
    let sign = (bits >> 31u) != 0u;
    var exp_bits = i32((bits >> 23u) & 0xffu) - 127;
    var mant = (bits & 0x7fffffu) | 0x800000u;
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

fn orbit_at_stacked(n: u32) -> StackedC {
    let z = orbit_at(n);
    return StackedC(sie_from_f32(z.x), sie_from_f32(z.y));
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

    var dz = StackedC(sie_from_f32(p.dz_re), sie_from_f32(p.dz_im));
    var d = StackedC(sie_from_f32(p.d_re), sie_from_f32(p.d_im));
    let dc = StackedC(sie_from_f32(p.dc_re), sie_from_f32(p.dc_im));
    let bail = sie_from_f32(uniforms.bailout_radius_squared);
    let glitch = sie_from_f32(uniforms.glitch_threshold);
    let tiny = sie_from_f32(1e-30);

    for (var step = 0u; step < uniforms.bout_iterations; step = step + 1u) {
        if (p.flags & FLAG_FINISHED) != 0u {
            break;
        }
        if (p.flags & FLAG_GLITCH) != 0u {
            dz = StackedC(sie_zero(), sie_zero());
            p.flags = (p.flags | FLAG_ACTIVE) & (~FLAG_GLITCH) & (~FLAG_FINISHED);
        }

        let z_ref = orbit_at_stacked(p.iteration_count);
        let z_full = sie_c_add(z_ref, dz);
        let z_ref_mag2 = sie_norm2(z_ref);
        let z_full_mag2 = sie_norm2(z_full);
        // Glitch: |Z+z| << |Z| (skip when Z≈0, i.e. zero orbit).
        if sie_cmp(z_ref_mag2, tiny) > 0
            && sie_cmp(z_full_mag2, sie_mul(glitch, z_ref_mag2)) < 0
        {
            p.flags = p.flags | FLAG_GLITCH;
            continue;
        }

        // d := 2 (Z+z) d ; dz := 2 Z dz + dz^2 + dc
        let new_d = sie_c_mul(sie_c_scale2(z_full), d);
        let dz2 = sie_c_mul(dz, dz);
        let two_z_dz = sie_c_mul(sie_c_scale2(z_ref), dz);
        dz = sie_c_add(sie_c_add(two_z_dz, dz2), dc);
        d = new_d;
        p.iteration_count = p.iteration_count + 1u;

        let z_ref_next = orbit_at_stacked(p.iteration_count);
        let z_full_next = sie_c_add(z_ref_next, dz);
        let rad = sie_norm2(z_full_next);
        let rad_f = sie_to_f32(rad);
        if rad_f < p.min_magnitude {
            p.min_magnitude = rad_f;
            p.min_magnitude_time = p.iteration_count;
        }
        if sie_cmp(rad, bail) > 0 {
            p.dz_re = sie_to_f32(z_full_next.re);
            p.dz_im = sie_to_f32(z_full_next.im);
            p.d_re = sie_to_f32(d.re);
            p.d_im = sie_to_f32(d.im);
            p.flags = p.flags | FLAG_ESCAPED | FLAG_FINISHED;
            break;
        }
    }

    if (p.flags & FLAG_FINISHED) == 0u {
        p.dz_re = sie_to_f32(dz.re);
        p.dz_im = sie_to_f32(dz.im);
        p.d_re = sie_to_f32(d.re);
        p.d_im = sie_to_f32(d.im);
    }
    points[i] = p;
}
