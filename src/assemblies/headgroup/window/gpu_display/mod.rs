use std::collections::HashMap;

use bytemuck::{Pod, Zeroable};
use eframe::egui_wgpu::{self, wgpu};
use egui::{PaintCallbackInfo, Rect};

use crate::assemblies::headgroup::window::sampling::*;
use crate::assemblies::structs::*;
use crate::constants::*;
use crate::intexp::*;
use crate::settings::*;
use crate::utils::*;

pub const GPU_TILE_SLOT_COUNT: u32 = 2048;
pub const GPU_TILE_SHEET_COLS: u32 = 32;
pub const GPU_TILE_CELL_SLOTS: u32 = 8;
pub const GPU_TILE_GRID_EMPTY: u32 = u32::MAX;

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct ShadeUniforms {
    pub viewport_size: [f32; 2]
    , pub seat_offset: [i32; 2]
    , pub zoom_match: u32
    , pub instruction_count: u32
    , pub bailout_radius: f32
    , pub bailout_max_extra: u32
    , pub origin_re: f32
    , pub origin_im: f32
    , pub space: f32
    , pub tile_count: u32
    , pub grid_w: u32
    , pub grid_h: u32
    , pub _pad1: u32
    , pub _pad2: u32
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct GpuInstruction {
    pub opcode: u32
    , pub shading: u32
    , pub normalizing: u32
    , pub thickness: u32
    , pub opacity_inside: f32
    , pub opacity_outside: f32
    , pub range: f32
    , pub period: f32
    , pub phase: f32
    , pub color_r: f32
    , pub color_g: f32
    , pub color_b: f32
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct GpuTileEntry {
    pub origin_x: i32
    , pub origin_y: i32
    , pub pan_x: i32
    , pub pan_y: i32
    , pub zoom_delta: i32
    , pub slot: u32
    , pub rank: u32
    , pub _pad: u32
}

#[derive(Clone, Debug)]
pub struct PendingTileUpload {
    pub id: u64
    , pub meta: Vec<[f32; 4]>
    , pub z: Vec<[f32; 4]>
}

pub struct ShadeFrame {
    pub uniforms: ShadeUniforms
    , pub instructions: Vec<GpuInstruction>
    , pub tile_entries: Vec<GpuTileEntry>
    , pub entry_ids: Vec<u64>
    , pub live_ids: Vec<u64>
    , pub tile_grid: Vec<u32>
    , pub pending_uploads: Vec<PendingTileUpload>
    , pub reset_gpu_slots: bool
}

pub struct GpuDisplayResources {
    pipeline: wgpu::RenderPipeline
    , bind_group_layout: wgpu::BindGroupLayout
    , uniform_buffer: wgpu::Buffer
    , instruction_buffer: wgpu::Buffer
    , instruction_capacity: u64
    , tile_entry_buffer: wgpu::Buffer
    , tile_entry_capacity: u64
    , tile_grid_buffer: wgpu::Buffer
    , tile_grid_capacity: u64
    , meta_texture: wgpu::Texture
    , meta_view: wgpu::TextureView
    , z_texture: wgpu::Texture
    , z_view: wgpu::TextureView
    , bind_group: wgpu::BindGroup
    , id_to_slot: HashMap<u64, u32>
    , free_slots: Vec<u32>
    , next_slot: u32
}

pub struct GpuDisplayCallback {
    pub frame: ShadeFrame
}

fn intexp_to_i32_checked(v: IntExp) -> Option<i32> {
    v.val.shift(v.exp).to_i32()
}

pub fn pos_delta_pixels(
    from_pos: &(IntExp, IntExp)
    , to_pos: &(IntExp, IntExp)
    , zoom_pot: i32
) -> Option<(i32, i32)> {
    let dx = (to_pos.0.clone() - from_pos.0.clone())
        .shift(zoom_pot)
        .shift(PIXELS_PER_UNIT_POT);
    let dy = (to_pos.1.clone() - from_pos.1.clone())
        .shift(zoom_pot)
        .shift(PIXELS_PER_UNIT_POT);
    Some((intexp_to_i32_checked(dx)?, intexp_to_i32_checked(dy)?))
}

pub fn seat_delta_pixels(
    hoard: &ObjectivePosAndZoom
    , current: &ObjectivePosAndZoom
) -> Option<(i32, i32)> {
    if hoard.zoom_pot != current.zoom_pot {
        return None;
    }
    pos_delta_pixels(&hoard.pos, &current.pos, hoard.zoom_pot)
}

fn normalizing_code(n: &Normalizing) -> u32 {
    match n {
        Normalizing::None {} => 0
        , Normalizing::Ln {} => 1
        , Normalizing::LnLn {} => 2
        , Normalizing::Reciprocal {} => 3
        , Normalizing::RecipLn {} => 4
    }
}

fn shading_code(s: &Shading) -> u32 {
    match s {
        Shading::Modular {} => 0
        , Shading::Sinus {} => 1
    }
}

pub fn pack_instructions(settings: &mut Settings) -> Vec<GpuInstruction> {
    let mut out = Vec::new();
    if settings.coloring_script.is_none() {
        settings.coloring_script = Some(DEFAULT_COLORING_SCRIPT.to_vec());
    }
    let Some(script) = settings.coloring_script.as_mut() else {
        return out;
    };
    for instruction in script.iter_mut() {
        match instruction {
            ColoringInstruction::PaintEscapeTime {
                opacity, color, range, shading_method, normalizing_method, ..
            } => {
                out.push(GpuInstruction {
                    opcode: 0
                    , shading: shading_code(&shading_method.shading)
                    , normalizing: normalizing_code(normalizing_method)
                    , thickness: 0
                    , opacity_inside: 0.0
                    , opacity_outside: *opacity as f32
                    , range: *range as f32 / 255.0
                    , period: shading_method.period.determine() as f32
                    , phase: shading_method.phase.determine() as f32
                    , color_r: color.0 as f32
                    , color_g: color.1 as f32
                    , color_b: color.2 as f32
                });
            }
            ColoringInstruction::PaintSmallTime {
                inside_opacity, outside_opacity, color, range, shading_method, normalizing_method, ..
            } => {
                out.push(GpuInstruction {
                    opcode: 1
                    , shading: shading_code(&shading_method.shading)
                    , normalizing: normalizing_code(normalizing_method)
                    , thickness: 0
                    , opacity_inside: *inside_opacity as f32
                    , opacity_outside: *outside_opacity as f32
                    , range: *range as f32 / 255.0
                    , period: shading_method.period.determine() as f32
                    , phase: shading_method.phase.determine() as f32
                    , color_r: color.0 as f32
                    , color_g: color.1 as f32
                    , color_b: color.2 as f32
                });
            }
            ColoringInstruction::PaintSmallness {
                inside_opacity, outside_opacity, color, range, shading_method, normalizing_method, ..
            } => {
                out.push(GpuInstruction {
                    opcode: 2
                    , shading: shading_code(&shading_method.shading)
                    , normalizing: normalizing_code(normalizing_method)
                    , thickness: 0
                    , opacity_inside: *inside_opacity as f32
                    , opacity_outside: *outside_opacity as f32
                    , range: *range as f32 / 255.0
                    , period: shading_method.period.determine() as f32
                    , phase: shading_method.phase.determine() as f32
                    , color_r: color.0 as f32
                    , color_g: color.1 as f32
                    , color_b: color.2 as f32
                });
            }
            ColoringInstruction::HighlightInFilaments { opacity, color, .. } => {
                out.push(GpuInstruction {
                    opcode: 3
                    , shading: 0
                    , normalizing: 0
                    , thickness: 0
                    , opacity_inside: 0.0
                    , opacity_outside: *opacity as f32
                    , range: 0.0
                    , period: 1.0
                    , phase: 0.0
                    , color_r: color.0 as f32
                    , color_g: color.1 as f32
                    , color_b: color.2 as f32
                });
            }
            ColoringInstruction::HighlightOutFilaments { opacity, color, .. } => {
                out.push(GpuInstruction {
                    opcode: 4
                    , shading: 0
                    , normalizing: 0
                    , thickness: 0
                    , opacity_inside: 0.0
                    , opacity_outside: *opacity as f32
                    , range: 0.0
                    , period: 1.0
                    , phase: 0.0
                    , color_r: color.0 as f32
                    , color_g: color.1 as f32
                    , color_b: color.2 as f32
                });
            }
            ColoringInstruction::HighlightNodes {
                inside_opacity, outside_opacity, color, thickness, ..
            } => {
                out.push(GpuInstruction {
                    opcode: 5
                    , shading: 0
                    , normalizing: 0
                    , thickness: *thickness as u32
                    , opacity_inside: *inside_opacity as f32
                    , opacity_outside: *outside_opacity as f32
                    , range: 0.0
                    , period: 1.0
                    , phase: 0.0
                    , color_r: color.0 as f32
                    , color_g: color.1 as f32
                    , color_b: color.2 as f32
                });
            }
            ColoringInstruction::HighlightSmallTimeEdges {
                inside_opacity, outside_opacity, color, ..
            } => {
                out.push(GpuInstruction {
                    opcode: 6
                    , shading: 0
                    , normalizing: 0
                    , thickness: 0
                    , opacity_inside: *inside_opacity as f32
                    , opacity_outside: *outside_opacity as f32
                    , range: 0.0
                    , period: 1.0
                    , phase: 0.0
                    , color_r: color.0 as f32
                    , color_g: color.1 as f32
                    , color_b: color.2 as f32
                });
            }
        }
    }
    out
}

fn pack_answer(answer: Answer) -> ([f32; 4], [f32; 4]) {
    match answer.result {
        MandelbrotResult::Outside { escape_time_r2, escape_z } => (
            [
                escape_time_r2 as f32
                , answer.min_magnitude_time as f32
                , answer.min_magnitude as f32
                , 1.0
            ]
            , [escape_z.0, escape_z.1, 0.0, 0.0]
        )
        , MandelbrotResult::Inside { period } => (
            [
                period as f32
                , answer.min_magnitude_time as f32
                , answer.min_magnitude as f32
                , 2.0
            ]
            , [0.0, 0.0, 0.0, 0.0]
        )
    }
}

pub fn pack_tile_upload(tile: &GPUTile, id: u64) -> PendingTileUpload {
    let edge = TILE_EDGE_LENGTH;
    let mut meta = vec![[0.0, 0.0, 0.0, 0.0]; edge * edge];
    let mut zbuf = vec![[0.0, 0.0, 0.0, 0.0]; edge * edge];
    for ly in 0..edge {
        for lx in 0..edge {
            let Some(a) = tile.get((lx, ly)) else { continue; };
            let answer = Answer::from(a);
            let (m, zpix) = pack_answer(answer);
            let i = ly * edge + lx;
            meta[i] = m;
            zbuf[i] = zpix;
        }
    }
    PendingTileUpload { id, meta, z: zbuf }
}

fn floor_div_i32(v: i32, edge: i32) -> i32 {
    let d = v / edge;
    let r = v % edge;
    if r != 0 && v < 0 {
        d - 1
    } else {
        d
    }
}

fn push_grid_cell(
    grid: &mut [u32]
    , grid_w: u32
    , cx: i32
    , cy: i32
    , entry_index: u32
    , rank: u32
    , entries: &[GpuTileEntry]
) {
    if cx < 0 || cy < 0 || cx >= grid_w as i32 {
        return;
    }
    let base = ((cy as u32) * grid_w + cx as u32) * GPU_TILE_CELL_SLOTS;
    if (base as usize) >= grid.len() {
        return;
    }
    for k in 0..GPU_TILE_CELL_SLOTS {
        let at = base as usize + k as usize;
        let cur = grid[at];
        if cur == GPU_TILE_GRID_EMPTY {
            grid[at] = entry_index;
            return;
        }
        if (cur as usize) < entries.len() && entries[cur as usize].rank < rank {
            grid[at] = entry_index;
            return;
        }
    }
}

fn register_tile_rect(
    grid: &mut [u32]
    , grid_w: u32
    , x0: i32
    , y0: i32
    , x1: i32
    , y1: i32
    , entry_index: u32
    , rank: u32
    , entries: &[GpuTileEntry]
) {
    let edge = TILE_EDGE_LENGTH as i32;
    let grid_h = (grid.len() as u32 / (grid_w * GPU_TILE_CELL_SLOTS)).max(1);
    let cx0 = floor_div_i32(x0, edge).max(0);
    let cy0 = floor_div_i32(y0, edge).max(0);
    let cx1 = floor_div_i32(x1.saturating_sub(1), edge).min(grid_w as i32 - 1);
    let cy1 = floor_div_i32(y1.saturating_sub(1), edge).min(grid_h as i32 - 1);
    if cx1 < cx0 || cy1 < cy0 {
        return;
    }
    for cy in cy0..=cy1 {
        for cx in cx0..=cx1 {
            push_grid_cell(grid, grid_w, cx, cy, entry_index, rank, entries);
        }
    }
}

fn register_lesser_rect_i64(
    grid: &mut [u32]
    , grid_w: u32
    , viewport: (u32, u32)
    , ox: i32
    , oy: i32
    , pan_x: i32
    , pan_y: i32
    , mag: i32
    , entry_index: u32
    , rank: u32
    , entries: &[GpuTileEntry]
) {
    let edge = TILE_EDGE_LENGTH as i64;
    let mag = mag as u32;
    let x0 = ((ox as i64) << mag).saturating_sub(pan_x as i64);
    let y0 = ((oy as i64) << mag).saturating_sub(pan_y as i64);
    let span = edge << mag;
    let x1 = x0.saturating_add(span);
    let y1 = y0.saturating_add(span);
    let max_x = viewport.0 as i64 + edge;
    let max_y = viewport.1 as i64 + edge;
    let x0c = x0.clamp(-edge, max_x) as i32;
    let y0c = y0.clamp(-edge, max_y) as i32;
    let x1c = x1.clamp(-edge, max_x) as i32;
    let y1c = y1.clamp(-edge, max_y) as i32;
    register_tile_rect(grid, grid_w, x0c, y0c, x1c, y1c, entry_index, rank, entries);
}

fn intexp_to_f32(v: &IntExp) -> f32 {
    let bits = v.clone().shift(24);
    let n: i32 = bits.into();
    (n as f32) * (-24f32).exp2()
}

pub fn build_shade_frame(
    sampling_context: &mut SamplingContext
    , settings: &mut Settings
) -> ShadeFrame {
    let t0 = std::time::Instant::now();
    sampling_context.prune_distant_tiles();
    let reset_gpu_slots = std::mem::take(&mut sampling_context.reset_gpu_tile_slots);
    let viewport = sampling_context.screen_size;
    let instructions = pack_instructions(settings);
    let space = (-(sampling_context.location.zoom_pot + PIXELS_PER_UNIT_POT) as f64).exp2() as f32;
    let origin_re = intexp_to_f32(&sampling_context.location.pos.0);
    let origin_im = intexp_to_f32(&(IntExp::ZERO - sampling_context.location.pos.1.clone()));
    let current = sampling_context.location.clone();
    let zoom = current.zoom_pot;
    let edge = TILE_EDGE_LENGTH as i32;
    let mut entries: Vec<GpuTileEntry> = Vec::new();
    let mut entry_ids: Vec<u64> = Vec::new();
    let mut overflow_skips = 0u32;
    let mut same_n = 0u32;
    let mut lesser_n = 0u32;
    let mut finer_n = 0u32;

    for ((z, ox, oy), versions) in &sampling_context.tiles {
        for (vi, tile) in versions.iter().enumerate() {
            let Some(ids) = sampling_context.tile_gpu_ids.get(&(*z, *ox, *oy)) else {
                continue;
            };
            let Some(&id) = ids.get(vi) else { continue; };
            if *z == zoom {
                let Some((pan_x, pan_y)) = seat_delta_pixels(&tile.location, &current) else {
                    overflow_skips += 1;
                    continue;
                };
                let rank = 3_000_000u32.saturating_sub(
                    (pan_x.abs().saturating_add(pan_y.abs())).min(2_999_999) as u32
                );
                entries.push(GpuTileEntry {
                    origin_x: *ox
                    , origin_y: *oy
                    , pan_x
                    , pan_y
                    , zoom_delta: 0
                    , slot: 0
                    , rank
                    , _pad: 0
                });
                entry_ids.push(id);
                same_n += 1;
            } else if *z < zoom {
                // Coarser / lesser: lowest preference band (finer wins when overlapping).
                let mag_delta = zoom - *z;
                if mag_delta > 31 {
                    overflow_skips += 1;
                    continue;
                }
                let Some((pan_x, pan_y)) = pos_delta_pixels(
                    &tile.location.pos
                    , &current.pos
                    , zoom
                ) else {
                    overflow_skips += 1;
                    continue;
                };
                let rank = 1_000_000u32.saturating_sub(
                    (pan_x.abs().saturating_add(pan_y.abs())).min(999_999) as u32
                );
                entries.push(GpuTileEntry {
                    origin_x: *ox
                    , origin_y: *oy
                    , pan_x
                    , pan_y
                    , zoom_delta: mag_delta
                    , slot: 0
                    , rank
                    , _pad: 0
                });
                entry_ids.push(id);
                lesser_n += 1;
            } else if *z > zoom {
                // Finer: prefer over coarser (dyadic: sample finer-first).
                let mag_delta = *z - zoom;
                if mag_delta > 31 {
                    overflow_skips += 1;
                    continue;
                }
                let Some((pan_x, pan_y)) = pos_delta_pixels(
                    &tile.location.pos
                    , &current.pos
                    , zoom
                ) else {
                    overflow_skips += 1;
                    continue;
                };
                let rank = 2_000_000u32.saturating_sub(
                    (pan_x.abs().saturating_add(pan_y.abs())).min(1_999_999) as u32
                );
                entries.push(GpuTileEntry {
                    origin_x: *ox
                    , origin_y: *oy
                    , pan_x
                    , pan_y
                    , zoom_delta: -mag_delta
                    , slot: 0
                    , rank
                    , _pad: 0
                });
                entry_ids.push(id);
                finer_n += 1;
            }
        }
    }

    let grid_w = ((viewport.0 as i32 + edge - 1) / edge).max(1) as u32 + 2;
    let grid_h = ((viewport.1 as i32 + edge - 1) / edge).max(1) as u32 + 2;
    let mut grid = vec![GPU_TILE_GRID_EMPTY; (grid_w * grid_h * GPU_TILE_CELL_SLOTS) as usize];
    let mut max_abs_pan = 0i32;
    for (i, entry) in entries.iter().enumerate() {
        let idx = i as u32;
        max_abs_pan = max_abs_pan
            .max(entry.pan_x.abs())
            .max(entry.pan_y.abs());
        if entry.zoom_delta == 0 {
            let Some(x0) = entry.origin_x.checked_sub(entry.pan_x) else {
                overflow_skips += 1;
                continue;
            };
            let Some(y0) = entry.origin_y.checked_sub(entry.pan_y) else {
                overflow_skips += 1;
                continue;
            };
            let Some(x1) = x0.checked_add(edge) else {
                overflow_skips += 1;
                continue;
            };
            let Some(y1) = y0.checked_add(edge) else {
                overflow_skips += 1;
                continue;
            };
            register_tile_rect(
                &mut grid
                , grid_w
                , x0
                , y0
                , x1
                , y1
                , idx
                , entry.rank
                , &entries
            );
        } else if entry.zoom_delta > 0 {
            register_lesser_rect_i64(
                &mut grid
                , grid_w
                , viewport
                , entry.origin_x
                , entry.origin_y
                , entry.pan_x
                , entry.pan_y
                , entry.zoom_delta
                , idx
                , entry.rank
                , &entries
            );
        } else if entry.zoom_delta < 0 {
            let mag = (-entry.zoom_delta) as u32;
            let span = (edge >> mag as i32).max(1);
            let Some(x0) = (entry.origin_x >> mag as i32).checked_sub(entry.pan_x) else {
                overflow_skips += 1;
                continue;
            };
            let Some(y0) = (entry.origin_y >> mag as i32).checked_sub(entry.pan_y) else {
                overflow_skips += 1;
                continue;
            };
            let Some(x1) = x0.checked_add(span) else {
                overflow_skips += 1;
                continue;
            };
            let Some(y1) = y0.checked_add(span) else {
                overflow_skips += 1;
                continue;
            };
            register_tile_rect(
                &mut grid
                , grid_w
                , x0
                , y0
                , x1
                , y1
                , idx
                , entry.rank
                , &entries
            );
        }
    }

    let pending_uploads = std::mem::take(&mut sampling_context.pending_tile_uploads);
    let upload_n = pending_uploads.len();
    let mut live_ids: Vec<u64> = Vec::new();
    for ids in sampling_context.tile_gpu_ids.values() {
        live_ids.extend(ids.iter().copied());
    }
    // #region agent log
    crate::assemblies::headgroup::window::agent_dbg(
        "H-FPS"
        , "gpu_display/mod.rs:build_shade_frame"
        , "tile_frame"
        , &format!(
            "{{\"ms\":{},\"entries\":{},\"uploads\":{},\"tiles\":{},\"live_ids\":{},\"reset\":{},\"overflow_skips\":{},\"max_abs_pan\":{},\"same\":{},\"lesser\":{},\"finer\":{},\"zoom\":{},\"grid\":[{},{}]}}"
            , t0.elapsed().as_secs_f64() * 1000.0
            , entries.len()
            , upload_n
            , sampling_context.tile_count()
            , live_ids.len()
            , reset_gpu_slots
            , overflow_skips
            , max_abs_pan
            , same_n
            , lesser_n
            , finer_n
            , zoom
            , grid_w
            , grid_h
        )
    );
    {
        use std::sync::atomic::{AtomicI32, Ordering};
        static LAST_PROBE_ZOOM: AtomicI32 = AtomicI32::new(i32::MIN);
        let prev = LAST_PROBE_ZOOM.swap(zoom, Ordering::Relaxed);
        if prev != zoom {
            let edge = TILE_EDGE_LENGTH as i32;
            let try_hit = |entry: &GpuTileEntry, seat: (i32, i32)| -> bool {
                if entry.zoom_delta == 0 {
                    let ax = seat.0 + entry.pan_x;
                    let ay = seat.1 + entry.pan_y;
                    ax >= entry.origin_x && ay >= entry.origin_y
                        && ax < entry.origin_x + edge && ay < entry.origin_y + edge
                } else if entry.zoom_delta > 0 {
                    let mag = entry.zoom_delta as u32;
                    let sx = (seat.0 + entry.pan_x) >> mag;
                    let sy = (seat.1 + entry.pan_y) >> mag;
                    sx >= entry.origin_x && sy >= entry.origin_y
                        && sx < entry.origin_x + edge && sy < entry.origin_y + edge
                } else {
                    false
                }
            };
            let probe_seat = |seat: (i32, i32)| -> (u32, i32, u32, u32) {
                let cx = floor_div_i32(seat.0, edge);
                let cy = floor_div_i32(seat.1, edge);
                if cx < 0 || cy < 0 || cx >= grid_w as i32 || cy >= grid_h as i32 {
                    return (0, 0, 0, 0);
                }
                let base = ((cy as u32) * grid_w + cx as u32) * GPU_TILE_CELL_SLOTS;
                let mut filled = 0u32;
                let mut hits = 0u32;
                let mut best_rank = 0u32;
                let mut best_zd = 0i32;
                for k in 0..GPU_TILE_CELL_SLOTS {
                    let idx = grid[base as usize + k as usize];
                    if idx == GPU_TILE_GRID_EMPTY {
                        continue;
                    }
                    filled += 1;
                    let Some(entry) = entries.get(idx as usize) else { continue; };
                    if try_hit(entry, seat) {
                        hits += 1;
                        if entry.rank >= best_rank {
                            best_rank = entry.rank;
                            best_zd = entry.zoom_delta;
                        }
                    }
                }
                (filled, best_zd, hits, best_rank)
            };
            let seats = [
                (viewport.0 as i32 / 2, viewport.1 as i32 / 2)
                , (16, viewport.1 as i32 / 2)
                , (viewport.0 as i32 - 16, viewport.1 as i32 / 2)
                , (viewport.0 as i32 / 2, 16)
                , (viewport.0 as i32 / 2, viewport.1 as i32 - 16)
            ];
            let mut miss_row = 0u32;
            let mut hit_row = 0u32;
            let y = viewport.1 as i32 / 2;
            let mut x = 0i32;
            while x < viewport.0 as i32 {
                let (filled, zd, hits, _) = probe_seat((x, y));
                if hits == 0 {
                    miss_row += 1;
                } else {
                    hit_row += 1;
                }
                let _ = (filled, zd);
                x += 32;
            }
            let c = probe_seat(seats[0]);
            let l = probe_seat(seats[1]);
            let r = probe_seat(seats[2]);
            crate::assemblies::headgroup::window::agent_dbg(
                "H-ZIB"
                , "gpu_display/mod.rs:build_shade_frame"
                , "grid_probe"
                , &format!(
                    "{{\"zoom\":{},\"lesser\":{},\"same\":{},\"center\":{{\"filled\":{},\"zd\":{},\"hits\":{}}},\"left\":{{\"filled\":{},\"zd\":{},\"hits\":{}}},\"right\":{{\"filled\":{},\"zd\":{},\"hits\":{}}},\"row_hit\":{},\"row_miss\":{}}}"
                    , zoom
                    , lesser_n
                    , same_n
                    , c.0, c.1, c.2
                    , l.0, l.1, l.2
                    , r.0, r.1, r.2
                    , hit_row
                    , miss_row
                )
            );
        }
    }
    // #endregion

    ShadeFrame {
        uniforms: ShadeUniforms {
            viewport_size: [viewport.0 as f32, viewport.1 as f32]
            , seat_offset: [0, 0]
            , zoom_match: 1
            , instruction_count: instructions.len() as u32
            , bailout_radius: settings.bailout_radius.determine() as f32
            , bailout_max_extra: settings.bailout_max_additional_iterations
            , origin_re
            , origin_im
            , space
            , tile_count: entries.len() as u32
            , grid_w
            , grid_h
            , _pad1: 0
            , _pad2: 0
        }
        , instructions
        , tile_entries: entries
        , entry_ids
        , live_ids
        , tile_grid: grid
        , pending_uploads
        , reset_gpu_slots
    }
}

pub fn ensure_resources(render_state: &egui_wgpu::RenderState) {
    let mut renderer = render_state.renderer.write();
    if renderer.callback_resources.get::<GpuDisplayResources>().is_some() {
        return;
    }
    let resources = GpuDisplayResources::new(
        &render_state.device
        , render_state.target_format
    );
    renderer.callback_resources.insert(resources);
}

pub fn paint_central_panel(ui: &mut egui::Ui, rect: Rect, frame: ShadeFrame) {
    ui.painter().add(egui_wgpu::Callback::new_paint_callback(
        rect
        , GpuDisplayCallback { frame }
    ));
}

impl GpuDisplayResources {
    fn new(device: &wgpu::Device, target_format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("headgroup_sample_shade")
            , source: wgpu::ShaderSource::Wgsl(
                concat!(include_str!("sampling.wgsl"), "\n", include_str!("shade.wgsl")).into()
            )
        });
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("headgroup_shade_bgl")
            , entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0
                    , visibility: wgpu::ShaderStages::FRAGMENT
                    , ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform
                        , has_dynamic_offset: false
                        , min_binding_size: None
                    }
                    , count: None
                }
                , wgpu::BindGroupLayoutEntry {
                    binding: 1
                    , visibility: wgpu::ShaderStages::FRAGMENT
                    , ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false }
                        , view_dimension: wgpu::TextureViewDimension::D2
                        , multisampled: false
                    }
                    , count: None
                }
                , wgpu::BindGroupLayoutEntry {
                    binding: 2
                    , visibility: wgpu::ShaderStages::FRAGMENT
                    , ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false }
                        , view_dimension: wgpu::TextureViewDimension::D2
                        , multisampled: false
                    }
                    , count: None
                }
                , wgpu::BindGroupLayoutEntry {
                    binding: 3
                    , visibility: wgpu::ShaderStages::FRAGMENT
                    , ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true }
                        , has_dynamic_offset: false
                        , min_binding_size: None
                    }
                    , count: None
                }
                , wgpu::BindGroupLayoutEntry {
                    binding: 4
                    , visibility: wgpu::ShaderStages::FRAGMENT
                    , ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true }
                        , has_dynamic_offset: false
                        , min_binding_size: None
                    }
                    , count: None
                }
                , wgpu::BindGroupLayoutEntry {
                    binding: 5
                    , visibility: wgpu::ShaderStages::FRAGMENT
                    , ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true }
                        , has_dynamic_offset: false
                        , min_binding_size: None
                    }
                    , count: None
                }
            ]
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("headgroup_shade_pl")
            , bind_group_layouts: &[&bind_group_layout]
            , push_constant_ranges: &[]
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("headgroup_shade_pipeline")
            , layout: Some(&pipeline_layout)
            , vertex: wgpu::VertexState {
                module: &shader
                , entry_point: Some("vs_main")
                , compilation_options: Default::default()
                , buffers: &[]
            }
            , fragment: Some(wgpu::FragmentState {
                module: &shader
                , entry_point: Some("fs_main")
                , compilation_options: Default::default()
                , targets: &[Some(wgpu::ColorTargetState {
                    format: target_format
                    , blend: Some(wgpu::BlendState::REPLACE)
                    , write_mask: wgpu::ColorWrites::ALL
                })]
            })
            , primitive: wgpu::PrimitiveState::default()
            , depth_stencil: None
            , multisample: wgpu::MultisampleState::default()
            , multiview: None
            , cache: None
        });
        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("headgroup_shade_uniforms")
            , size: std::mem::size_of::<ShadeUniforms>() as u64
            , usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::UNIFORM
            , mapped_at_creation: false
        });
        let instruction_capacity = 64u64;
        let instruction_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("headgroup_shade_instructions")
            , size: instruction_capacity * std::mem::size_of::<GpuInstruction>() as u64
            , usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::STORAGE
            , mapped_at_creation: false
        });
        let tile_entry_capacity = 64u64;
        let tile_entry_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("headgroup_tile_entries")
            , size: tile_entry_capacity * std::mem::size_of::<GpuTileEntry>() as u64
            , usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::STORAGE
            , mapped_at_creation: false
        });
        let tile_grid_capacity = 256u64;
        let tile_grid_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("headgroup_tile_grid")
            , size: tile_grid_capacity * 4
            , usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::STORAGE
            , mapped_at_creation: false
        });
        let sheet = (
            GPU_TILE_SHEET_COLS * TILE_EDGE_LENGTH as u32
            , (GPU_TILE_SLOT_COUNT / GPU_TILE_SHEET_COLS) * TILE_EDGE_LENGTH as u32
        );
        let (meta_texture, meta_view) = create_float_texture(device, sheet, "tile_meta_sheet");
        let (z_texture, z_view) = create_float_texture(device, sheet, "tile_z_sheet");
        let bind_group = create_bind_group(
            device
            , &bind_group_layout
            , &uniform_buffer
            , &meta_view
            , &z_view
            , &instruction_buffer
            , &tile_entry_buffer
            , &tile_grid_buffer
        );
        Self {
            pipeline
            , bind_group_layout
            , uniform_buffer
            , instruction_buffer
            , instruction_capacity
            , tile_entry_buffer
            , tile_entry_capacity
            , tile_grid_buffer
            , tile_grid_capacity
            , meta_texture
            , meta_view
            , z_texture
            , z_view
            , bind_group
            , id_to_slot: HashMap::new()
            , free_slots: Vec::new()
            , next_slot: 0
        }
    }

    fn rebuild_bind_group(&mut self, device: &wgpu::Device) {
        self.bind_group = create_bind_group(
            device
            , &self.bind_group_layout
            , &self.uniform_buffer
            , &self.meta_view
            , &self.z_view
            , &self.instruction_buffer
            , &self.tile_entry_buffer
            , &self.tile_grid_buffer
        );
    }

    fn ensure_instruction_capacity(&mut self, device: &wgpu::Device, count: u64) {
        if count <= self.instruction_capacity {
            return;
        }
        let capacity = count.next_power_of_two().max(16);
        self.instruction_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("headgroup_shade_instructions")
            , size: capacity * std::mem::size_of::<GpuInstruction>() as u64
            , usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::STORAGE
            , mapped_at_creation: false
        });
        self.instruction_capacity = capacity;
        self.rebuild_bind_group(device);
    }

    fn ensure_tile_entry_capacity(&mut self, device: &wgpu::Device, count: u64) {
        let count = count.max(1);
        if count <= self.tile_entry_capacity {
            return;
        }
        let capacity = count.next_power_of_two().max(16);
        self.tile_entry_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("headgroup_tile_entries")
            , size: capacity * std::mem::size_of::<GpuTileEntry>() as u64
            , usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::STORAGE
            , mapped_at_creation: false
        });
        self.tile_entry_capacity = capacity;
        self.rebuild_bind_group(device);
    }

    fn ensure_tile_grid_capacity(&mut self, device: &wgpu::Device, count: u64) {
        let count = count.max(1);
        if count <= self.tile_grid_capacity {
            return;
        }
        let capacity = count.next_power_of_two().max(256);
        self.tile_grid_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("headgroup_tile_grid")
            , size: capacity * 4
            , usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::STORAGE
            , mapped_at_creation: false
        });
        self.tile_grid_capacity = capacity;
        self.rebuild_bind_group(device);
    }

    fn reset_slots(&mut self) {
        self.id_to_slot.clear();
        self.free_slots.clear();
        self.next_slot = 0;
    }

    fn reclaim_dead_slots(&mut self, live_ids: &[u64]) {
        use std::collections::HashSet;
        let live: HashSet<u64> = live_ids.iter().copied().collect();
        let dead: Vec<u64> = self.id_to_slot
            .keys()
            .copied()
            .filter(|id| !live.contains(id))
            .collect();
        for id in dead {
            if let Some(slot) = self.id_to_slot.remove(&id) {
                self.free_slots.push(slot);
            }
        }
    }

    fn alloc_slot(&mut self) -> Option<u32> {
        if let Some(slot) = self.free_slots.pop() {
            return Some(slot);
        }
        if self.next_slot >= GPU_TILE_SLOT_COUNT {
            return None;
        }
        let slot = self.next_slot;
        self.next_slot += 1;
        Some(slot)
    }

    fn slot_for_id(&mut self, id: u64) -> Option<u32> {
        if let Some(&slot) = self.id_to_slot.get(&id) {
            return Some(slot);
        }
        let slot = self.alloc_slot()?;
        self.id_to_slot.insert(id, slot);
        Some(slot)
    }

    fn prepare(
        &mut self
        , device: &wgpu::Device
        , queue: &wgpu::Queue
        , frame: &ShadeFrame
    ) {
        if frame.reset_gpu_slots {
            self.reset_slots();
            // #region agent log
            crate::assemblies::headgroup::window::agent_dbg(
                "H-GREY"
                , "gpu_display/mod.rs:prepare"
                , "gpu_slots_reset"
                , "{\"reason\":\"clear_tiles\"}"
            );
            // #endregion
        }
        self.reclaim_dead_slots(&frame.live_ids);

        let mut alloc_fail = 0u32;
        for upload in &frame.pending_uploads {
            let Some(slot) = self.slot_for_id(upload.id) else {
                alloc_fail += 1;
                continue;
            };
            let origin = [
                (slot % GPU_TILE_SHEET_COLS) * TILE_EDGE_LENGTH as u32
                , (slot / GPU_TILE_SHEET_COLS) * TILE_EDGE_LENGTH as u32
            ];
            write_float_texture_at(
                queue
                , &self.meta_texture
                , &upload.meta
                , (TILE_EDGE_LENGTH as u32, TILE_EDGE_LENGTH as u32)
                , origin
            );
            write_float_texture_at(
                queue
                , &self.z_texture
                , &upload.z
                , (TILE_EDGE_LENGTH as u32, TILE_EDGE_LENGTH as u32)
                , origin
            );
        }

        let mut entries = frame.tile_entries.clone();
        for (entry, id) in entries.iter_mut().zip(frame.entry_ids.iter()) {
            if let Some(&slot) = self.id_to_slot.get(id) {
                entry.slot = slot;
            } else if let Some(slot) = self.slot_for_id(*id) {
                entry.slot = slot;
            }
        }

        self.ensure_instruction_capacity(device, frame.instructions.len() as u64);
        self.ensure_tile_entry_capacity(device, entries.len().max(1) as u64);
        self.ensure_tile_grid_capacity(device, frame.tile_grid.len().max(1) as u64);

        let mut uniforms = frame.uniforms;
        uniforms.tile_count = entries.len() as u32;
        queue.write_buffer(
            &self.uniform_buffer
            , 0
            , bytemuck::bytes_of(&uniforms)
        );
        if !frame.instructions.is_empty() {
            queue.write_buffer(
                &self.instruction_buffer
                , 0
                , bytemuck::cast_slice(&frame.instructions)
            );
        }
        if !entries.is_empty() {
            queue.write_buffer(
                &self.tile_entry_buffer
                , 0
                , bytemuck::cast_slice(&entries)
            );
        }
        if !frame.tile_grid.is_empty() {
            queue.write_buffer(
                &self.tile_grid_buffer
                , 0
                , bytemuck::cast_slice(&frame.tile_grid)
            );
        }
        // #region agent log
        if alloc_fail > 0 || frame.reset_gpu_slots {
            crate::assemblies::headgroup::window::agent_dbg(
                "H-GREY"
                , "gpu_display/mod.rs:prepare"
                , "gpu_slot_stats"
                , &format!(
                    "{{\"alloc_fail\":{},\"used\":{},\"free\":{},\"next\":{},\"uploads\":{},\"entries\":{},\"live\":{}}}"
                    , alloc_fail
                    , self.id_to_slot.len()
                    , self.free_slots.len()
                    , self.next_slot
                    , frame.pending_uploads.len()
                    , frame.tile_entries.len()
                    , frame.live_ids.len()
                )
            );
        }
        // #endregion
    }

    fn paint(&self, render_pass: &mut wgpu::RenderPass<'static>) {
        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_bind_group(0, &self.bind_group, &[]);
        render_pass.draw(0..3, 0..1);
    }
}

fn create_float_texture(
    device: &wgpu::Device
    , size: (u32, u32)
    , label: &str
) -> (wgpu::Texture, wgpu::TextureView) {
    let width = size.0.max(1);
    let height = size.1.max(1);
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label)
        , size: wgpu::Extent3d {
            width
            , height
            , depth_or_array_layers: 1
        }
        , mip_level_count: 1
        , sample_count: 1
        , dimension: wgpu::TextureDimension::D2
        , format: wgpu::TextureFormat::Rgba32Float
        , usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST
        , view_formats: &[]
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    (texture, view)
}

fn write_float_texture_at(
    queue: &wgpu::Queue
    , texture: &wgpu::Texture
    , pixels: &[[f32; 4]]
    , size: (u32, u32)
    , origin: [u32; 2]
) {
    if pixels.is_empty() || size.0 == 0 || size.1 == 0 {
        return;
    }
    let unpadded = (size.0 * 16) as usize;
    let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT as usize;
    let padded = (unpadded + align - 1) / align * align;
    let mut bytes = vec![0u8; padded * size.1 as usize];
    let src = bytemuck::cast_slice::<[f32; 4], u8>(pixels);
    for y in 0..size.1 as usize {
        let src_off = y * unpadded;
        let dst_off = y * padded;
        bytes[dst_off..dst_off + unpadded]
            .copy_from_slice(&src[src_off..src_off + unpadded]);
    }
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture
            , mip_level: 0
            , origin: wgpu::Origin3d {
                x: origin[0]
                , y: origin[1]
                , z: 0
            }
            , aspect: wgpu::TextureAspect::All
        }
        , &bytes
        , wgpu::TexelCopyBufferLayout {
            offset: 0
            , bytes_per_row: Some(padded as u32)
            , rows_per_image: Some(size.1)
        }
        , wgpu::Extent3d {
            width: size.0
            , height: size.1
            , depth_or_array_layers: 1
        }
    );
}

fn create_bind_group(
    device: &wgpu::Device
    , layout: &wgpu::BindGroupLayout
    , uniform_buffer: &wgpu::Buffer
    , meta_view: &wgpu::TextureView
    , z_view: &wgpu::TextureView
    , instruction_buffer: &wgpu::Buffer
    , tile_entry_buffer: &wgpu::Buffer
    , tile_grid_buffer: &wgpu::Buffer
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("headgroup_shade_bg")
        , layout
        , entries: &[
            wgpu::BindGroupEntry {
                binding: 0
                , resource: uniform_buffer.as_entire_binding()
            }
            , wgpu::BindGroupEntry {
                binding: 1
                , resource: wgpu::BindingResource::TextureView(meta_view)
            }
            , wgpu::BindGroupEntry {
                binding: 2
                , resource: wgpu::BindingResource::TextureView(z_view)
            }
            , wgpu::BindGroupEntry {
                binding: 3
                , resource: instruction_buffer.as_entire_binding()
            }
            , wgpu::BindGroupEntry {
                binding: 4
                , resource: tile_entry_buffer.as_entire_binding()
            }
            , wgpu::BindGroupEntry {
                binding: 5
                , resource: tile_grid_buffer.as_entire_binding()
            }
        ]
    })
}

impl egui_wgpu::CallbackTrait for GpuDisplayCallback {
    fn prepare(
        &self
        , device: &wgpu::Device
        , queue: &wgpu::Queue
        , _screen_descriptor: &egui_wgpu::ScreenDescriptor
        , _egui_encoder: &mut wgpu::CommandEncoder
        , callback_resources: &mut egui_wgpu::CallbackResources
    ) -> Vec<wgpu::CommandBuffer> {
        let Some(resources) = callback_resources.get_mut::<GpuDisplayResources>() else {
            return Vec::new();
        };
        resources.prepare(device, queue, &self.frame);
        Vec::new()
    }

    fn paint(
        &self
        , _info: PaintCallbackInfo
        , render_pass: &mut wgpu::RenderPass<'static>
        , callback_resources: &egui_wgpu::CallbackResources
    ) {
        let Some(resources) = callback_resources.get::<GpuDisplayResources>() else {
            return;
        };
        resources.paint(render_pass);
    }
}

#[cfg(test)]
mod b_ten_1_tests {
    use super::*;
    use crate::constants::NORES_ANSWER;
    use crate::utils::ObjectivePosAndZoom;

    #[test]
    fn nores_answer_packs_as_outside_not_missing() {
        let mut tile = Tile::new((0, 0), -2);
        tile.set((0, 0), NORES_ANSWER);
        let gpu = GPUTile::from_answer_tile(
            &tile
            , (64, 64)
            , ObjectivePosAndZoom::from((-2, -2, -2))
        );
        let upload = pack_tile_upload(&gpu, 1);
        let meta = upload.meta[0];
        assert_eq!(meta[3], 1.0, "NORES must pack KIND_OUTSIDE, got kind={}", meta[3]);
        assert_eq!(meta[0], 1.0, "NORES escape time must be 1");
    }
}
