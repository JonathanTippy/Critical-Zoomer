//! Scatter terminal GpuPertPoint rows into a production-atlas slot (GPU-native handoff).
//! Avoids map_async harvest of the full point buffer on stationary fill.
//! D-GPU-3/4: per-tile completion counter bumps only on first seat done bit 0→1.

const FLAG_ESCAPED: u32 = 2u;
const FLAG_FINISHED: u32 = 4u;
const FLAG_PERIODIC: u32 = 16u;
const FLAG_GLITCH: u32 = 8u;
const TILE_EDGE: u32 = 64u;
const SEAT_DONE_WORDS: u32 = 128u;

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

struct ScatterParams {
    slot_origin_x: u32,
    slot_origin_y: u32,
    point_count: u32,
    slot_index: u32,
}

@group(0) @binding(0) var<storage, read> points: array<GpuPertPoint>;
@group(0) @binding(1) var<storage, read> local_seats: array<u32>;
@group(0) @binding(2) var<uniform> params: ScatterParams;
@group(0) @binding(3) var meta_tex: texture_storage_2d<rgba32float, write>;
@group(0) @binding(4) var z_tex: texture_storage_2d<rgba32float, write>;
@group(0) @binding(5) var<storage, read_write> tile_counters: array<atomic<u32>>;
@group(0) @binding(6) var<storage, read_write> seat_done: array<atomic<u32>>;
@group(0) @binding(7) var<storage, read_write> batch_terminals: atomic<u32>;

fn is_terminal(flags: u32) -> bool {
    // Match bout terminal flags for scheduling; D-GPU-1 completion is escaped|repeated
    // (FINISHED accompanies those on the bout path).
    return (flags & (FLAG_ESCAPED | FLAG_FINISHED | FLAG_PERIODIC)) != 0u
        && (flags & FLAG_GLITCH) == 0u;
}

@compute @workgroup_size(64, 1, 1)
fn scatter(@builtin(global_invocation_id) gid: vec3<u32>) {
    let i = gid.x;
    if i >= params.point_count {
        return;
    }
    let p = points[i];
    if !is_terminal(p.flags) {
        return;
    }
    let local = local_seats[i];
    if local >= TILE_EDGE * TILE_EDGE {
        return;
    }

    // Scheduling: count every terminal seat in this dispatch (may already be done).
    atomicAdd(&batch_terminals, 1u);

    // D-GPU-4: only first 0→1 transition commits + bumps the tile counter.
    let word = local / 32u;
    let bit = 1u << (local % 32u);
    let done_idx = params.slot_index * SEAT_DONE_WORDS + word;
    let prev = atomicOr(&seat_done[done_idx], bit);
    if (prev & bit) != 0u {
        return;
    }

    let lx = local % TILE_EDGE;
    let ly = local / TILE_EDGE;
    let tx = params.slot_origin_x + lx;
    let ty = params.slot_origin_y + ly;

    // Terminal calibrated outcome stored in atlas (publisher biases to Answer — D-PUB-4).
    // kind: .w = 1 Outside (escaped), 2 Inside (repeated).
    var meta_pix = vec4<f32>(0.0, 0.0, 0.0, 0.0);
    var zpix = vec4<f32>(0.0, 0.0, 0.0, 0.0);
    if (p.flags & FLAG_PERIODIC) != 0u {
        meta_pix = vec4<f32>(
            f32(p.detected_period),
            f32(p.min_magnitude_time),
            p.min_magnitude,
            2.0,
        );
    } else {
        meta_pix = vec4<f32>(
            f32(p.iteration_count),
            f32(p.min_magnitude_time),
            p.min_magnitude,
            1.0,
        );
        zpix = vec4<f32>(p.dz_re, p.dz_im, 0.0, 0.0);
    }
    textureStore(meta_tex, vec2<i32>(i32(tx), i32(ty)), meta_pix);
    textureStore(z_tex, vec2<i32>(i32(tx), i32(ty)), zpix);
    atomicAdd(&tile_counters[params.slot_index], 1u);
}
