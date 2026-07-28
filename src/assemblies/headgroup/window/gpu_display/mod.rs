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

#[cfg(test)]
pub(crate) mod shade_harness;
#[cfg(test)]
pub(crate) mod shade_oracle;
#[cfg(test)]
mod shade_tests;

pub mod finished_answer;
#[cfg(test)]
pub mod color;

/// Slots the atlas starts with. Not a ceiling: the headgroup hoard grows with
/// the user's memory limit, which requirements put at "unlimited" maximum, so
/// running out of slots grows the atlas rather than dropping a tile.
pub const GPU_TILE_SLOT_COUNT: u32 = 2048;
pub const GPU_TILE_CELL_SLOTS: u32 = 8;
pub const GPU_TILE_GRID_EMPTY: u32 = u32::MAX;

// Sheet geometry is shared with the workgroup's production atlas so a handoff
// between the two hoards is a plain slot-to-slot copy. See tile_sheet.
pub use crate::assemblies::tile_sheet::{
    SHEET_COLS as GPU_TILE_SHEET_COLS, SLOT_BYTES as GPU_TILE_SLOT_BYTES,
};
use crate::assemblies::tile_sheet;

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
    // CPU-packed texels. Empty when `production_slot` is set: the headgroup
    // then copies from the workgroup atlas instead of uploading bytes.
    , pub meta: Vec<[f32; 4]>
    , pub z: Vec<[f32; 4]>
    // Workgroup production-atlas slot to copy from. Released after the copy.
    , pub production_slot: Option<u32>
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
    // Slots the sheets currently hold. Grows on demand; never a fixed cap.
    , slot_capacity: u32
    // Slots the device can never exceed, from its max texture dimension.
    , slot_ceiling: u32
    // Slots the last prepare could not place. Non-zero means the hoard wants
    // more VRAM than the atlas can currently give, which is a memory bump.
    , slots_denied: u32
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
    PendingTileUpload { id, meta, z: zbuf, production_slot: None }
}

/// Queue a handoff of a tile that already lives in the workgroup atlas.
pub fn pending_handoff(id: u64, production_slot: u32) -> PendingTileUpload {
    PendingTileUpload {
        id
        , meta: Vec::new()
        , z: Vec::new()
        , production_slot: Some(production_slot)
    }
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
    let mut live_ids: Vec<u64> = Vec::new();
    for ids in sampling_context.tile_gpu_ids.values() {
        live_ids.extend(ids.iter().copied());
    }

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
        let slot_ceiling = tile_sheet::max_slots_for(device);
        let slot_capacity = GPU_TILE_SLOT_COUNT.min(slot_ceiling);
        let (meta_texture, meta_view) =
            tile_sheet::create_sheet(device, slot_capacity, "tile_meta_sheet");
        let (z_texture, z_view) =
            tile_sheet::create_sheet(device, slot_capacity, "tile_z_sheet");
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
            , slot_capacity
            , slot_ceiling
            , slots_denied: 0
        }
    }

    /// Slots the atlas currently holds.
    pub fn slot_capacity(&self) -> u32 {
        self.slot_capacity
    }

    /// VRAM the atlas currently occupies, for the memory budget.
    pub fn atlas_bytes(&self) -> u64 {
        u64::from(self.slot_capacity) * GPU_TILE_SLOT_BYTES
    }

    /// Tiles the last frame could not place for want of atlas room.
    ///
    /// The headgroup turns this into a memory bump: on-screen and lookahead work
    /// is never evicted for memory, so when it does not fit, the limit rises.
    pub fn slots_denied(&self) -> u32 {
        self.slots_denied
    }

    /// Grow the sheets so at least `wanted` slots fit, preserving what is in them.
    ///
    /// Copies texture to texture on the same device, so growing the hoard never
    /// round-trips a tile through CPU memory.
    fn grow_atlas(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, wanted: u32) -> bool {
        if wanted <= self.slot_capacity {
            return true;
        }
        if self.slot_capacity >= self.slot_ceiling {
            return false;
        }
        let capacity = wanted
            .next_power_of_two()
            .max(GPU_TILE_SHEET_COLS)
            .min(self.slot_ceiling);
        let (meta_texture, meta_view) =
            tile_sheet::create_sheet(device, capacity, "tile_meta_sheet");
        let (z_texture, z_view) = tile_sheet::create_sheet(device, capacity, "tile_z_sheet");

        // Slot origins depend only on the column count, so the old contents
        // occupy a prefix of rows and copy across as one block.
        let old_sheet = tile_sheet::sheet_size_for(self.slot_capacity);
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("headgroup_atlas_grow")
        });
        for (src, dst) in [
            (&self.meta_texture, &meta_texture)
            , (&self.z_texture, &z_texture)
        ] {
            encoder.copy_texture_to_texture(
                src.as_image_copy()
                , dst.as_image_copy()
                , wgpu::Extent3d {
                    width: old_sheet.0
                    , height: old_sheet.1
                    , depth_or_array_layers: 1
                }
            );
        }
        queue.submit(Some(encoder.finish()));

        self.meta_texture = meta_texture;
        self.meta_view = meta_view;
        self.z_texture = z_texture;
        self.z_view = z_view;
        self.slot_capacity = capacity;
        self.rebuild_bind_group(device);
        true
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

    /// Take a slot, growing the atlas rather than refusing while growth is possible.
    fn alloc_slot(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) -> Option<u32> {
        if let Some(slot) = self.free_slots.pop() {
            return Some(slot);
        }
        if self.next_slot >= self.slot_capacity
            && !self.grow_atlas(device, queue, self.next_slot + 1)
        {
            return None;
        }
        if self.next_slot >= self.slot_capacity {
            return None;
        }
        let slot = self.next_slot;
        self.next_slot += 1;
        Some(slot)
    }

    fn slot_for_id(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, id: u64) -> Option<u32> {
        if let Some(&slot) = self.id_to_slot.get(&id) {
            return Some(slot);
        }
        let slot = self.alloc_slot(device, queue)?;
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
        }
        self.reclaim_dead_slots(&frame.live_ids);

        // Size the atlas to the whole live hoard up front, so growth happens in
        // one reallocation per frame instead of once per tile that overflows.
        let wanted = (self.id_to_slot.len() + frame.pending_uploads.len()) as u32;
        if wanted > self.slot_capacity {
            self.grow_atlas(device, queue, wanted);
        }

        let mut alloc_fail = 0u32;
        let mut handoff_encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("headgroup_tile_handoff")
        });
        let mut handoff_copies = 0u32;
        let mut handoff_cpu_fallback = 0u32;
        let mut released_slots: Vec<u32> = Vec::new();
        let production = crate::assemblies::workgroup_new::production_atlas::ProductionAtlas::shared();
        for upload in &frame.pending_uploads {
            let Some(slot) = self.slot_for_id(device, queue, upload.id) else {
                alloc_fail += 1;
                continue;
            };
            if let Some(src_slot) = upload.production_slot {
                // GPU-to-GPU handoff: the workgroup finished this tile in its
                // own atlas; copy it into the headgroup hoard without a readback.
                if let Some(atlas) = &production {
                    let atlas = atlas.lock().expect("production atlas poisoned");
                    atlas.copy_slot_to(
                        &mut handoff_encoder
                        , src_slot
                        , &self.meta_texture
                        , &self.z_texture
                        , slot
                    );
                    handoff_copies += 1;
                    released_slots.push(src_slot);
                } else if !upload.meta.is_empty() {
                    tile_sheet::write_slot(queue, &self.meta_texture, slot, &upload.meta);
                    tile_sheet::write_slot(queue, &self.z_texture, slot, &upload.z);
                    handoff_cpu_fallback += 1;
                } else {
                    alloc_fail += 1;
                }
            } else if !upload.meta.is_empty() {
                tile_sheet::write_slot(queue, &self.meta_texture, slot, &upload.meta);
                tile_sheet::write_slot(queue, &self.z_texture, slot, &upload.z);
            }
        }
        if handoff_copies > 0 {
            queue.submit(Some(handoff_encoder.finish()));
            if let Some(atlas) = &production {
                let mut atlas = atlas.lock().expect("production atlas poisoned");
                for src_slot in released_slots {
                    atlas.release(src_slot);
                }
            }
        }

        let mut entries = frame.tile_entries.clone();
        for (entry, id) in entries.iter_mut().zip(frame.entry_ids.iter()) {
            if let Some(&slot) = self.id_to_slot.get(id) {
                entry.slot = slot;
            } else if let Some(slot) = self.slot_for_id(device, queue, *id) {
                entry.slot = slot;
            } else {
                alloc_fail += 1;
            }
        }
        // Whatever could not be placed is the headgroup's cue to raise the limit:
        // on-screen and lookahead tiles are never evicted for memory.
        self.slots_denied = alloc_fail;

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
    }

    fn paint(&self, render_pass: &mut wgpu::RenderPass<'static>) {
        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_bind_group(0, &self.bind_group, &[]);
        render_pass.draw(0..3, 0..1);
    }
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
mod atlas_tests {
    use super::*;

    fn resources() -> Option<(std::sync::Arc<crate::gpu_context::GpuContext>, GpuDisplayResources)> {
        let shared = crate::gpu_context::GpuContext::shared()?;
        let resources = GpuDisplayResources::new(
            &shared.device
            , wgpu::TextureFormat::Rgba8Unorm
        );
        Some((shared, resources))
    }

    fn upload(id: u64, fill: f32) -> PendingTileUpload {
        let seats = TILE_EDGE_LENGTH * TILE_EDGE_LENGTH;
        PendingTileUpload {
            id
            , meta: vec![[fill, 0.0, 0.0, 1.0]; seats]
            , z: vec![[fill, 0.0, 0.0, 0.0]; seats]
            , production_slot: None
        }
    }

    fn frame(uploads: Vec<PendingTileUpload>) -> ShadeFrame {
        let live_ids: Vec<u64> = uploads.iter().map(|u| u.id).collect();
        ShadeFrame {
            uniforms: ShadeUniforms {
                viewport_size: [8.0, 8.0]
                , seat_offset: [0, 0]
                , zoom_match: 1
                , instruction_count: 0
                , bailout_radius: 2.0
                , bailout_max_extra: 0
                , origin_re: 0.0
                , origin_im: 0.0
                , space: 1.0
                , tile_count: 0
                , grid_w: 1
                , grid_h: 1
                , _pad1: 0
                , _pad2: 0
            }
            , instructions: Vec::new()
            , tile_entries: Vec::new()
            , entry_ids: Vec::new()
            , live_ids
            , tile_grid: Vec::new()
            , pending_uploads: uploads
            , reset_gpu_slots: false
        }
    }

    // r[verify cz.int.memory-bump+1]
    #[test]
    fn the_atlas_grows_rather_than_refusing_a_tile() {
        let Some((shared, mut resources)) = resources() else { return };
        let start = resources.slot_capacity();
        // Ask for one more tile than the atlas currently holds. Requirements put
        // the memory maximum at unlimited, so capacity must follow demand.
        let uploads: Vec<_> = (0..=start as u64).map(|id| upload(id, 1.0)).collect();
        resources.prepare(&shared.device, &shared.queue, &frame(uploads));
        assert!(
            resources.slot_capacity() > start
            , "atlas stayed at {start} slots instead of growing to fit the hoard"
        );
        assert_eq!(
            resources.slots_denied(), 0
            , "no tile may be refused while the atlas can still grow"
        );
    }

    // r[verify cz.hoarding.one-answer-per-point+1]
    #[test]
    fn growing_the_atlas_keeps_the_slot_a_tile_already_holds() {
        let Some((shared, mut resources)) = resources() else { return };
        resources.prepare(&shared.device, &shared.queue, &frame(vec![upload(7, 1.0)]));
        let slot_before = *resources.id_to_slot.get(&7).expect("tile 7 placed");

        let start = resources.slot_capacity();
        let mut uploads: Vec<_> = (100..100 + start as u64).map(|id| upload(id, 0.5)).collect();
        // Keep tile 7 live so growth must carry it, not reclaim it.
        uploads.push(upload(7, 1.0));
        resources.prepare(&shared.device, &shared.queue, &frame(uploads));

        assert!(resources.slot_capacity() > start, "expected the atlas to grow");
        assert_eq!(
            resources.id_to_slot.get(&7).copied()
            , Some(slot_before)
            , "a live tile must keep its slot across a growth, or its data moves out from under the shader"
        );
    }

    #[test]
    fn atlas_bytes_track_capacity() {
        let Some((_shared, resources)) = resources() else { return };
        assert_eq!(
            resources.atlas_bytes()
            , u64::from(resources.slot_capacity()) * GPU_TILE_SLOT_BYTES
        );
    }
    // r[verify cz.seamless.gpu-preferred+1]
    #[test]
    fn a_production_slot_handoff_lands_in_the_headgroup_atlas() {
        let Some((shared, mut resources)) = resources() else { return };
        let Some(production) = crate::assemblies::workgroup_new::production_atlas::ProductionAtlas::shared() else {
            return;
        };
        let src_slot = {
            let mut atlas = production.lock().unwrap();
            let slot = atlas.acquire().expect("production slot");
            let seats = TILE_EDGE_LENGTH * TILE_EDGE_LENGTH;
            atlas.write_slot(
                slot
                , &vec![[9.0, 0.0, 0.0, 1.0]; seats]
                , &vec![[1.0, 2.0, 3.0, 4.0]; seats]
            );
            slot
        };
        let frame = ShadeFrame {
            uniforms: ShadeUniforms {
                viewport_size: [8.0, 8.0]
                , seat_offset: [0, 0]
                , zoom_match: 1
                , instruction_count: 0
                , bailout_radius: 2.0
                , bailout_max_extra: 0
                , origin_re: 0.0
                , origin_im: 0.0
                , space: 1.0
                , tile_count: 0
                , grid_w: 1
                , grid_h: 1
                , _pad1: 0
                , _pad2: 0
            }
            , instructions: Vec::new()
            , tile_entries: Vec::new()
            , entry_ids: Vec::new()
            , live_ids: vec![42]
            , tile_grid: Vec::new()
            , pending_uploads: vec![PendingTileUpload {
                id: 42
                , meta: Vec::new()
                , z: Vec::new()
                , production_slot: Some(src_slot)
            }]
            , reset_gpu_slots: false
        };
        let before = production.lock().unwrap().slots_in_use();
        resources.prepare(&shared.device, &shared.queue, &frame);
        assert!(
            resources.id_to_slot.contains_key(&42)
            , "handoff must allocate a headgroup slot for the tile"
        );
        assert!(
            production.lock().unwrap().slots_in_use() < before
            , "production slot must be released after the headgroup copies it"
        );
        assert_eq!(resources.slots_denied(), 0);
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

#[cfg(test)]
mod b_disp_parity_tests {
    use super::*;
    use crate::settings::{Settings, DEFAULT_COLORING_SCRIPT};

    #[test]
    fn default_script_packs_nonempty_instructions() {
        let mut s = Settings::DEFAULT;
        s.coloring_script = Some(DEFAULT_COLORING_SCRIPT.into());
        let packed = pack_instructions(&mut s);
        assert!(!packed.is_empty());
        assert!(packed.len() >= 3, "default script has ≥3 ops, got {}", packed.len());
    }

    #[test]
    fn default_script_includes_escape_opcode() {
        let mut s = Settings::DEFAULT;
        s.coloring_script = Some(DEFAULT_COLORING_SCRIPT.into());
        let packed = pack_instructions(&mut s);
        assert!(packed.iter().any(|i| i.opcode == 0), "OP_ESCAPE expected");
    }

    #[test]
    fn vsync_fifo_present_for_fps_cap() {
        assert_eq!(
            crate::assemblies::headgroup::window::HEADGROUP_PRESENT_MODE,
            wgpu::PresentMode::Fifo
        );
    }
}
