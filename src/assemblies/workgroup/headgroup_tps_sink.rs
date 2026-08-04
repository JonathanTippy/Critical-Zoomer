// read delivery.md for project context
//! Headgroup-shaped TPS counter for workgroup probes (standards HUD metric).
// r[impl cz.perf.home-10000tps-gpu+1]

use std::collections::HashMap;

use crate::assemblies::structs::GpuTileHandle;
use crate::constants::{TILE_EDGE_LENGTH, TILE_SEAT_COUNT};

/// Tracks GPU-resident tile completions the way the headgroup hoard does.
pub struct HeadgroupTpsSink {
    completed_whole: u64,
    handle_filled: HashMap<(i32, i32, i32), u32>,
}

impl HeadgroupTpsSink {
    pub fn new() -> Self {
        HeadgroupTpsSink {
            completed_whole: 0,
            handle_filled: HashMap::new(),
        }
    }

    pub fn completed_whole_tiles(&self) -> u64 {
        self.completed_whole
    }

    /// Fill percent from GPU-resident handle seat counts (not workgroup answer_tiles).
    pub fn gpu_resident_fill_percent(&self, seats_total: usize) -> f64 {
        if seats_total == 0 {
            return 100.0;
        }
        // Edge tiles report TILE_SEAT_COUNT filled but fewer screen seats — clamp.
        let seats: usize = self
            .handle_filled
            .values()
            .map(|&f| (f as usize).min(TILE_SEAT_COUNT))
            .sum::<usize>()
            .min(seats_total);
        (seats as f64) * 100.0 / (seats_total as f64)
    }

    /// Mirror `sampling.rs::ingest_gpu_handle` — returns true on newly completed whole.
    pub fn ingest_gpu_handle(&mut self, mut handle: GpuTileHandle) -> bool {
        let key = handle.absolute_key();
        let existing_filled = self.handle_filled.get(&key).copied().unwrap_or(0);
        if handle.filled_seats < existing_filled {
            if let Some(prod) = handle.production_slot.take() {
                if let Some(atlas) =
                    crate::assemblies::workgroup::production_atlas::ProductionAtlas::shared()
                {
                    if let Ok(mut atlas) = atlas.lock() {
                        atlas.release(prod);
                    }
                }
            }
            return false;
        }
        let was_complete = existing_filled >= TILE_SEAT_COUNT as u32;
        let now_complete = handle.filled_seats >= TILE_SEAT_COUNT as u32;
        let newly_completed_whole = now_complete && !was_complete;
        self.handle_filled.insert(key, handle.filled_seats);
        if newly_completed_whole {
            self.completed_whole = self.completed_whole.saturating_add(1);
        }
        newly_completed_whole
    }

    /// Build a production-slot handle for a whole tile at `origin`.
    /// Build a production-slot handle for a whole tile at `origin`.
    /// `filled_seats` comes from the on-device completion counter (D-GPU-3).
    pub fn whole_tile_handle(
        origin: (usize, usize),
        screen_res: (usize, usize),
        location: &crate::utils::ObjectivePosAndZoom,
        production_slot: u32,
        filled_seats: u32,
    ) -> GpuTileHandle {
        GpuTileHandle {
            origin_seat: origin,
            magnification_pot: location.zoom_pot,
            screen_res,
            location: location.clone(),
            production_slot: Some(production_slot),
            filled_seats,
            // Bypass payload is GPU-resident calibrated work (D-PUB-4).
            calibrated: true,
            cpu_fallback: None,
            cpu_calibrated: None,
        }
    }

    /// Count filled seats for an origin from screen_done (for partial WIP handles).
    pub fn filled_for_origin(
        origin: (usize, usize),
        screen_res: (usize, usize),
        screen_done: &[bool],
    ) -> u32 {
        let edge = TILE_EDGE_LENGTH;
        let mut filled = 0u32;
        for ly in 0..edge {
            for lx in 0..edge {
                let sx = origin.0 + lx;
                let sy = origin.1 + ly;
                if sx >= screen_res.0 || sy >= screen_res.1 {
                    continue;
                }
                let idx = sy * screen_res.0 + sx;
                if screen_done[idx] {
                    filled += 1;
                }
            }
        }
        filled
    }

    pub fn valid_seat_count(origin: (usize, usize), screen_res: (usize, usize)) -> u32 {
        let edge = TILE_EDGE_LENGTH;
        let mut n = 0u32;
        for ly in 0..edge {
            for lx in 0..edge {
                if origin.0 + lx < screen_res.0 && origin.1 + ly < screen_res.1 {
                    n += 1;
                }
            }
        }
        n
    }
}

impl Default for HeadgroupTpsSink {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::intexp::IntExp;
    use crate::utils::ObjectivePosAndZoom;

    #[test]
    fn ingest_counts_newly_completed_whole_once() {
        let mut sink = HeadgroupTpsSink::new();
        let loc = ObjectivePosAndZoom {
            pos: (IntExp::ZERO, IntExp::ZERO),
            zoom_pot: 0,
        };
        let h = HeadgroupTpsSink::whole_tile_handle((0, 0), (64, 64), &loc, 0, 4096);
        assert!(sink.ingest_gpu_handle(h.clone()));
        assert_eq!(sink.completed_whole_tiles(), 1);
        assert!(!sink.ingest_gpu_handle(h));
        assert_eq!(sink.completed_whole_tiles(), 1);
    }
}
