// f32 GPU colorer — line-port of color.rs (exact Color32 parity with OG f32 shade path).

struct Pixel {
    kind: u32,           // 0 = Outside, 1 = Inside
    big_time: u32,
    small_time: u32,
    loop_period: u32,
    smallness: f32,
    gradient_angle: f32,
    _pad0: f32,
    _pad1: f32,
}

struct Layer {
    kind: u32,           // 0 escape, 1 small_time, 2 smallness, 3 in_fil, 4 out_fil, 5 nodes, 6 ste
    opacity_in: u32,
    opacity_out: u32,
    color_r: u32,
    color_g: u32,
    color_b: u32,
    range_u8: u32,
    shading: u32,        // 0 modular, 1 sinus
    normalizing: u32,    // 0 none, 1 lnln, 2 ln, 3 recip, 4 recip_ln
    thickness: u32,
    period: f32,
    period_recip: f32,
    phase: f32,
    range_f: f32,
    _pad0: f32,
    _pad1: f32,
}

struct Frame {
    width: u32,
    height: u32,
    layer_count: u32,
    _pad: u32,
}

@group(0) @binding(0) var<storage, read> pixels: array<Pixel>;
@group(0) @binding(1) var<uniform> frame: Frame;
@group(0) @binding(2) var<storage, read> layers: array<Layer>;
@group(0) @binding(3) var<storage, read_write> out_rgba: array<u32>;

fn idx(x: i32, y: i32) -> u32 {
    return u32(y) * frame.width + u32(x);
}

fn in_bounds(x: i32, y: i32) -> bool {
    return x >= 0 && y >= 0 && x < i32(frame.width) && y < i32(frame.height);
}

fn normalize_f32(method: u32, input: f32) -> f32 {
    switch method {
        case 1u: { return log(log(input)); }
        case 2u: { return log(input); }
        case 3u: { return 1.0 / input; }
        case 4u: { return log(1.0 / input); }
        default: { return input; }
    }
}

fn shade_brightness(shading: u32, phase: f32, period: f32, period_recip: f32, n: f32) -> f32 {
    let tau = 6.28318530717958647692;
    if shading == 0u {
        return ((n + phase) % period) * period_recip;
    }
    return (1.0 - cos((n + phase) * tau * period_recip)) * 0.5;
}

fn modify_color(r: u32, g: u32, b: u32, brightness: f32, range: f32) -> vec3<u32> {
    var delta_b = i32(((brightness * 255.0) - 127.0) * range);
    let cmax = i32(max(r, max(g, b)));
    let cmin = i32(min(r, min(g, b)));
    if cmin + delta_b < 0 {
        delta_b = 0 - cmin;
    }
    if cmax + delta_b > 255 {
        delta_b = 255 - cmax;
    }
    return vec3<u32>(
        u32(i32(r) + delta_b),
        u32(i32(g) + delta_b),
        u32(i32(b) + delta_b),
    );
}

fn layer_colors(bottom: vec3<u32>, top_r: u32, top_g: u32, top_b: u32, top_a: u32) -> vec3<u32> {
    let top_share = top_a;
    let bottom_share = 255u - top_share;
    return vec3<u32>(
        (bottom.x * bottom_share + top_r * top_share) >> 8u,
        (bottom.y * bottom_share + top_g * top_share) >> 8u,
        (bottom.z * bottom_share + top_b * top_share) >> 8u,
    );
}

fn pack_rgb(c: vec3<u32>) -> u32 {
    return (c.x & 255u) | ((c.y & 255u) << 8u) | ((c.z & 255u) << 16u) | (255u << 24u);
}

fn sample_outside_ext(cx: i32, cy: i32, sx: i32, sy: i32) -> vec3<f32> {
    // returns (raw, ext, valid) — valid in z as 1.0/0.0
    if !in_bounds(sx, sy) {
        return vec3<f32>(0.0, 0.0, 0.0);
    }
    let p = pixels[idx(sx, sy)];
    if p.kind != 0u {
        return vec3<f32>(0.0, 0.0, 0.0);
    }
    let ox = f32(cx - sx);
    let oy = f32(cy - sy);
    let ang = p.gradient_angle;
    let projection = ox * cos(ang) + oy * sin(ang);
    let raw = f32(p.big_time);
    return vec3<f32>(raw, raw + projection, 1.0);
}

fn is_in_filament(x: i32, y: i32) -> bool {
    let c = sample_outside_ext(x, y, x, y);
    let u = sample_outside_ext(x, y, x, y - 1);
    let d = sample_outside_ext(x, y, x, y + 1);
    let l = sample_outside_ext(x, y, x - 1, y);
    let r = sample_outside_ext(x, y, x + 1, y);
    if c.z < 0.5 {
        return false;
    }
    let peak_ud = u.z > 0.5 && d.z > 0.5 && c.y > u.y && c.y > d.y
        && (abs(c.x - u.x) > 1.0 || abs(c.x - d.x) > 1.0);
    let peak_lr = l.z > 0.5 && r.z > 0.5 && c.y > l.y && c.y > r.y
        && (abs(c.x - l.x) > 1.0 || abs(c.x - r.x) > 1.0);
    return peak_ud || peak_lr;
}

fn loop_period_at(x: i32, y: i32) -> vec2<u32> {
    // x = period, y = valid (1/0). Period 0 = unknown → invalid.
    if !in_bounds(x, y) {
        return vec2<u32>(0u, 0u);
    }
    let p = pixels[idx(x, y)];
    if p.kind != 1u || p.loop_period == 0u {
        return vec2<u32>(0u, 0u);
    }
    return vec2<u32>(p.loop_period, 1u);
}

fn is_increased_u32(c: vec2<u32>, u: vec2<u32>, d: vec2<u32>, l: vec2<u32>, r: vec2<u32>) -> bool {
    if c.y == 0u {
        return false;
    }
    if u.y == 1u && u.x < c.x { return true; }
    if d.y == 1u && d.x < c.x { return true; }
    if l.y == 1u && l.x < c.x { return true; }
    if r.y == 1u && r.x < c.x { return true; }
    return false;
}

fn is_out_filament(x: i32, y: i32) -> bool {
    let c = loop_period_at(x, y);
    let u = loop_period_at(x, y - 1);
    let d = loop_period_at(x, y + 1);
    let l = loop_period_at(x - 1, y);
    let r = loop_period_at(x + 1, y);
    return is_increased_u32(c, u, d, l, r);
}

fn small_time_at(x: i32, y: i32) -> vec2<u32> {
    if !in_bounds(x, y) {
        return vec2<u32>(0u, 0u);
    }
    let p = pixels[idx(x, y)];
    return vec2<u32>(p.small_time, 1u);
}

fn is_node_tree(x: i32, y: i32) -> bool {
    let c = small_time_at(x, y);
    let u = small_time_at(x, y - 1);
    let d = small_time_at(x, y + 1);
    let l = small_time_at(x - 1, y);
    let r = small_time_at(x + 1, y);
    return is_increased_u32(c, u, d, l, r);
}

fn smallness_at(x: i32, y: i32) -> vec2<f32> {
    // x = smallness, y = valid
    if !in_bounds(x, y) {
        return vec2<f32>(0.0, 0.0);
    }
    let p = pixels[idx(x, y)];
    return vec2<f32>(p.smallness, 1.0);
}

fn is_node(x: i32, y: i32, thickness: u32) -> bool {
    let t = i32(thickness);
    let c = smallness_at(x, y);
    let u = smallness_at(x, y - t);
    let d = smallness_at(x, y + t);
    let l = smallness_at(x - t, y);
    let r = smallness_at(x + t, y);
    if c.y < 0.5 || u.y < 0.5 || d.y < 0.5 || l.y < 0.5 || r.y < 0.5 {
        return false;
    }
    return d.x > c.x && c.x < u.x && l.x > c.x && c.x < r.x;
}

fn apply_layer(acc: vec3<u32>, layer: Layer, x: i32, y: i32, p: Pixel) -> vec3<u32> {
    switch layer.kind {
        case 0u: {
            if p.kind != 0u { return acc; }
            let n = normalize_f32(layer.normalizing, f32(p.big_time));
            let br = shade_brightness(layer.shading, layer.phase, layer.period, layer.period_recip, n);
            let rgb = modify_color(layer.color_r, layer.color_g, layer.color_b, br, layer.range_f);
            return layer_colors(acc, rgb.x, rgb.y, rgb.z, layer.opacity_out);
        }
        case 1u: {
            let n = normalize_f32(layer.normalizing, f32(p.small_time));
            let br = shade_brightness(layer.shading, layer.phase, layer.period, layer.period_recip, n);
            let rgb = modify_color(layer.color_r, layer.color_g, layer.color_b, br, layer.range_f);
            let op = select(layer.opacity_out, layer.opacity_in, p.kind == 1u);
            return layer_colors(acc, rgb.x, rgb.y, rgb.z, op);
        }
        case 2u: {
            let n = normalize_f32(layer.normalizing, p.smallness);
            let br = shade_brightness(layer.shading, layer.phase, layer.period, layer.period_recip, n);
            let rgb = modify_color(layer.color_r, layer.color_g, layer.color_b, br, layer.range_f);
            let op = select(layer.opacity_out, layer.opacity_in, p.kind == 1u);
            return layer_colors(acc, rgb.x, rgb.y, rgb.z, op);
        }
        case 3u: {
            if p.kind != 0u { return acc; }
            if is_in_filament(x, y) {
                return layer_colors(acc, layer.color_r, layer.color_g, layer.color_b, layer.opacity_out);
            }
            return acc;
        }
        case 4u: {
            if p.kind != 1u { return acc; }
            if is_out_filament(x, y) {
                return layer_colors(acc, layer.color_r, layer.color_g, layer.color_b, layer.opacity_out);
            }
            return acc;
        }
        case 5u: {
            if is_node(x, y, layer.thickness) {
                let op = select(layer.opacity_out, layer.opacity_in, p.kind == 1u);
                return layer_colors(acc, layer.color_r, layer.color_g, layer.color_b, op);
            }
            return acc;
        }
        case 6u: {
            if is_node_tree(x, y) {
                let op = select(layer.opacity_out, layer.opacity_in, p.kind == 1u);
                return layer_colors(acc, layer.color_r, layer.color_g, layer.color_b, op);
            }
            return acc;
        }
        default: { return acc; }
    }
}

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let i = gid.x;
    let n = frame.width * frame.height;
    if i >= n {
        return;
    }
    let x = i32(i % frame.width);
    let y = i32(i / frame.width);
    let p = pixels[i];
    var acc = vec3<u32>(0u, 0u, 0u);
    for (var li = 0u; li < frame.layer_count; li++) {
        acc = apply_layer(acc, layers[li], x, y, p);
    }
    out_rgba[i] = pack_rgb(acc);
}
