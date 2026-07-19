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

fn bailout_escape(raw: RawAnswer, seat: vec2<i32>) -> Finished {
    if (raw.kind < 0.5) {
        return Finished(KIND_MISSING, 0.0, 0.0, 0.0, 0.0);
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
    if (f.kind > 1.5) { return f.loop_period; }
    return -1.0;
}

fn opt_small_time(f: Finished) -> f32 {
    if (f.kind < 0.5) { return -1.0; }
    if (f.small_time < 0.5) { return -1.0; }
    return f.small_time;
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

fn raw_small_time(seat: vec2<i32>) -> f32 {
    let raw = load_raw(seat);
    if (raw.kind < 0.5) { return -1.0; }
    if (raw.small_time < 0.5) { return -1.0; }
    return raw.small_time;
}

fn is_ste(seat: vec2<i32>) -> bool {
    return is_increased(
        raw_small_time(seat),
        raw_small_time(seat + vec2<i32>(0, -1)),
        raw_small_time(seat + vec2<i32>(0, 1)),
        raw_small_time(seat + vec2<i32>(-1, 0)),
        raw_small_time(seat + vec2<i32>(1, 0)),
    );
}

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

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    if (uniforms.zoom_match == 0u) {
        return vec4<f32>(0.05, 0.05, 0.08, 1.0);
    }
    let screen_seat = vec2<i32>(floor(in.uv * uniforms.viewport_size));
    let finished = finished_at(screen_seat);
    if (finished.kind < 0.5) {
        return vec4<f32>(0.0, 0.0, 0.0, 1.0);
    }

    var rgb = vec3<f32>(0.0, 0.0, 0.0);
    let count = uniforms.instruction_count;
    for (var i = 0u; i < count; i = i + 1u) {
        let inst = instructions[i];
        var opacity = 0.0;
        var apply = false;
        var n = 0.0;

        if (inst.opcode == OP_ESCAPE_TIME) {
            if (finished.kind > 0.5 && finished.kind < 1.5) {
                n = normalize_value(finished.big_time, inst.normalizing);
                opacity = inst.opacity_outside;
                apply = opacity > 0.0;
            }
        } else if (inst.opcode == OP_SMALL_TIME) {
            n = normalize_value(finished.small_time, inst.normalizing);
            if (finished.kind > 1.5) {
                opacity = inst.opacity_inside;
            } else {
                opacity = inst.opacity_outside;
            }
            apply = opacity > 0.0;
        } else if (inst.opcode == OP_SMALLNESS) {
            n = normalize_value(finished.smallness, inst.normalizing);
            if (finished.kind > 1.5) {
                opacity = inst.opacity_inside;
            } else {
                opacity = inst.opacity_outside;
            }
            apply = opacity > 0.0;
        } else if (inst.opcode == OP_IN_FILAMENT) {
            if (finished.kind > 0.5 && finished.kind < 1.5 && is_in_filament(screen_seat)) {
                opacity = inst.opacity_outside;
                apply = true;
                let top = vec3<f32>(inst.color_r, inst.color_g, inst.color_b);
                rgb = layer_colors(rgb, top, opacity);
                continue;
            }
        } else if (inst.opcode == OP_OUT_FILAMENT) {
            if (finished.kind > 1.5 && is_out_filament(screen_seat)) {
                opacity = inst.opacity_outside;
                apply = true;
                let top = vec3<f32>(inst.color_r, inst.color_g, inst.color_b);
                rgb = layer_colors(rgb, top, opacity);
                continue;
            }
        } else if (inst.opcode == OP_NODES) {
            if (is_node(screen_seat, i32(inst.thickness))) {
                if (finished.kind > 1.5) {
                    opacity = inst.opacity_inside;
                } else {
                    opacity = inst.opacity_outside;
                }
                if (opacity > 0.0) {
                    let top = vec3<f32>(inst.color_r, inst.color_g, inst.color_b);
                    rgb = layer_colors(rgb, top, opacity);
                }
            }
            continue;
        } else if (inst.opcode == OP_STE) {
            if (is_ste(screen_seat)) {
                if (finished.kind > 1.5) {
                    opacity = inst.opacity_inside;
                } else {
                    opacity = inst.opacity_outside;
                }
                if (opacity > 0.0) {
                    let top = vec3<f32>(inst.color_r, inst.color_g, inst.color_b);
                    rgb = layer_colors(rgb, top, opacity);
                }
            }
            continue;
        }

        if (apply) {
            let brightness = shade_value(n, inst.period, inst.phase, inst.shading);
            let base = vec3<f32>(inst.color_r, inst.color_g, inst.color_b);
            let top = modify_color(base, brightness, inst.range);
            rgb = layer_colors(rgb, top, opacity);
        }
    }

    return vec4<f32>(rgb / 255.0, 1.0);
}
