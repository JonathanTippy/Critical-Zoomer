struct Uniforms {
    viewport_size: vec2<f32>,
    seat_offset: vec2<i32>,
    zoom_match: u32,
    instruction_count: u32,
    bailout_radius: f32,
    bailout_max_extra: u32,
    origin_re: f32,
    origin_im: f32,
    space: f32,
    tile_count: u32,
    grid_w: u32,
    grid_h: u32,
    nores_r: f32,
    nores_g: f32,
    nores_b: f32,
    edge_margin: u32,
    _pad_end: u32,
    _pad_end2: u32,
}

struct TileEntry {
    origin_x: i32,
    origin_y: i32,
    pan_x: i32,
    pan_y: i32,
    zoom_delta: i32,
    slot: u32,
    rank: u32,
    _pad: u32,
}

@group(0) @binding(0) var<uniform> uniforms: Uniforms;
@group(0) @binding(1) var answer_meta: texture_2d<f32>;
@group(0) @binding(2) var answer_z: texture_2d<f32>;
@group(0) @binding(4) var<storage, read> tile_entries: array<TileEntry>;
@group(0) @binding(5) var<storage, read> tile_grid: array<u32>;

const KIND_MISSING: f32 = 0.0;
const KIND_OUTSIDE: f32 = 1.0;
const KIND_INSIDE: f32 = 2.0;

const TILE_EDGE: i32 = 64;
const SHEET_COLS: u32 = 32u;
const CELL_SLOTS: u32 = 8u;
const GRID_EMPTY: u32 = 0xffffffffu;

struct RawAnswer {
    kind: f32,
    escape_or_period: f32,
    small_time: f32,
    smallness: f32,
    zx: f32,
    zy: f32,
}

struct VsOut {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VsOut {
    // Top-left is UV (0,0) / seat (0,0). WGPU NDC has +Y up, so the first
    // vertex sits at y=+1; a bottom-left origin would flip the painted frame
    // relative to the oracle and the tile seat grid.
    var positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, 1.0),
        vec2<f32>(3.0, 1.0),
        vec2<f32>(-1.0, -3.0),
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

fn floor_div(a: i32, b: i32) -> i32 {
    let q = a / b;
    let r = a % b;
    if (r != 0 && (a < 0) != (b < 0)) {
        return q - 1;
    }
    return q;
}

fn sample_slot(slot: u32, local: vec2<i32>) -> RawAnswer {
    if (local.x < 0 || local.y < 0 || local.x >= TILE_EDGE || local.y >= TILE_EDGE) {
        return RawAnswer(KIND_MISSING, 0.0, 0.0, 0.0, 0.0, 0.0);
    }
    let col = slot % SHEET_COLS;
    let row = slot / SHEET_COLS;
    let tex = vec2<i32>(i32(col) * TILE_EDGE + local.x, i32(row) * TILE_EDGE + local.y);
    let packed = textureLoad(answer_meta, tex, 0);
    let z = textureLoad(answer_z, tex, 0);
    return RawAnswer(packed.a, packed.r, packed.g, packed.b, z.r, z.g);
}

fn try_tile(entry: TileEntry, screen: vec2<i32>) -> RawAnswer {
    if (entry.zoom_delta == 0) {
        let seat = screen + vec2<i32>(entry.pan_x, entry.pan_y);
        let local = seat - vec2<i32>(entry.origin_x, entry.origin_y);
        return sample_slot(entry.slot, local);
    }
    if (entry.zoom_delta > 0 && entry.zoom_delta <= 31) {
        let shifted = screen + vec2<i32>(entry.pan_x, entry.pan_y);
        let source = vec2<i32>(shifted.x >> u32(entry.zoom_delta), shifted.y >> u32(entry.zoom_delta));
        let local = source - vec2<i32>(entry.origin_x, entry.origin_y);
        return sample_slot(entry.slot, local);
    }
    if (entry.zoom_delta < 0 && entry.zoom_delta >= -31) {
        let mag = u32(-entry.zoom_delta);
        let shifted = screen + vec2<i32>(entry.pan_x, entry.pan_y);
        let source = vec2<i32>(shifted.x << mag, shifted.y << mag);
        let local = source - vec2<i32>(entry.origin_x, entry.origin_y);
        return sample_slot(entry.slot, local);
    }
    return RawAnswer(KIND_MISSING, 0.0, 0.0, 0.0, 0.0, 0.0);
}

fn load_raw(seat: vec2<i32>) -> RawAnswer {
    let gw = i32(uniforms.grid_w);
    let gh = i32(uniforms.grid_h);
    if (gw <= 0 || gh <= 0 || uniforms.tile_count == 0u) {
        return RawAnswer(KIND_MISSING, 0.0, 0.0, 0.0, 0.0, 0.0);
    }
    let cx = floor_div(seat.x, TILE_EDGE);
    let cy = floor_div(seat.y, TILE_EDGE);
    if (cx < 0 || cy < 0 || cx >= gw || cy >= gh) {
        return RawAnswer(KIND_MISSING, 0.0, 0.0, 0.0, 0.0, 0.0);
    }
    let base = (u32(cy) * uniforms.grid_w + u32(cx)) * CELL_SLOTS;
    var best = RawAnswer(KIND_MISSING, 0.0, 0.0, 0.0, 0.0, 0.0);
    var best_rank = 0u;
    for (var k = 0u; k < CELL_SLOTS; k = k + 1u) {
        let idx = tile_grid[base + k];
        if (idx == GRID_EMPTY) {
            continue;
        }
        if (idx >= uniforms.tile_count) {
            continue;
        }
        let entry = tile_entries[idx];
        let sample = try_tile(entry, seat);
        if (sample.kind > 0.5 && entry.rank >= best_rank) {
            best = sample;
            best_rank = entry.rank;
        }
    }
    return best;
}
