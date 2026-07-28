struct Instruction {
    opcode: u32,
    shading: u32,
    normalizing: u32,
    thickness: u32,
    opacity_inside: f32,
    opacity_outside: f32,
    range: f32,
    period: f32,
    phase: f32,
    color_r: f32,
    color_g: f32,
    color_b: f32,
}

@group(0) @binding(3) var<storage, read> instructions: array<Instruction>;

const OP_ESCAPE_TIME: u32 = 0u;
const OP_SMALL_TIME: u32 = 1u;
const OP_SMALLNESS: u32 = 2u;
const OP_IN_FILAMENT: u32 = 3u;
const OP_OUT_FILAMENT: u32 = 4u;
const OP_NODES: u32 = 5u;
const OP_STE: u32 = 6u;

const SHADE_MODULAR: u32 = 0u;
const SHADE_SINUS: u32 = 1u;

const NORM_NONE: u32 = 0u;
const NORM_LN: u32 = 1u;
const NORM_LNLN: u32 = 2u;
const NORM_RECIP: u32 = 3u;
const NORM_RECIP_LN: u32 = 4u;

struct Finished {
    kind: f32,
    big_time: f32,
    small_time: f32,
    smallness: f32,
    loop_period: f32,
}

// Guard the domain rather than hand back an infinity: the log floors its input at 1, the
// log of the log floors at e, and the reciprocals keep their divisor off zero.
fn normalize_value(n: f32, method: u32) -> f32 {
    if (method == NORM_LN) {
        return log(max(n, 1.0));
    }
    if (method == NORM_LNLN) {
        return log(log(max(n, exp(1.0))));
    }
    if (method == NORM_RECIP) {
        return 1.0 / max(n, 1.0e-6);
    }
    if (method == NORM_RECIP_LN) {
        return 1.0 / max(log(max(n, 1.0)), 1.0e-6);
    }
    return n;
}

fn shade_value(n: f32, period: f32, phase: f32, shading: u32) -> f32 {
    let period_recip = 1.0 / max(period, 1.0e-6);
    if (shading == SHADE_SINUS) {
        return (1.0 - cos((n + phase) * 6.28318530718 * period_recip)) * 0.5;
    }
    return fract((n + phase) * period_recip);
}

fn modify_color(color: vec3<f32>, brightness: f32, range: f32) -> vec3<f32> {
    var delta = ((brightness * 255.0) - 127.0) * range;
    let cmax = max(max(color.r, color.g), color.b);
    let cmin = min(min(color.r, color.g), color.b);
    if (cmin + delta < 0.0) { delta = -cmin; }
    if (cmax + delta > 255.0) { delta = 255.0 - cmax; }
    return clamp(color + vec3<f32>(delta, delta, delta), vec3<f32>(0.0), vec3<f32>(255.0));
}

fn layer_colors(bottom: vec3<f32>, top: vec3<f32>, opacity: f32) -> vec3<f32> {
    let top_share = opacity;
    let bottom_share = 255.0 - top_share;
    return floor((bottom * bottom_share + top * top_share) / 256.0);
}

// r[impl cz.shade.escape-continues-to-bailout+1]
// An outside answer carries on from where it crossed r=2 up to the settings bailout radius.
// A seat no tile covers is not black: it reads as a point which escaped immediately, with a
// smallness too large to ever be mistaken for a node.
fn bailout_escape(raw: RawAnswer, seat: vec2<i32>) -> Finished {
    if (raw.kind < 0.5) {
        return Finished(KIND_OUTSIDE, 1.0, 0.0, 1.0e30, 0.0);
    }
    if (raw.kind > 1.5) {
        return Finished(KIND_INSIDE, 0.0, raw.small_time, raw.smallness, raw.escape_or_period);
    }
    let c = vec2<f32>(
        uniforms.origin_re + f32(seat.x) * uniforms.space,
        uniforms.origin_im - f32(seat.y) * uniforms.space,
    );
    var z = vec2<f32>(raw.zx, raw.zy);
    var iterations = raw.escape_or_period;
    let r2 = uniforms.bailout_radius * uniforms.bailout_radius;
    var extra = 0u;
    loop {
        let mag2 = dot(z, z);
        if (mag2 > r2) { break; }
        if (extra >= uniforms.bailout_max_extra) { break; }
        z = vec2<f32>(z.x * z.x - z.y * z.y, 2.0 * z.x * z.y) + c;
        iterations += 1.0;
        extra += 1u;
    }
    return Finished(KIND_OUTSIDE, iterations, raw.small_time, raw.smallness, 0.0);
}

fn finished_at(screen_seat: vec2<i32>) -> Finished {
    return bailout_escape(load_raw(screen_seat), screen_seat);
}

fn opt_escape_time(f: Finished) -> f32 {
    if (f.kind > 0.5 && f.kind < 1.5) { return f.big_time; }
    return -1.0;
}

fn opt_period(f: Finished) -> f32 {
    if (f.kind > 1.5 && f.loop_period > 0.5) { return f.loop_period; }
    return -1.0;
}

fn opt_smallness(f: Finished) -> f32 {
    if (f.kind < 0.5) { return -1.0; }
    return f.smallness;
}

fn is_increased(v: f32, up: f32, down: f32, left: f32, right: f32) -> bool {
    if (v >= 0.0 && up >= 0.0 && up < v) { return true; }
    if (v >= 0.0 && down >= 0.0 && down < v) { return true; }
    if (v >= 0.0 && left >= 0.0 && left < v) { return true; }
    if (v >= 0.0 && right >= 0.0 && right < v) { return true; }
    return false;
}

fn slope_sign_changed(v: f32, up: f32, down: f32, left: f32, right: f32) -> bool {
    if (v >= 0.0 && up >= 0.0 && down >= 0.0 && down < v && v > up) { return true; }
    if (v >= 0.0 && left >= 0.0 && right >= 0.0 && left < v && v > right) { return true; }
    return false;
}

fn is_local_minimum(v: f32, up: f32, down: f32, left: f32, right: f32) -> bool {
    return v >= 0.0 && up >= 0.0 && down >= 0.0 && left >= 0.0 && right >= 0.0
        && down > v && v < up && left > v && v < right;
}

// r[impl cz.shade.in-filament-slope-inversion+1]
fn is_in_filament(seat: vec2<i32>) -> bool {
    let c = finished_at(seat);
    let u = finished_at(seat + vec2<i32>(0, -1));
    let d = finished_at(seat + vec2<i32>(0, 1));
    let l = finished_at(seat + vec2<i32>(-1, 0));
    let r = finished_at(seat + vec2<i32>(1, 0));
    return slope_sign_changed(
        opt_escape_time(c),
        opt_escape_time(u),
        opt_escape_time(d),
        opt_escape_time(l),
        opt_escape_time(r),
    );
}

// r[impl cz.shade.out-filament-period-step+1]
fn is_out_filament(seat: vec2<i32>) -> bool {
    let c = finished_at(seat);
    let u = finished_at(seat + vec2<i32>(0, -1));
    let d = finished_at(seat + vec2<i32>(0, 1));
    let l = finished_at(seat + vec2<i32>(-1, 0));
    let r = finished_at(seat + vec2<i32>(1, 0));
    return is_increased(
        opt_period(c),
        opt_period(u),
        opt_period(d),
        opt_period(l),
        opt_period(r),
    );
}

// A small time of zero says there is no ridge here, so it never takes part in the edge
// comparison on either side. It is still an ordinary value to paint with.
fn raw_small_time(seat: vec2<i32>) -> f32 {
    let raw = load_raw(seat);
    if (raw.kind < 0.5) { return -1.0; }
    if (raw.small_time < 0.5) { return -1.0; }
    return raw.small_time;
}

// r[impl cz.shade.small-time-edge-nonzero+1]
fn is_ste(seat: vec2<i32>) -> bool {
    return is_increased(
        raw_small_time(seat),
        raw_small_time(seat + vec2<i32>(0, -1)),
        raw_small_time(seat + vec2<i32>(0, 1)),
        raw_small_time(seat + vec2<i32>(-1, 0)),
        raw_small_time(seat + vec2<i32>(1, 0)),
    );
}

// r[impl cz.shade.node-smallness-minimum+1]
fn is_node(seat: vec2<i32>, thickness: i32) -> bool {
    let c = finished_at(seat);
    let u = finished_at(seat + vec2<i32>(0, -thickness));
    let d = finished_at(seat + vec2<i32>(0, thickness));
    let l = finished_at(seat + vec2<i32>(-thickness, 0));
    let r = finished_at(seat + vec2<i32>(thickness, 0));
    return is_local_minimum(
        opt_smallness(c),
        opt_smallness(u),
        opt_smallness(d),
        opt_smallness(l),
        opt_smallness(r),
    );
}

fn paint(bottom: vec3<f32>, base: vec3<f32>, n: f32, inst: Instruction, opacity: f32) -> vec3<f32> {
    let brightness = shade_value(n, inst.period, inst.phase, inst.shading);
    let top = modify_color(base, brightness, inst.range);
    return layer_colors(bottom, top, opacity);
}

// r[impl cz.shade.layers-in-script-order+1]
// Each layer goes over what is under it, in the order the script is written. A layer whose
// opacity for this seat's side is zero is skipped outright, so a disabled layer costs nothing
// and cannot nudge the pixel it is sitting on.
@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    if (uniforms.zoom_match == 0u) {
        return vec4<f32>(0.05, 0.05, 0.08, 1.0);
    }
    let screen_seat = vec2<i32>(floor(in.uv * uniforms.viewport_size));
    let finished = finished_at(screen_seat);
    let inside = finished.kind > 1.5;
    let outside = finished.kind > 0.5 && finished.kind < 1.5;

    var rgb = vec3<f32>(0.0, 0.0, 0.0);
    let count = uniforms.instruction_count;
    for (var i = 0u; i < count; i = i + 1u) {
        let inst = instructions[i];
        let base = vec3<f32>(inst.color_r, inst.color_g, inst.color_b);
        var side = inst.opacity_outside;
        if (inside) {
            side = inst.opacity_inside;
        }

        if (inst.opcode == OP_ESCAPE_TIME) {
            if (outside && inst.opacity_outside > 0.0) {
                let n = normalize_value(finished.big_time, inst.normalizing);
                rgb = paint(rgb, base, n, inst, inst.opacity_outside);
            }
        } else if (inst.opcode == OP_SMALL_TIME) {
            if (side > 0.0) {
                let n = normalize_value(finished.small_time, inst.normalizing);
                rgb = paint(rgb, base, n, inst, side);
            }
        } else if (inst.opcode == OP_SMALLNESS) {
            if (side > 0.0) {
                let n = normalize_value(finished.smallness, inst.normalizing);
                rgb = paint(rgb, base, n, inst, side);
            }
        } else if (inst.opcode == OP_IN_FILAMENT) {
            if (outside && inst.opacity_outside > 0.0 && is_in_filament(screen_seat)) {
                rgb = layer_colors(rgb, base, inst.opacity_outside);
            }
        } else if (inst.opcode == OP_OUT_FILAMENT) {
            if (inside && inst.opacity_outside > 0.0 && is_out_filament(screen_seat)) {
                rgb = layer_colors(rgb, base, inst.opacity_outside);
            }
        } else if (inst.opcode == OP_NODES) {
            if (side > 0.0 && is_node(screen_seat, i32(inst.thickness))) {
                rgb = layer_colors(rgb, base, side);
            }
        } else if (inst.opcode == OP_STE) {
            if (side > 0.0 && is_ste(screen_seat)) {
                rgb = layer_colors(rgb, base, side);
            }
        }
    }

    return vec4<f32>(rgb / 255.0, 1.0);
}
