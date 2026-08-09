// Naive Mandelbrot bout — f32. Scalar fields for stable host/GPU layout (64 bytes/seat).

struct Seat {
    c_x: f32,
    c_y: f32,
    z_x: f32,
    z_y: f32,
    dc_x: f32,
    dc_y: f32,
    real_squared: f32,
    imag_squared: f32,
    real_imag: f32,
    iterations: u32,
    loop_zx: f32,
    loop_zy: f32,
    loop_iter: u32,
    smallness: f32,
    small_time: u32,
    flags: u32,       // bit0 active, bit1 escapes, bit2 repeats
    seat_index: u32,
    _pad: u32,
}

struct Finish {
    seat_index: u32,
    flags: u32,
    iterations: u32,
    small_time: u32,
    smallness: f32,
    iter_delta: u32,
    z_x: f32,
    z_y: f32,
    dc_x: f32,
    dc_y: f32,
    c_x: f32,
    c_y: f32,
}

struct Params {
    r_squared: f32,
    epsilon: f32,
    cap: u32,
    wip_count: u32,
    generation: u32,
    _p0: u32,
    _p1: u32,
    _p2: u32,
}

@group(0) @binding(0) var<storage, read_write> seats: array<Seat>;
@group(0) @binding(1) var<storage, read_write> finishes: array<Finish>;
@group(0) @binding(2) var<storage, read_write> finish_count: array<atomic<u32>, 1>;
@group(0) @binding(3) var<uniform> params: Params;
@group(0) @binding(4) var<storage, read_write> iter_total: array<atomic<u32>, 1>;

fn update_products(i: u32) {
    var s = seats[i];
    s.real_squared = s.z_x * s.z_x;
    s.imag_squared = s.z_y * s.z_y;
    s.real_imag = s.z_x * s.z_y;
    let rad = s.real_squared + s.imag_squared;
    if (rad < s.smallness) {
        s.smallness = rad;
        s.small_time = s.iterations;
    }
    seats[i] = s;
}

fn iterate_once(i: u32) {
    var s = seats[i];
    let ndx = 2.0 * (s.z_x * s.dc_x - s.z_y * s.dc_y) + 1.0;
    let ndy = 2.0 * (s.z_x * s.dc_y + s.z_y * s.dc_x);
    let nzx = s.real_squared - s.imag_squared + s.c_x;
    let nzy = 2.0 * s.real_imag + s.c_y;
    s.z_x = nzx;
    s.z_y = nzy;
    s.dc_x = ndx;
    s.dc_y = ndy;
    s.iterations = s.iterations + 1u;
    seats[i] = s;
}

fn near(ax: f32, ay: f32, bx: f32, by: f32, e: f32) -> bool {
    return ax >= (bx - e) && ax <= (bx + e) && ay >= (by - e) && ay <= (by + e);
}

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let i = gid.x;
    if (i >= params.wip_count) { return; }
    var s = seats[i];
    if ((s.flags & 1u) == 0u) { return; }
    s.flags = 1u;
    seats[i] = s;

    let iters_before = s.iterations;
    var n = 0u;
    loop {
        if (n >= params.cap) { break; }
        update_products(i);
        s = seats[i];
        if ((s.real_squared + s.imag_squared) > params.r_squared) {
            s.flags = s.flags | 2u;
            seats[i] = s;
            break;
        }
        iterate_once(i);
        s = seats[i];
        if (near(s.z_x, s.z_y, s.loop_zx, s.loop_zy, params.epsilon)) {
            s.flags = s.flags | 4u;
            seats[i] = s;
            break;
        }
        if (s.iterations >= (s.loop_iter << 1u)) {
            s.loop_zx = s.z_x;
            s.loop_zy = s.z_y;
            s.loop_iter = s.iterations;
            seats[i] = s;
        }
        n = n + 1u;
    }

    s = seats[i];
    let delta = s.iterations - iters_before;
    atomicAdd(&iter_total[0], delta);

    if ((s.flags & 6u) != 0u) {
        s.flags = s.flags & (~1u);
        seats[i] = s;
        let slot = atomicAdd(&finish_count[0], 1u);
        finishes[slot] = Finish(
            s.seat_index,
            s.flags,
            s.iterations,
            s.small_time,
            s.smallness,
            delta,
            s.z_x, s.z_y,
            s.dc_x, s.dc_y,
            s.c_x, s.c_y,
        );
    }
}
