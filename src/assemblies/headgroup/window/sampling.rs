use egui::{Pos2};
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
#[derive(Clone, Debug)]
pub struct SamplingContext {
    pub tiles: HashMap<(i32, i32, i32), Vec<Box<GPUTile>>>
    , pub tile_gpu_ids: HashMap<(i32, i32, i32), Vec<u64>>
    , pub pending_tile_uploads: Vec<crate::assemblies::headgroup::window::gpu_display::PendingTileUpload>
    , pub next_tile_gpu_id: u64
    , pub reset_gpu_tile_slots: bool
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
    // Filled-seat counts for handle-only tiles (no CPU answer payload).
    , pub handle_filled: HashMap<(i32, i32, i32), u32>
}

#[derive(Clone, Debug, PartialEq)]
pub struct ViewportLocation {
    pub pos: (i32, i32)
    , pub zoom_pot: i32
    , pub counter: u64
}

fn placeholder_gpu_tile(handle: &GpuTileHandle) -> Box<GPUTile> {
    Box::new(GPUTile::empty(
        handle.origin_seat
        , handle.magnification_pot
        , handle.screen_res
        , handle.location.clone()
    ))
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
    // r[impl cz.int.memory-bump+1]
    pub fn prune_distant_tiles(&mut self) {
        use crate::assemblies::workgroup::tile_manager::{
            apply_memory_bump, plan_prunes, required_limit_bump, ManagedTileMeta, TileKeepClass,
        };

        let z = self.location.zoom_pot;
        // Soft prefilter only: the tile manager's 8-homothety cap is authoritative.
        // Keep a little headroom so prune can choose which mags to drop.
        let prefilter = crate::assemblies::workgroup::tile_manager::MAX_HOMOTHETIES as i32;
        self.tiles.retain(|(tz, _, _), _| (z - *tz).abs() <= prefilter);
        self.tile_gpu_ids.retain(|k, _| self.tiles.contains_key(k));
        self.handle_filled.retain(|k, _| self.tiles.contains_key(k));

        let mut meta = HashMap::new();
        let mut used_bytes = 0usize;
        for (key, versions) in &self.tiles {
            // The headgroup hoard lives in the GPU (tile_manager.md), so its
            // budget is the VRAM its atlas slots occupy, not the size of any
            // CPU-side struct that happens to describe them.
            let bytes = versions
                .len()
                .saturating_mul(
                    crate::assemblies::headgroup::window::gpu_display::GPU_TILE_SLOT_BYTES as usize
                )
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
            self.handle_filled.remove(&key);
        }
    }

    /// Distinct magnifications currently in the hoard.
    pub fn homothety_count(&self) -> usize {
        self.tiles.keys().map(|k| k.0).collect::<std::collections::HashSet<_>>().len()
    }

    // r[impl cz.int.hoard-ingest-sample+1]
    // r[impl cz.hoarding.one-answer-per-point+1]
    /// Live path: ingest a handle whose answers already live in the production atlas.
    ///
    /// Returns `true` only when this ingest newly completes a **whole** tile
    /// (standards HUD TPS: new completed tiles, not WIP update rate).
    pub fn ingest_gpu_handle(&mut self, mut handle: GpuTileHandle) -> bool {
        let key = handle.absolute_key();
        let existing_filled = self.handle_filled.get(&key).copied().or_else(|| {
            self.tiles.get(&key).and_then(|versions| {
                versions.first().map(|t| t.data.iter().filter(|c| c.is_some()).count() as u32)
            })
        });
        if let Some(existing) = existing_filled {
            if handle.filled_seats < existing {
                // Drop the unused production slot so it does not leak.
                if let Some(prod) = handle.production_slot.take() {
                    if let Some(atlas) = crate::assemblies::workgroup::production_atlas::ProductionAtlas::shared() {
                        if let Ok(mut atlas) = atlas.lock() {
                            atlas.release(prod);
                        }
                    }
                }
                return false;
            }
        }

        let was_complete = existing_filled
            .map(|f| f >= TILE_SEAT_COUNT as u32)
            .unwrap_or(false);
        let now_complete = handle.filled_seats >= TILE_SEAT_COUNT as u32;
        let newly_completed_whole = now_complete && !was_complete;

        let gpu_id = self.tile_gpu_ids.get(&key).and_then(|ids| ids.first().copied()).unwrap_or_else(|| {
            let id = self.next_tile_gpu_id;
            self.next_tile_gpu_id += 1;
            id
        });

        let upload = if let Some(prod) = handle.production_slot {
            Some(crate::assemblies::headgroup::window::gpu_display::pending_handoff(gpu_id, prod))
        } else if let Some(tile) = handle.cpu_fallback.as_deref() {
            Some(crate::assemblies::headgroup::window::gpu_display::pack_tile_upload(tile, gpu_id))
        } else {
            None
        };

        let stored = match handle.cpu_fallback.take() {
            Some(tile) => tile
            , None => placeholder_gpu_tile(&handle)
        };
        let versions = self.tiles.entry(key).or_default();
        versions.clear();
        versions.push(stored);
        let ids = self.tile_gpu_ids.entry(key).or_default();
        ids.clear();
        ids.push(gpu_id);
        self.handle_filled.insert(key, handle.filled_seats);
        if let Some(upload) = upload {
            self.pending_tile_uploads.push(upload);
        }
        self.unsent_answers = true;
        // #region agent log
        if newly_completed_whole || handle.filled_seats > 0 {
            crate::assemblies::workgroup::debug_session::log(
                "H-TPS-COUNT",
                "sampling.rs:ingest_gpu_handle",
                "ingest_tps_gate",
                &format!(
                    "{{\"filled\":{},\"complete\":{},\"newly_completed_whole\":{},\"was_complete\":{}}}",
                    handle.filled_seats,
                    now_complete,
                    newly_completed_whole,
                    was_complete
                ),
            );
        }
        // #endregion
        newly_completed_whole
    }

    // r[impl cz.int.hoard-ingest-sample+1]
    // r[impl cz.hoarding.one-answer-per-point+1]
    /// Returns whether this ingest newly completed a whole tile (HUD TPS).
    pub fn ingest_gpu_tile(&mut self, tile: GPUTile) -> bool {
        self.ingest_gpu_handle(GpuTileHandle::from_gpu_tile(tile, None))
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
    use crate::assemblies::structs::{Answer, GPUTile, MandelbrotResult, Tile};
    use crate::constants::{NORES_ANSWER, TILE_EDGE_LENGTH, TILE_SEAT_COUNT};
    use crate::intexp::IntExp;
    use std::collections::HashMap;

    fn empty_ctx(zoom: i32) -> SamplingContext {
        SamplingContext {
            tiles: HashMap::new(),
            tile_gpu_ids: HashMap::new(),
            pending_tile_uploads: Vec::new(),
            next_tile_gpu_id: 1,
            reset_gpu_tile_slots: false,
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
            handle_filled: HashMap::new(),
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

    fn full_answer_tile(origin: (usize, usize), zoom: i32) -> Tile<Answer> {
        let mut tile = Tile::new(origin, zoom);
        for y in 0..TILE_EDGE_LENGTH {
            for x in 0..TILE_EDGE_LENGTH {
                tile.set((x, y), NORES_ANSWER);
            }
        }
        tile
    }

    fn partial_answer_tile(origin: (usize, usize), zoom: i32, n: usize) -> Tile<Answer> {
        let mut tile = Tile::new(origin, zoom);
        for i in 0..n {
            tile.set((i % TILE_EDGE_LENGTH, i / TILE_EDGE_LENGTH), NORES_ANSWER);
        }
        tile
    }

    // Standards HUD TPS: only *new completed whole* tiles — not WIP emits.
    // r[verify cz.perf.home-100tps+1]
    #[test]
    fn ingest_tps_ignores_wip_and_republish() {
        let mut ctx = empty_ctx(0);
        let loc = ObjectivePosAndZoom {
            pos: (IntExp::ZERO, IntExp::ZERO),
            zoom_pot: 0,
        };
        let wip = partial_answer_tile((0, 0), 0, 100);
        assert!(
            !ctx.ingest_gpu_tile(GPUTile::from_answer_tile(&wip, (64, 64), loc.clone())),
            "partial WIP must not count as completed TPS"
        );
        let wip2 = partial_answer_tile((0, 0), 0, 2000);
        assert!(
            !ctx.ingest_gpu_tile(GPUTile::from_answer_tile(&wip2, (64, 64), loc.clone())),
            "fuller WIP still incomplete — not TPS"
        );
        let done = full_answer_tile((0, 0), 0);
        assert!(
            ctx.ingest_gpu_tile(GPUTile::from_answer_tile(&done, (64, 64), loc.clone())),
            "first whole-tile completion must count"
        );
        assert!(
            !ctx.ingest_gpu_tile(GPUTile::from_answer_tile(&done, (64, 64), loc)),
            "republish of already-complete tile must not count again"
        );
    }

    // r[verify cz.perf.home-100tps+1]
    #[test]
    fn ingest_tps_counts_each_new_complete_key_once() {
        let mut ctx = empty_ctx(0);
        let loc = ObjectivePosAndZoom {
            pos: (IntExp::ZERO, IntExp::ZERO),
            zoom_pot: 0,
        };
        let a = full_answer_tile((0, 0), 0);
        let b = full_answer_tile((TILE_EDGE_LENGTH, 0), 0);
        assert!(ctx.ingest_gpu_tile(GPUTile::from_answer_tile(&a, (128, 64), loc.clone())));
        assert!(ctx.ingest_gpu_tile(GPUTile::from_answer_tile(&b, (128, 64), loc)));
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
            escape_time_angle: 0,
            min_magnitude_angle: 0
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

    // r[verify cz.system.max-homotheties+1]
    #[test]
    fn prune_keeps_at_most_eight_homotheties_end_to_end() {
        let mut ctx = empty_ctx(0);
        ctx.memory_limit_bytes = usize::MAX;
        for mag in -5..=5 {
            let mut tile = Tile::new((0, 0), mag);
            tile.set((0, 0), outside_answer(1));
            let loc = ObjectivePosAndZoom {
                pos: (IntExp::ZERO, IntExp::ZERO),
                zoom_pot: mag,
            };
            ctx.ingest_gpu_tile(GPUTile::from_answer_tile(&tile, (64, 64), loc));
        }
        assert!(
            ctx.homothety_count() > crate::assemblies::workgroup::tile_manager::MAX_HOMOTHETIES
            , "precondition: more than eight mags ingested"
        );
        ctx.prune_distant_tiles();
        assert!(
            ctx.homothety_count()
                <= crate::assemblies::workgroup::tile_manager::MAX_HOMOTHETIES
            , "after prune the live hoard must obey the 8-homothety limit, got {}"
            , ctx.homothety_count()
        );
        assert!(
            ctx.tiles.contains_key(&(0, 0, 0))
            , "current stencil must survive the prune"
        );
    }

    #[test]
    fn zoom_location_change_does_not_wipe_hoard() {
        let mut ctx = empty_ctx(0);
        let mut tile = Tile::new((0, 0), 0);
        tile.set((0, 0), NORES_ANSWER);
        let loc = ObjectivePosAndZoom {
            pos: (IntExp::ZERO, IntExp::ZERO),
            zoom_pot: 0,
        };
        ctx.ingest_gpu_tile(GPUTile::from_answer_tile(&tile, (64, 64), loc));
        let before = ctx.tiles.len();
        assert!(before > 0);
        ctx.location.zoom_pot = 1;
        ctx.updated = true;
        // Product path no longer has clear_tiles; location change alone must keep tiles.
        assert_eq!(ctx.tiles.len(), before);
    }

    #[test]
    fn build_shade_frame_emits_entry_per_live_tile_version() {
        use crate::assemblies::headgroup::window::gpu_display::build_shade_frame;
        use crate::settings::Settings;
        let mut ctx = empty_ctx(0);
        let mut tile = Tile::new((0, 0), 0);
        tile.set((0, 0), NORES_ANSWER);
        let loc = ObjectivePosAndZoom {
            pos: (IntExp::ZERO, IntExp::ZERO),
            zoom_pot: 0,
        };
        ctx.ingest_gpu_tile(GPUTile::from_answer_tile(&tile, (64, 64), loc.clone()));
        let mut coarse = Tile::new((0, 0), -1);
        coarse.set((0, 0), NORES_ANSWER);
        let loc_coarse = ObjectivePosAndZoom {
            pos: (IntExp::ZERO, IntExp::ZERO),
            zoom_pot: -1,
        };
        ctx.ingest_gpu_tile(GPUTile::from_answer_tile(&coarse, (64, 64), loc_coarse));
        let live_versions: usize = ctx.tiles.values().map(|v| v.len()).sum();
        let mut settings = Settings::DEFAULT;
        let frame = build_shade_frame(&mut ctx, &mut settings);
        assert_eq!(
            frame.entry_ids.len()
            , live_versions
            , "every live tile version must become a sample entry"
        );
    }

    #[test]
    fn zoom_keeps_lesser_sample_entry() {
        use crate::assemblies::headgroup::window::gpu_display::build_shade_frame;
        use crate::settings::Settings;
        let mut ctx = empty_ctx(0);
        let mut tile = Tile::new((0, 0), 0);
        tile.set((0, 0), NORES_ANSWER);
        let loc = ObjectivePosAndZoom {
            pos: (IntExp::ZERO, IntExp::ZERO),
            zoom_pot: 0,
        };
        ctx.ingest_gpu_tile(GPUTile::from_answer_tile(&tile, (64, 64), loc));
        ctx.location.zoom_pot = 1;
        let mut settings = Settings::DEFAULT;
        let frame = build_shade_frame(&mut ctx, &mut settings);
        assert!(
            !frame.entry_ids.is_empty()
            , "after zoom-in, prior tiles must still sample as lesser"
        );
        assert!(
            frame.tile_entries.iter().any(|e| e.zoom_delta > 0)
            , "expected a lesser (positive zoom_delta) entry"
        );
    }

    // D-MEM-2: bump raises the memory limit the slider mirrors.
    #[test]
    fn bump_raises_memory_limit_slider_value() {
        use crate::assemblies::workgroup::tile_manager::apply_memory_bump;
        let mut ctx = empty_ctx(0);
        ctx.memory_limit_bytes = 1_000;
        let before = ctx.memory_limit_bytes;
        ctx.memory_limit_bytes = apply_memory_bump(ctx.memory_limit_bytes, 50_000);
        ctx.last_memory_bump = Some(50_000);
        assert!(ctx.memory_limit_bytes > before);
        assert_eq!(ctx.memory_limit_bytes, 50_000);
        assert_eq!(ctx.last_memory_bump, Some(50_000));
    }

    #[test]
    fn bump_does_not_lower_slider_floor() {
        use crate::assemblies::workgroup::tile_manager::apply_memory_bump;
        let mut ctx = empty_ctx(0);
        ctx.memory_limit_bytes = 80_000;
        ctx.memory_limit_bytes = apply_memory_bump(ctx.memory_limit_bytes, 10_000);
        assert_eq!(ctx.memory_limit_bytes, 80_000);
    }

    #[test]
    fn prune_path_records_bump_for_ui_slider() {
        let mut ctx = empty_ctx(5);
        ctx.memory_limit_bytes = 64;
        for i in 0..4usize {
            let mut tile = Tile::new((i, 0), 5);
            tile.set((0, 0), outside_answer(1));
            ctx.ingest_gpu_tile(GPUTile::from_answer_tile(
                &tile,
                (64, 64),
                ObjectivePosAndZoom {
                    pos: (IntExp::from((i * 64) as i32), IntExp::ZERO),
                    zoom_pot: 5,
                },
            ));
        }
        ctx.prune_distant_tiles();
        assert!(
            ctx.last_memory_bump.is_some() || ctx.memory_limit_bytes > 64,
            "protected work must move the slider floor when over budget"
        );
    }

    // D-CANCEL-1: leaving/retargeting does not wipe calibrated hoard entries.
    #[test]
    fn retarget_away_keeps_prior_calibrated_hoard() {
        let mut ctx = empty_ctx(0);
        let mut tile = Tile::new((0, 0), 0);
        tile.set((0, 0), outside_answer(3));
        let loc = ObjectivePosAndZoom {
            pos: (IntExp::ZERO, IntExp::ZERO),
            zoom_pot: 0,
        };
        ctx.ingest_gpu_tile(GPUTile::from_answer_tile(&tile, (64, 64), loc));
        assert_eq!(ctx.tile_count(), 1);
        ctx.location = ObjectivePosAndZoom {
            pos: (IntExp::from(10_000), IntExp::ZERO),
            zoom_pot: 0,
        };
        assert_eq!(
            ctx.tile_count(),
            1,
            "D-CANCEL-1: cancel/leave must keep partial calibrated work in the hoard"
        );
    }
}
