# Design (non-authoritative index)

Assistant-owned index for this folder. The developer's files speak for themselves.

## Live

- `design-target.md` — the developer's current target: **build v0.0.9 but on GPU** (views, not tiles; full remap of old work).
- `workgroup-virtues.md` — enshrined study of the v0.0.9 workgroup (commit e6a0560): the mechanisms and invariants that kept it from getting behind pivots, confusing work storage, stalling, or publishing stale work. **Read before changing anything in the workgroup.** Every later regression traced to breaking one of its seven invariants; they are not to be re-broken.

## Stale

- `Stale/` — the tile-era authoritative design set (architecture was likewise moved to `docs/Stale/`). Kept for reference only; the tile machine proved too complicated to get right (see `design-target.md`). Do not implement from these without an explicit developer request.

## Code baseline

The live rust code is release **v0.0.9** (commit e6a0560), restored in commit 3bbba12. Tile-era code is preserved in git history (pre-revert commits, tip 3f877f3) and in git stash "tile-era code WIP before v0.0.9 revert".
