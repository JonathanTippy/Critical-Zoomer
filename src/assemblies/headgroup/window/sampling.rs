use egui::{Color32, Pos2};
use std::collections::HashMap;

use crate::assemblies::headgroup::window::gpu_display::{
    absolute_tile_origin
    , world_ul_seats
};
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
        // r[impl cz.hoarding.one-answer-per-point+1]
        // r[impl cz.system.max-homotheties+1]
        // Retain across pans; drop absurd mag distance, then enforce ~8-homothety
        // via the shared tile manager. Until TileSession owns multi-mag lookahead,
        // only the current mag is protected as CurrentStencil; nearby mags are
        // HoardedNearFocus (prunable for the cap), not Lookahead.
        let z = self.location.zoom_pot;
        self.tiles.retain(|(tz, _, _), _| {
            let d = z - *tz;
            d >= -31 && d <= 31
        });
        let mut meta = HashMap::new();
        for (key, versions) in &self.tiles {
            let keep = if key.0 == z {
                crate::assemblies::workgroup_new::tile_manager::TileKeepClass::CurrentStencil
            } else if key.0 > z && key.0 <= z + 8 {
                // Deeper mags from the foveated DFS column.
                crate::assemblies::workgroup_new::tile_manager::TileKeepClass::Lookahead
            } else if (key.0 - z).abs() <= 8 {
                crate::assemblies::workgroup_new::tile_manager::TileKeepClass::HoardedNearFocus
            } else {
                crate::assemblies::workgroup_new::tile_manager::TileKeepClass::UnrelatedHoard
            };
            let bytes = versions.len().saturating_mul(4096);
            meta.insert(*key, crate::assemblies::workgroup_new::tile_manager::ManagedTileMeta {
                keep
                , bytes
            });
        }
        let used: usize = meta.values().map(|m| m.bytes).sum();
        let plan = crate::assemblies::workgroup_new::tile_manager::plan_prunes(
            &meta
            , usize::MAX
            , used
        );
        for key in plan {
            self.tiles.remove(&key);
            self.tile_gpu_ids.remove(&key);
        }
        self.tile_gpu_ids.retain(|k, _| self.tiles.contains_key(k));
    }

    pub fn ingest_gpu_tile(&mut self, tile: GPUTile) {
        let zoom = tile.location.zoom_pot;
        // Absolute dyadic origin so a pan to a new screen-UL does not overwrite the
        // prior tile that still covers the same complex region under a shifted view.
        let Some((abs_ox, abs_oy)) = absolute_tile_origin(
            &tile.location
            , tile.origin_seat
        ) else {
            return;
        };
        let key = (zoom, abs_ox, abs_oy);
        let new_filled = tile.data.iter().filter(|c| c.is_some()).count();
        let slot = self.tiles.entry(key).or_default();
        let ids = self.tile_gpu_ids.entry(key).or_default();
        if let Some(existing) = slot.first() {
            let old_filled = existing.data.iter().filter(|c| c.is_some()).count();
            if new_filled < old_filled {
                // Never replace a fuller answer with a sparser one (session rebuild noise).
                return;
            }
        }
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
        let Some((cx, cy)) = world_ul_seats(&self.location) else {
            return None;
        };
        let world = (seat.0.wrapping_add(cx), seat.1.wrapping_add(cy));
        let mut best: Option<(i32, Answer)> = None;
        for ((z, ox, oy), versions) in &self.tiles {
            if *z != zoom {
                continue;
            }
            if world.0 < *ox || world.1 < *oy
                || world.0 >= *ox + edge || world.1 >= *oy + edge
            {
                continue;
            }
            let local = ((world.0 - *ox) as usize, (world.1 - *oy) as usize);
            let dist = ox.wrapping_sub(cx).abs().saturating_add(oy.wrapping_sub(cy).abs());
            for tile in versions {
                if let Some(a) = tile.get(local) {
                    match best {
                        Some((best_dist, _)) if best_dist <= dist => {}
                        _ => { best = Some((dist, Answer::from(a))); }
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
        let Some((cx, cy)) = world_ul_seats(&self.location) else {
            return None;
        };
        let world_fine = (seat.0.wrapping_add(cx), seat.1.wrapping_add(cy));
        let source_seat = (world_fine.0 >> mag_delta, world_fine.1 >> mag_delta);
        let mut best: Option<(i32, Answer)> = None;
        for ((z, ox, oy), versions) in &self.tiles {
            if *z != source_zoom {
                continue;
            }
            if source_seat.0 < *ox || source_seat.1 < *oy
                || source_seat.0 >= *ox + edge || source_seat.1 >= *oy + edge
            {
                continue;
            }
            let local = (
                (source_seat.0 - *ox) as usize
                , (source_seat.1 - *oy) as usize
            );
            let dist = ((*ox as i64) << mag_delta as u32)
                .saturating_sub(cx as i64)
                .abs()
                .saturating_add(
                    ((*oy as i64) << mag_delta as u32)
                        .saturating_sub(cy as i64)
                        .abs()
                );
            let dist = dist.min(i32::MAX as i64) as i32;
            for tile in versions {
                if let Some(a) = tile.get(local) {
                    match best {
                        Some((best_dist, _)) if best_dist <= dist => {}
                        _ => { best = Some((dist, Answer::from(a))); }
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

#[cfg(test)]
mod hoard_tests {
    use super::*;

    // r[verify cz.hoarding.one-answer-per-point+1]
    #[test]
    fn prune_keeps_current_and_near_zoom_keys() {
        let mut ctx = SamplingContext {
            tiles: HashMap::new(),
            tile_gpu_ids: HashMap::new(),
            pending_tile_uploads: Vec::new(),
            next_tile_gpu_id: 0,
            reset_gpu_tile_slots: false,
            color_screen: None,
            proximate_answers: true,
            unsent_answers: true,
            screen_size: (800, 480),
            location: ObjectivePosAndZoom {
                pos: (IntExp::ZERO, IntExp::ZERO),
                zoom_pot: 5,
            },
            updated: false,
            mouse_drag_start: None,
        };
        ctx.tiles.insert((5, 0, 0), Vec::new());
        ctx.tiles.insert((4, 0, 0), Vec::new());
        ctx.tiles.insert((-40, 0, 0), Vec::new());
        ctx.prune_distant_tiles();
        assert!(ctx.tiles.contains_key(&(5, 0, 0)));
        assert!(ctx.tiles.contains_key(&(4, 0, 0)));
        assert!(!ctx.tiles.contains_key(&(-40, 0, 0)));
    }

    #[test]
    fn clear_tiles_empties_hoard() {
        let mut ctx = SamplingContext {
            tiles: HashMap::new(),
            tile_gpu_ids: HashMap::new(),
            pending_tile_uploads: Vec::new(),
            next_tile_gpu_id: 0,
            reset_gpu_tile_slots: false,
            color_screen: None,
            proximate_answers: true,
            unsent_answers: true,
            screen_size: (800, 480),
            location: ObjectivePosAndZoom {
                pos: (IntExp::ZERO, IntExp::ZERO),
                zoom_pot: 0,
            },
            updated: false,
            mouse_drag_start: None,
        };
        ctx.tiles.insert((0, 0, 0), Vec::new());
        ctx.clear_tiles();
        assert_eq!(ctx.tile_count(), 0);
        assert!(ctx.reset_gpu_tile_slots);
    }

    // r[verify cz.system.max-homotheties+1]
    #[test]
    fn prune_distant_enforces_homothety_budget() {
        let mut ctx = SamplingContext {
            tiles: HashMap::new(),
            tile_gpu_ids: HashMap::new(),
            pending_tile_uploads: Vec::new(),
            next_tile_gpu_id: 0,
            reset_gpu_tile_slots: false,
            color_screen: None,
            proximate_answers: true,
            unsent_answers: true,
            screen_size: (800, 480),
            location: ObjectivePosAndZoom {
                pos: (IntExp::ZERO, IntExp::ZERO),
                zoom_pot: 20,
            },
            updated: false,
            mouse_drag_start: None,
        };
        ctx.tiles.insert((20, 0, 0), Vec::new());
        for mag in 0..15 {
            ctx.tiles.insert((mag, 0, 0), Vec::new());
        }
        ctx.prune_distant_tiles();
        let mags: std::collections::HashSet<i32> = ctx.tiles.keys().map(|k| k.0).collect();
        assert!(
            mags.len() <= crate::assemblies::workgroup_new::tile_manager::MAX_HOMOTHETIES
            , "live prune must enforce MAX_HOMOTHETIES, got {}"
            , mags.len()
        );
        assert!(mags.contains(&20), "current mag must survive");
    }

    // r[verify cz.hoarding.one-answer-per-point+1]
    #[test]
    fn pan_ingest_keeps_prior_absolute_tile_key() {
        let loc0 = ObjectivePosAndZoom {
            pos: (IntExp::ZERO, IntExp::ZERO)
            , zoom_pot: 2
        };
        let loc1 = ObjectivePosAndZoom {
            pos: (
                IntExp::from(1).shift(-(2 + PIXELS_PER_UNIT_POT))
                , IntExp::ZERO
            )
            , zoom_pot: 2
        };
        let mut tile = Tile::new((0, 0), 2);
        tile.set((0, 0), NORES_ANSWER);
        let gpu0 = GPUTile::from_answer_tile(&tile, (64, 64), loc0.clone());
        let gpu1 = GPUTile::from_answer_tile(&tile, (64, 64), loc1.clone());
        let mut ctx = SamplingContext {
            tiles: HashMap::new()
            , tile_gpu_ids: HashMap::new()
            , pending_tile_uploads: Vec::new()
            , next_tile_gpu_id: 0
            , reset_gpu_tile_slots: false
            , color_screen: None
            , proximate_answers: true
            , unsent_answers: true
            , screen_size: (64, 64)
            , location: loc0.clone()
            , updated: false
            , mouse_drag_start: None
        };
        ctx.ingest_gpu_tile(gpu0);
        ctx.location = loc1;
        ctx.ingest_gpu_tile(gpu1);
        assert_eq!(ctx.tile_count(), 2, "pan must not clobber the prior absolute tile key");
    }

    // r[verify cz.hoarding.one-answer-per-point+1]
    #[test]
    fn ingest_rejects_sparser_replacement_at_same_key() {
        let loc = ObjectivePosAndZoom {
            pos: (IntExp::ZERO, IntExp::ZERO)
            , zoom_pot: 0
        };
        let mut full = Tile::new((0, 0), 0);
        full.set((0, 0), NORES_ANSWER);
        full.set((1, 0), NORES_ANSWER);
        let mut sparse = Tile::new((0, 0), 0);
        sparse.set((0, 0), NORES_ANSWER);
        let mut ctx = SamplingContext {
            tiles: HashMap::new()
            , tile_gpu_ids: HashMap::new()
            , pending_tile_uploads: Vec::new()
            , next_tile_gpu_id: 0
            , reset_gpu_tile_slots: false
            , color_screen: None
            , proximate_answers: true
            , unsent_answers: true
            , screen_size: (64, 64)
            , location: loc.clone()
            , updated: false
            , mouse_drag_start: None
        };
        ctx.ingest_gpu_tile(GPUTile::from_answer_tile(&full, (64, 64), loc.clone()));
        ctx.ingest_gpu_tile(GPUTile::from_answer_tile(&sparse, (64, 64), loc));
        let filled = ctx.tiles.values().next().unwrap()[0]
            .data
            .iter()
            .filter(|c| c.is_some())
            .count();
        assert_eq!(filled, 2, "sparser tile must not replace a fuller answer");
    }

    // r[verify cz.hoarding.one-answer-per-point+1]
    #[test]
    fn absolute_origin_differs_across_one_pixel_pan() {
        let loc0 = ObjectivePosAndZoom {
            pos: (IntExp::ZERO, IntExp::ZERO)
            , zoom_pot: 3
        };
        let loc1 = ObjectivePosAndZoom {
            pos: (
                IntExp::from(1).shift(-(3 + PIXELS_PER_UNIT_POT))
                , IntExp::ZERO
            )
            , zoom_pot: 3
        };
        let a0 = absolute_tile_origin(&loc0, (0, 0)).unwrap();
        let a1 = absolute_tile_origin(&loc1, (0, 0)).unwrap();
        assert_ne!(a0, a1);
        assert_eq!(a1.0 - a0.0, 1);
    }
}
