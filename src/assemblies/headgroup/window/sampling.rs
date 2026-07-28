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
    // Soft CPU/VRAM budget for the tile hoard (bytes). Bumps when protected tiles exceed it.
    , pub memory_limit_bytes: usize
    // Last limit bump request (protected-byte total), if any, from prune/manager.
    , pub last_memory_bump: Option<usize>
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

    // r[impl cz.int.memory-bump+1]
    pub fn prune_distant_tiles(&mut self) {
        use crate::assemblies::workgroup_new::tile_manager::{
            apply_memory_bump, plan_prunes, required_limit_bump, ManagedTileMeta, TileKeepClass,
        };

        let z = self.location.zoom_pot;
        self.tiles.retain(|(tz, _, _), _| {
            let d = z - *tz;
            d >= -31 && d <= 31
        });
        self.tile_gpu_ids.retain(|k, _| self.tiles.contains_key(k));

        let mut meta = HashMap::new();
        let mut used_bytes = 0usize;
        for (key, versions) in &self.tiles {
            let bytes = versions
                .iter()
                .map(|t| std::mem::size_of_val(&t.data).max(TILE_SEAT_COUNT))
                .sum::<usize>()
                .max(1);
            let mag_delta = (key.0 - z).abs();
            let keep = if key.0 == z {
                TileKeepClass::CurrentStencil
            } else if mag_delta == 1 {
                TileKeepClass::Lookahead
            } else if mag_delta <= 8 {
                TileKeepClass::HoardedNearFocus
            } else {
                TileKeepClass::UnrelatedHoard
            };
            meta.insert(*key, ManagedTileMeta { keep, bytes });
            used_bytes = used_bytes.saturating_add(bytes);
        }

        if let Some(needed) = required_limit_bump(&meta, self.memory_limit_bytes) {
            self.last_memory_bump = Some(needed);
            self.memory_limit_bytes = apply_memory_bump(self.memory_limit_bytes, needed);
        }

        for key in plan_prunes(&meta, self.memory_limit_bytes, used_bytes) {
            self.tiles.remove(&key);
            self.tile_gpu_ids.remove(&key);
        }
    }

    // r[impl cz.int.hoard-ingest-sample+1]
    // r[impl cz.hoarding.one-answer-per-point+1]
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
            let existing_filled = slot[0].data.iter().filter(|c| c.is_some()).count();
            let incoming_filled = tile.data.iter().filter(|c| c.is_some()).count();
            // r[impl cz.hoarding.one-answer-per-point+1]
            if incoming_filled < existing_filled {
                return;
            }
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
        if std::env::var("CZ_DEBUG_FILL")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false)
        {
            let mut filled = 0u32;
            let mut nores = 0u32;
            for ly in 0..crate::constants::TILE_EDGE_LENGTH {
                for lx in 0..crate::constants::TILE_EDGE_LENGTH {
                    let Some(a) = tile.get((lx, ly)) else { continue };
                    filled += 1;
                    let answer: crate::assemblies::structs::Answer = a.into();
                    if let crate::assemblies::structs::MandelbrotResult::Outside {
                        escape_time_r2, ..
                    } = answer.result
                    {
                        if escape_time_r2 == 1 && answer.min_magnitude.is_infinite() {
                            nores += 1;
                        }
                    }
                }
            }
            let _ = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open("/tmp/cz_fill_debug.log")
                .and_then(|mut file| {
                    use std::io::Write;
                    writeln!(
                        file
                        , "ingest key=[{},{},{}] gpu_id={} filled={} nores={} hoard={}"
                        , key.0, key.1, key.2, gpu_id, filled, nores, self.tile_count()
                    )
                });
        }
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
        // Finer before lesser when overlapping (dyadic: finer wins).
        let mut mag = self.location.zoom_pot + 1;
        let max_mag = self.location.zoom_pot + 16;
        while mag <= max_mag {
            if let Some(answer) = self.lookup_finer(seat_i, mag) {
                return answer;
            }
            mag += 1;
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

    /// Sample a finer (higher magnification) tile into the viewport seat.
    /// Finer seat = viewport seat left-shifted by mag delta (top-left of the finer neighborhood).
    pub fn lookup_finer(&self, seat: (i32, i32), source_zoom: i32) -> Option<Answer> {
        let mag_delta = source_zoom - self.location.zoom_pot;
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
                let source_seat = (
                    (seat.0.wrapping_add(dx_fine)).wrapping_shl(mag_delta as u32)
                    , (seat.1.wrapping_add(dy_fine)).wrapping_shl(mag_delta as u32)
                );
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

#[cfg(test)]
mod hoard_tests {
    use super::*;
    use crate::assemblies::structs::{GPUTile, MandelbrotResult, Tile};
    use crate::constants::{NORES_ANSWER, TILE_EDGE_LENGTH};
    use crate::intexp::IntExp;
    use std::collections::HashMap;

    fn empty_ctx(zoom: i32) -> SamplingContext {
        SamplingContext {
            tiles: HashMap::new(),
            tile_gpu_ids: HashMap::new(),
            pending_tile_uploads: Vec::new(),
            next_tile_gpu_id: 1,
            reset_gpu_tile_slots: false,
            color_screen: None,
            proximate_answers: true,
            unsent_answers: true,
            screen_size: (64, 64),
            location: ObjectivePosAndZoom {
                pos: (IntExp::ZERO, IntExp::ZERO),
                zoom_pot: zoom,
            },
            updated: false,
            mouse_drag_start: None,
            memory_limit_bytes: 1_000_000_000,
            last_memory_bump: None,
        }
    }

    // r[verify cz.hoarding.one-answer-per-point+1]
    #[test]
    fn ingest_keeps_single_tile_per_absolute_key() {
        let mut ctx = empty_ctx(0);
        let mut a = Tile::new((0, 0), 0);
        a.set((0, 0), NORES_ANSWER);
        let loc = ObjectivePosAndZoom {
            pos: (IntExp::ZERO, IntExp::ZERO),
            zoom_pot: 0,
        };
        ctx.ingest_gpu_tile(GPUTile::from_answer_tile(&a, (64, 64), loc.clone()));
        ctx.ingest_gpu_tile(GPUTile::from_answer_tile(&a, (64, 64), loc));
        assert_eq!(ctx.tiles.len(), 1);
        assert_eq!(ctx.tiles.values().next().unwrap().len(), 1);
    }

    // r[verify cz.hoarding.one-answer-per-point+1]
    // r[verify cz.int.hoard-ingest-sample+1]
    #[test]
    fn pan_ingest_keeps_distinct_absolute_keys() {
        let mut ctx = empty_ctx(0);
        let mut tile = Tile::new((0, 0), 0);
        tile.set((0, 0), NORES_ANSWER);
        let loc0 = ObjectivePosAndZoom {
            pos: (IntExp::ZERO, IntExp::ZERO),
            zoom_pot: 0,
        };
        let loc1 = ObjectivePosAndZoom {
            pos: (IntExp::from(1).shift(-(PIXELS_PER_UNIT_POT)), IntExp::ZERO),
            zoom_pot: 0,
        };
        ctx.ingest_gpu_tile(GPUTile::from_answer_tile(&tile, (64, 64), loc0));
        let before = ctx.tiles.len();
        ctx.ingest_gpu_tile(GPUTile::from_answer_tile(&tile, (64, 64), loc1));
        assert!(ctx.tiles.len() >= before);
    }

    // r[verify cz.int.hoard-ingest-sample+1]
    #[test]
    fn nores_ingest_stays_outside() {
        let mut ctx = empty_ctx(0);
        let mut tile = Tile::new((0, 0), 0);
        tile.set((0, 0), NORES_ANSWER);
        let loc = ObjectivePosAndZoom {
            pos: (IntExp::ZERO, IntExp::ZERO),
            zoom_pot: 0,
        };
        ctx.ingest_gpu_tile(GPUTile::from_answer_tile(&tile, (64, 64), loc));
        let stored = ctx.tiles.values().next().unwrap()[0].get((0, 0)).unwrap();
        match Answer::from(stored).result {
            MandelbrotResult::Outside { .. } => {}
            MandelbrotResult::Inside { .. } => panic!("NORES must stay Outside after ingest"),
        }
    }

    fn outside_answer(escape_time: u64) -> Answer {
        Answer {
            result: MandelbrotResult::Outside {
                escape_time_r2: escape_time,
                escape_z: (2.0, 0.0),
            },
            min_magnitude_time: 0,
            min_magnitude: 4.0,
        }
    }

    // Overlapping same / coarser / finer: finer must win when same is absent.
    #[test]
    fn overlapping_prefers_finer_before_lesser() {
        let mut ctx = empty_ctx(0);
        let loc0 = ObjectivePosAndZoom {
            pos: (IntExp::ZERO, IntExp::ZERO),
            zoom_pot: 0,
        };
        // Coarser tile at mag -1 covering the origin neighborhood.
        let mut coarse = Tile::new((0, 0), -1);
        for y in 0..TILE_EDGE_LENGTH {
            for x in 0..TILE_EDGE_LENGTH {
                coarse.set((x, y), outside_answer(11));
            }
        }
        let loc_coarse = ObjectivePosAndZoom {
            pos: (IntExp::ZERO, IntExp::ZERO),
            zoom_pot: -1,
        };
        ctx.ingest_gpu_tile(GPUTile::from_answer_tile(&coarse, (64, 64), loc_coarse));

        // Finer tile at mag +1; only the top-left finer seat that maps to viewport (0,0).
        let mut fine = Tile::new((0, 0), 1);
        for y in 0..TILE_EDGE_LENGTH {
            for x in 0..TILE_EDGE_LENGTH {
                fine.set((x, y), outside_answer(77));
            }
        }
        let loc_fine = ObjectivePosAndZoom {
            pos: (IntExp::ZERO, IntExp::ZERO),
            zoom_pot: 1,
        };
        ctx.ingest_gpu_tile(GPUTile::from_answer_tile(&fine, (64, 64), loc_fine));

        let got = ctx.lookup_answer_viewport((0, 0));
        match got.result {
            MandelbrotResult::Outside { escape_time_r2, .. } => {
                assert_eq!(
                    escape_time_r2, 77,
                    "finer must win over coarser when same-mag is missing"
                );
            }
            MandelbrotResult::Inside { .. } => panic!("expected Outside from finer tile"),
        }

        // Same-mag still beats finer when present.
        let mut same = Tile::new((0, 0), 0);
        for y in 0..TILE_EDGE_LENGTH {
            for x in 0..TILE_EDGE_LENGTH {
                same.set((x, y), outside_answer(33));
            }
        }
        ctx.ingest_gpu_tile(GPUTile::from_answer_tile(&same, (64, 64), loc0));
        let got_same = ctx.lookup_answer_viewport((0, 0));
        match got_same.result {
            MandelbrotResult::Outside { escape_time_r2, .. } => {
                assert_eq!(escape_time_r2, 33, "same-mag must beat finer");
            }
            MandelbrotResult::Inside { .. } => panic!("expected Outside from same-mag tile"),
        }
    }
}
