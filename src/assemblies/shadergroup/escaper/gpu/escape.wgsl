//! f32 bailout-tail compute — R=2 answers → continue to animated radius.
//! Resident answer buffer; radius is a uniform. Shared device with colorer.

struct Answer {
    kind: u32,          // 0 Escapes, 1 Repeats, 2 Dummy
    escape_time: u32,
    small_time: u32,
    loop_period: u32,
    zr: f32,
    zi: f32,
    dcr: f32,
    dci: f32,
    cr: f32,
    ci: f32,
    smallness: f32,
    _pad: f32,
}

struct Params {
    radius_sq: f32,
    max_extra: u32,
    count: u32,
    _pad: u32,
}

struct OutValue {
    kind: u32,          // 0 Outside, 1 Inside
    big_time: u32,
    small_time: u32,
    loop_period: u32,
    smallness: f32,
    gradient_angle: f32,
    _pad0: f32,
    _pad1: f32,
}

@group(0) @binding(0) var<storage, read> answers: array<Answer>;
@group(0) @binding(1) var<uniform> params: Params;
@group(0) @binding(2) var<storage, read_write> out_values: array<OutValue>;

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let i = gid.x;
    if (i >= params.count) {
        return;
    }
    let a = answers[i];
    if (a.kind == 1u) {
        // Interior pass-through.
        out_values[i] = OutValue(
            1u,
            0u,
            a.small_time,
            a.loop_period,
            a.smallness,
            0.0,
            0.0,
            0.0,
        );
        return;
    }
    if (a.kind == 2u) {
        out_values[i] = OutValue(1u, 0u, 0u, 0u, 100.0, 0.0, 0.0, 0.0);
        return;
    }

    // Escapes: continue from R≈2 toward bailout radius.
    var z = vec2<f32>(a.zr, a.zi);
    var dc = vec2<f32>(a.dcr, a.dci);
    let c = vec2<f32>(a.cr, a.ci);
    var iters = a.escape_time;
    var rs = z.x * z.x;
    var is_ = z.y * z.y;
    var ri = z.x * z.y;
    var extra = 0u;
    loop {
        if (rs + is_ > params.radius_sq) {
            break;
        }
        if (extra >= params.max_extra) {
            break;
        }
        let d_new = vec2<f32>(
            2.0 * (z.x * dc.x - z.y * dc.y) + 1.0,
            2.0 * (z.x * dc.y + z.y * dc.x),
        );
        z = vec2<f32>(rs - is_ + c.x, 2.0 * ri + c.y);
        dc = d_new;
        iters = iters + 1u;
        rs = z.x * z.x;
        is_ = z.y * z.y;
        ri = z.x * z.y;
        extra = extra + 1u;
    }
    // arg(z / dc), reflected because screen y grows downward.
    let gradient_angle = atan2(-(z.y * dc.x - z.x * dc.y), z.x * dc.x + z.y * dc.y);
    out_values[i] = OutValue(
        0u,
        iters,
        a.small_time,
        0u,
        a.smallness,
        gradient_angle,
        0.0,
        0.0,
    );
}
