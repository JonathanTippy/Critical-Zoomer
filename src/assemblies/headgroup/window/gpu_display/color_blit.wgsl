struct Uniforms {
    viewport_size: vec2<f32>,
    source_size: vec2<f32>,
    seat_offset: vec2<i32>,
    zoom_match: u32,
    _pad: u32,
}

@group(0) @binding(0) var<uniform> uniforms: Uniforms;
@group(0) @binding(1) var color_atlas: texture_2d<f32>;
@group(0) @binding(2) var color_sampler: sampler;

struct VsOut {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VsOut {
    var positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(3.0, -1.0),
        vec2<f32>(-1.0, 3.0),
    );
    var uvs = array<vec2<f32>, 3>(
        vec2<f32>(0.0, 0.0),
        vec2<f32>(2.0, 0.0),
        vec2<f32>(0.0, 2.0),
    );
    var out: VsOut;
    out.position = vec4<f32>(positions[vertex_index], 0.0, 1.0);
    out.uv = uvs[vertex_index];
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    if (uniforms.zoom_match == 0u) {
        return vec4<f32>(0.05, 0.05, 0.08, 1.0);
    }
    let screen_seat = vec2<i32>(floor(in.uv * uniforms.viewport_size));
    let source_seat = screen_seat + uniforms.seat_offset;
    if (source_seat.x < 0 || source_seat.y < 0
        || source_seat.x >= i32(uniforms.source_size.x)
        || source_seat.y >= i32(uniforms.source_size.y)) {
        return vec4<f32>(0.0, 0.0, 0.0, 1.0);
    }
    let uv = (vec2<f32>(source_seat) + vec2<f32>(0.5, 0.5)) / uniforms.source_size;
    return textureSampleLevel(color_atlas, color_sampler, uv, 0.0);
}
