//! Shared tile collection manager (function, not actor).
//! Design: docs/design/tile_manager.md / docs/architecture.md Tile Manager.
// r[impl cz.system.tile-manager-protect-current-lookahead+1]
// r[impl cz.system.max-homotheties+1]

use std::collections::HashMap;

/// Preference when choosing what to prune (lower = keep longer).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum TileKeepClass {
    CurrentStencil = 0,
    Lookahead = 1,
    HoardedNearFocus = 2,
    UnrelatedHoard = 3,
}

pub const MAX_HOMOTHETIES: usize = 8;

#[derive(Clone, Debug)]
pub struct ManagedTileKey {
    pub mag_pot: i32,
    pub origin: (i32, i32),
}

#[derive(Clone, Debug)]
pub struct ManagedTileMeta {
    pub keep: TileKeepClass,
    pub bytes: usize,
}

/// Decide which tiles to evict under a memory budget and ~8-homothety cap.
/// Returns keys that should be pruned (worst first).
pub fn plan_prunes(
    tiles: &HashMap<(i32, i32, i32), ManagedTileMeta>,
    memory_limit_bytes: usize,
    used_bytes: usize,
) -> Vec<(i32, i32, i32)> {
    let mut by_mag: HashMap<i32, usize> = HashMap::new();
    for ((mag, _, _), _) in tiles {
        *by_mag.entry(*mag).or_insert(0) += 1;
    }
    let mut excess_mags = by_mag.len().saturating_sub(MAX_HOMOTHETIES);

    let mut candidates: Vec<((i32, i32, i32), TileKeepClass, usize)> = tiles
        .iter()
        .map(|(k, m)| (*k, m.keep, m.bytes))
        .collect();
    // Evict unrelated / far hoard before lookahead / current.
    candidates.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| b.2.cmp(&a.2)));

    let mut to_prune = Vec::new();
    let mut bytes = used_bytes;

    // First drop tiles belonging to excess magnifications (prefer unrelated).
    if excess_mags > 0 {
        let mut mag_counts = by_mag;
        for (key, keep, size) in &candidates {
            if excess_mags == 0 {
                break;
            }
            if matches!(keep, TileKeepClass::CurrentStencil | TileKeepClass::Lookahead) {
                continue;
            }
            let mag = key.0;
            if mag_counts.get(&mag).copied().unwrap_or(0) == 0 {
                continue;
            }
            // Only prune a mag entirely when we still exceed the cap.
            if mag_counts.len() <= MAX_HOMOTHETIES {
                break;
            }
            to_prune.push(*key);
            bytes = bytes.saturating_sub(*size);
            if let Some(c) = mag_counts.get_mut(&mag) {
                *c = c.saturating_sub(1);
                if *c == 0 {
                    mag_counts.remove(&mag);
                    excess_mags = excess_mags.saturating_sub(1);
                }
            }
        }
    }

    for (key, keep, size) in candidates {
        if bytes <= memory_limit_bytes {
            break;
        }
        if matches!(keep, TileKeepClass::CurrentStencil | TileKeepClass::Lookahead) {
            // Never prune on-screen or lookahead for memory (architecture).
            continue;
        }
        if to_prune.contains(&key) {
            continue;
        }
        to_prune.push(key);
        bytes = bytes.saturating_sub(size);
    }

    to_prune
}

/// If current+lookahead alone exceed the limit, the limit must bump (design).
// r[impl cz.int.memory-bump+1]
pub fn required_limit_bump(
    tiles: &HashMap<(i32, i32, i32), ManagedTileMeta>,
    memory_limit_bytes: usize,
) -> Option<usize> {
    let protected: usize = tiles
        .values()
        .filter(|m| {
            matches!(
                m.keep,
                TileKeepClass::CurrentStencil | TileKeepClass::Lookahead
            )
        })
        .map(|m| m.bytes)
        .sum();
    if protected > memory_limit_bytes {
        Some(protected)
    } else {
        None
    }
}

/// Raise the memory limit to at least `needed` (never lowers).
// r[impl cz.int.memory-bump+1]
pub fn apply_memory_bump(current_limit_bytes: usize, needed: usize) -> usize {
    current_limit_bytes.max(needed)
}

/// Bytes of current-stencil + lookahead tiles (slider floor / on-demand minimum).
// r[impl cz.system.memory-default-1gb+1]
pub fn protected_bytes(tiles: &HashMap<(i32, i32, i32), ManagedTileMeta>) -> usize {
    tiles
        .values()
        .filter(|m| {
            matches!(
                m.keep,
                TileKeepClass::CurrentStencil | TileKeepClass::Lookahead
            )
        })
        .map(|m| m.bytes)
        .sum()
}

/// Slider floor in GB: at least 0.125, raised by protected footprint when larger.
pub fn memory_slider_floor_gb(protected_byte_total: usize) -> f64 {
    let from_protected = (protected_byte_total as f64) / 1_000_000_000.0;
    from_protected.max(0.125)
}


#[cfg(test)]
mod tests {
    use super::*;

    fn meta(keep: TileKeepClass, bytes: usize) -> ManagedTileMeta {
        ManagedTileMeta { keep, bytes }
    }

    // r[verify cz.system.tile-manager-protect-current-lookahead+1]
    #[test]
    fn never_prunes_current_or_lookahead_for_memory() {
        let mut tiles = HashMap::new();
        tiles.insert((0, 0, 0), meta(TileKeepClass::CurrentStencil, 900));
        tiles.insert((1, 0, 0), meta(TileKeepClass::Lookahead, 900));
        tiles.insert((-1, 0, 0), meta(TileKeepClass::UnrelatedHoard, 100));
        let plan = plan_prunes(&tiles, 500, 1900);
        assert!(!plan.contains(&(0, 0, 0)));
        assert!(!plan.contains(&(1, 0, 0)));
        assert!(plan.contains(&(-1, 0, 0)));
    }

    // r[verify cz.system.tile-manager-protect-current-lookahead+1]
    #[test]
    fn bumps_when_protected_exceeds_limit() {
        let mut tiles = HashMap::new();
        tiles.insert((0, 0, 0), meta(TileKeepClass::CurrentStencil, 800));
        tiles.insert((1, 0, 0), meta(TileKeepClass::Lookahead, 800));
        assert_eq!(required_limit_bump(&tiles, 1000), Some(1600));
        assert_eq!(required_limit_bump(&tiles, 2000), None);
    }

    #[test]
    fn prefers_unrelated_before_near_hoard() {
        let mut tiles = HashMap::new();
        tiles.insert((0, 0, 0), meta(TileKeepClass::CurrentStencil, 10));
        tiles.insert((-2, 1, 1), meta(TileKeepClass::UnrelatedHoard, 50));
        tiles.insert((-1, 2, 2), meta(TileKeepClass::HoardedNearFocus, 50));
        let plan = plan_prunes(&tiles, 40, 110);
        assert_eq!(plan.first().copied(), Some((-2, 1, 1)));
    }

    // r[verify cz.system.tile-manager-protect-current-lookahead+1]
    #[test]
    fn protected_only_hoard_never_appears_in_prune_plan() {
        let mut tiles = HashMap::new();
        tiles.insert((0, 0, 0), meta(TileKeepClass::CurrentStencil, 5000));
        tiles.insert((1, 0, 0), meta(TileKeepClass::Lookahead, 5000));
        let plan = plan_prunes(&tiles, 1, 10_000);
        assert!(
            plan.is_empty()
            , "current+lookahead alone must never be memory-pruned; bump instead"
        );
        assert_eq!(required_limit_bump(&tiles, 1), Some(10_000));
    }

    // r[verify cz.system.max-homotheties+1]
    #[test]
    fn nine_unprotected_mags_pruned_until_at_most_eight_remain() {
        let mut tiles = HashMap::new();
        tiles.insert((0, 0, 0), meta(TileKeepClass::CurrentStencil, 1));
        for mag in 1..=9 {
            tiles.insert((mag, 0, 0), meta(TileKeepClass::UnrelatedHoard, 1));
        }
        let plan = plan_prunes(&tiles, usize::MAX, 10);
        let mut remaining: HashMap<i32, ()> = HashMap::new();
        remaining.insert(0, ());
        for mag in 1..=9 {
            if !plan.iter().any(|k| k.0 == mag) {
                remaining.insert(mag, ());
            }
        }
        assert!(
            remaining.len() <= MAX_HOMOTHETIES
            , "after prune plan, remaining mags={} > MAX_HOMOTHETIES"
            , remaining.len()
        );
        assert!(!plan.contains(&(0, 0, 0)));
    }

    // r[verify cz.system.max-homotheties+1]
    #[test]
    fn prune_order_is_unrelated_then_near_never_protected() {
        let mut tiles = HashMap::new();
        tiles.insert((0, 0, 0), meta(TileKeepClass::CurrentStencil, 10));
        tiles.insert((1, 0, 0), meta(TileKeepClass::Lookahead, 10));
        tiles.insert((-1, 1, 1), meta(TileKeepClass::HoardedNearFocus, 40));
        tiles.insert((-2, 2, 2), meta(TileKeepClass::UnrelatedHoard, 40));
        let plan = plan_prunes(&tiles, 50, 100);
        assert!(plan.contains(&(-2, 2, 2)));
        assert_eq!(plan.first().copied(), Some((-2, 2, 2)));
        assert!(!plan.contains(&(0, 0, 0)));
        assert!(!plan.contains(&(1, 0, 0)));
    }

    // r[verify cz.system.max-homotheties+1]
    #[test]
    fn enforces_max_homothety_count_on_unprotected() {
        let mut tiles = HashMap::new();
        tiles.insert((0, 0, 0), meta(TileKeepClass::CurrentStencil, 1));
        for mag in 1..=10 {
            tiles.insert((mag, 0, 0), meta(TileKeepClass::UnrelatedHoard, 1));
        }
        let plan = plan_prunes(&tiles, usize::MAX, 11);
        // Must drop enough unprotected mags to approach the 8-homothety budget.
        assert!(!plan.is_empty());
        assert!(!plan.contains(&(0, 0, 0)));
    }

    #[test]
    fn protected_bytes_sums_current_and_lookahead_only() {
        let mut tiles = HashMap::new();
        tiles.insert((0, 0, 0), meta(TileKeepClass::CurrentStencil, 100));
        tiles.insert((1, 0, 0), meta(TileKeepClass::Lookahead, 50));
        tiles.insert((-1, 0, 0), meta(TileKeepClass::UnrelatedHoard, 999));
        assert_eq!(protected_bytes(&tiles), 150);
    }

    #[test]
    fn memory_slider_floor_gb_respects_eighth() {
        assert!((memory_slider_floor_gb(0) - 0.125).abs() < 1e-9);
    }

    #[test]
    fn memory_slider_floor_gb_raises_with_protected() {
        assert!((memory_slider_floor_gb(2_000_000_000) - 2.0).abs() < 1e-9);
    }
}
