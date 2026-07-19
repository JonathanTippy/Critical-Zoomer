use egui::{Color32, Pos2};
use std::collections::HashMap;

use crate::assemblies::headgroup::window::gpu_display::{pos_delta_pixels, seat_delta_pixels};
use crate::assemblies::structs::*;
use crate::constants::*;
use crate::intexp::*;
use crate::utils::*;

pub enum ZoomerCommand {
    SetFocus { pixel_x: u32, pixel_y: u32 }
    ,
    SetZoom { pot: i32 }
    ,
    Zoom { pot: i32, center_screenspace_pos: (i32, i32) }
    ,
    Move { pixels_x: IntExp, pixels_y: IntExp }
    ,
    MoveTo { x: IntExp, y: IntExp }
    ,
    SetPos { real: IntExp, imag: IntExp }
    ,
    NavigateTo { real: IntExp, imag: IntExp, pot: i32 }
    ,
    TrackPoint { point_id: u64, point_real: IntExp, point_imag: IntExp }
    ,
    UntrackPoint { point_id: u64 }
    ,
    UntrackAllPoints
}
pub const NUMBER_OF_COMMANDS: u16 = 10;

#[derive(Clone, Debug)]
pub struct SamplingContext {
    pub tiles: HashMap<(i32, i32, i32), Vec<GPUTile>>
    , pub tile_gpu_ids: HashMap<(i32, i32, i32), Vec<u64>>
    , pub pending_tile_uploads: Vec<crate::assemblies::headgroup::window::gpu_display::PendingTileUpload>
    , pub next_tile_gpu_id: u64
    , pub reset_gpu_tile_slots: bool
    , pub color_screen: Option<View<Color32>>
    , pub proximate_answers: bool
    , pub unsent_answers: bool
    , pub screen_size: (u32, u32)
    , pub location: ObjectivePosAndZoom
    , pub updated: bool
    , pub mouse_drag_start: Option<(ObjectivePosAndZoom, Pos2)>
}

#[derive(Clone, Debug, PartialEq)]
pub struct ViewportLocation {
    pub pos: (i32, i32)
    , pub zoom_pot: i32
    , pub counter: u64
}

pub fn floor_div_tile(v: i32) -> i32 {
    let edge = TILE_EDGE_LENGTH as i32;
    let d = v / edge;
    let r = v % edge;
    if r != 0 && v < 0 {
        d - 1
    } else {
        d
    }
}

impl SamplingContext {
    pub fn clear_tiles(&mut self) {
        self.tiles.clear();
        self.tile_gpu_ids.clear();
        self.pending_tile_uploads.clear();
        self.reset_gpu_tile_slots = true;
        self.color_screen = None;
        self.proximate_answers = true;
        self.unsent_answers = true;
    }

    pub fn prune_distant_tiles(&mut self) {
        let z = self.location.zoom_pot;
        self.tiles.retain(|(tz, _, _), _| {
            let d = z - *tz;
            d >= -31 && d <= 31
        });
        self.tile_gpu_ids.retain(|k, _| self.tiles.contains_key(k));
    }

    pub fn ingest_gpu_tile(&mut self, tile: GPUTile) {
        let zoom = tile.location.zoom_pot;
        let key = (
            zoom
            , tile.origin_seat.0 as i32
            , tile.origin_seat.1 as i32
        );
        let slot = self.tiles.entry(key).or_default();
        let ids = self.tile_gpu_ids.entry(key).or_default();
        let gpu_id = if slot.is_empty() {
            let id = self.next_tile_gpu_id;
            self.next_tile_gpu_id += 1;
            slot.push(tile.clone());
            ids.clear();
            ids.push(id);
            id
        } else {
            slot.truncate(1);
            ids.truncate(1);
            slot[0] = tile.clone();
            if ids.is_empty() {
                let id = self.next_tile_gpu_id;
                self.next_tile_gpu_id += 1;
                ids.push(id);
                id
            } else {
                ids[0]
            }
        };
        self.pending_tile_uploads.push(
            crate::assemblies::headgroup::window::gpu_display::pack_tile_upload(&tile, gpu_id)
        );
        self.unsent_answers = true;
        // #region agent log
        let hoard = self.tile_count();
        if hoard < 8 || hoard % 16 == 0 {
            crate::assemblies::headgroup::window::agent_dbg(
                "H-PAN-D"
                , "sampling.rs:ingest"
                , "ingest_tile"
                , &format!(
                    "{{\"key\":[{},{},{}],\"view_zoom\":{},\"hoard\":{},\"gpu_id\":{}}}"
                    , key.0, key.1, key.2
                    , self.location.zoom_pot
                    , hoard
                    , gpu_id
                )
            );
        }
        // #endregion
    }

    pub fn lookup_answer_viewport(&self, seat: (usize, usize)) -> Answer {
        let seat_i = (seat.0 as i32, seat.1 as i32);
        if let Some(answer) = self.lookup_at_zoom(seat_i, self.location.zoom_pot) {
            return answer;
        }
        let mut mag = self.location.zoom_pot - 1;
        let min_mag = self.location.zoom_pot - 16;
        while mag >= min_mag {
            if let Some(answer) = self.lookup_lesser(seat_i, mag) {
                return answer;
            }
            mag -= 1;
        }
        NORES_ANSWER
    }

    pub fn lookup_at_zoom(&self, seat: (i32, i32), zoom: i32) -> Option<Answer> {
        let edge = TILE_EDGE_LENGTH as i32;
        let mut best: Option<(i32, Answer)> = None;
        for ((z, ox, oy), versions) in &self.tiles {
            if *z != zoom {
                continue;
            }
            for tile in versions {
                let Some((dx, dy)) = seat_delta_pixels(&tile.location, &self.location) else {
                    continue;
                };
                let ax = seat.0 + dx;
                let ay = seat.1 + dy;
                if ax >= *ox && ay >= *oy && ax < *ox + edge && ay < *oy + edge {
                    let local = ((ax - *ox) as usize, (ay - *oy) as usize);
                    if let Some(a) = tile.get(local) {
                        let dist = dx.abs().saturating_add(dy.abs());
                        match best {
                            Some((best_dist, _)) if best_dist <= dist => {}
                            _ => { best = Some((dist, Answer::from(a))); }
                        }
                    }
                }
            }
        }
        best.map(|(_, a)| a)
    }

    pub fn lookup_lesser(&self, seat: (i32, i32), source_zoom: i32) -> Option<Answer> {
        let mag_delta = self.location.zoom_pot - source_zoom;
        if mag_delta <= 0 {
            return None;
        }
        let edge = TILE_EDGE_LENGTH as i32;
        let mut best: Option<(i32, Answer)> = None;
        for ((z, ox, oy), versions) in &self.tiles {
            if *z != source_zoom {
                continue;
            }
            for tile in versions {
                let Some((dx_fine, dy_fine)) = pos_delta_pixels(
                    &tile.location.pos
                    , &self.location.pos
                    , self.location.zoom_pot
                ) else {
                    continue;
                };
                let Some((dx_coarse, dy_coarse)) = pos_delta_pixels(
                    &tile.location.pos
                    , &self.location.pos
                    , source_zoom
                ) else {
                    continue;
                };
                let source_seat = (
                    (seat.0.wrapping_add(dx_fine)) >> mag_delta
                    , (seat.1.wrapping_add(dy_fine)) >> mag_delta
                );
                // #region agent log
                if seat == (
                    self.screen_size.0 as i32 / 2
                    , self.screen_size.1 as i32 / 2
                ) {
                    let shifted = (
                        dx_coarse.wrapping_shl(mag_delta as u32)
                        , dy_coarse.wrapping_shl(mag_delta as u32)
                    );
                    crate::assemblies::headgroup::window::agent_dbg(
                        "H-ZOOM-A"
                        , "sampling.rs:lookup_lesser"
                        , "lesser_map"
                        , &format!(
                            "{{\"seat\":[{},{}],\"source_zoom\":{},\"mag_delta\":{},\"d_coarse\":[{},{}],\"d_fine\":[{},{}],\"d_shifted\":[{},{}],\"source_seat\":[{},{}],\"origin\":[{},{}]}}"
                            , seat.0, seat.1
                            , source_zoom
                            , mag_delta
                            , dx_coarse, dy_coarse
                            , dx_fine, dy_fine
                            , shifted.0, shifted.1
                            , source_seat.0, source_seat.1
                            , ox, oy
                        )
                    );
                }
                // #endregion
                if source_seat.0 >= *ox && source_seat.1 >= *oy
                    && source_seat.0 < *ox + edge && source_seat.1 < *oy + edge
                {
                    let local = (
                        (source_seat.0 - *ox) as usize
                        , (source_seat.1 - *oy) as usize
                    );
                    if let Some(a) = tile.get(local) {
                        let dist = dx_fine.abs().saturating_add(dy_fine.abs());
                        match best {
                            Some((best_dist, _)) if best_dist <= dist => {}
                            _ => { best = Some((dist, Answer::from(a))); }
                        }
                    }
                }
            }
        }
        best.map(|(_, a)| a)
    }

    pub fn tile_count(&self) -> usize {
        self.tiles.values().map(|v| v.len()).sum()
    }
}
