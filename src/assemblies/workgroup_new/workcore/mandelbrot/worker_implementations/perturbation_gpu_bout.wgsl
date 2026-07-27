struct Uniforms {
    bailout_radius_squared: f32,
    bout_iterations: u32,
    orbit_len: u32,
    point_count: u32,
    glitch_threshold: f32,
    _pad0: f32,
    _pad1: f32,
    _pad2: f32,
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
    _pad0: u32,
    _pad1: u32,
}

@group(0) @binding(0) var<uniform> uniforms: Uniforms;
@group(0) @binding(1) var<storage, read_write> points: array<GpuPertPoint>;
@group(0) @binding(2) var<storage, read> orbit: array<vec2<f32>>;

const FLAG_ACTIVE: u32 = 1u;
const FLAG_ESCAPED: u32 = 2u;
const FLAG_FINISHED: u32 = 4u;
const FLAG_GLITCH: u32 = 8u;

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
    for (var step = 0u; step < uniforms.bout_iterations; step = step + 1u) {
        if (p.flags & FLAG_FINISHED) != 0u {
            break;
        }
        let n = p.iteration_count;
        let z_ref = orbit_at(n);
        var dz = vec2<f32>(p.dz_re, p.dz_im);
        let z_full = z_ref + dz;
        let z_ref_mag2 = dot(z_ref, z_ref);
        let z_full_mag2 = dot(z_full, z_full);
        if z_ref_mag2 > 1e-30 && z_full_mag2 < uniforms.glitch_threshold * z_ref_mag2 {
            p.flags = p.flags | FLAG_GLITCH | FLAG_FINISHED;
            break;
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
            // Store escaped full z in dz slots for the host.
            p.dz_re = z_full_next.x;
            p.dz_im = z_full_next.y;
            p.flags = p.flags | FLAG_ESCAPED | FLAG_FINISHED;
            break;
        }
    }
    points[i] = p;
}
