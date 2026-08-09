# Design (non-authoritative index)

Assistant-owned index for this folder. The developer's files speak for themselves.

## Live

- `design-target.md` — the developer's current target: **build v0.0.9 but on GPU** (views, not tiles; full remap of old work).
- `naive-gpu-design.md` — Naive GPU port design lock (2026-08-08): wgpu, F32 + optional F64, control-plane queues / parallel bouts, FLOP-ratio IPS bar. **First pass implemented** (live `mode:naive-gpu`); FLOP→IPS tuning still open.
- `workgroup-virtues.md` — enshrined study of the v0.0.9 workgroup (commit e6a0560): the mechanisms and invariants that kept it from getting behind pivots, confusing work storage, stalling, or publishing stale work. **Read before changing anything in the workgroup.** Every later regression traced to breaking one of its seven invariants; they are not to be re-broken.
- `depth-design.md` — perturbation with background reference worker; partially implemented (see file status). CGenerator admission + PPS-selected kernel dispatch in progress.

## Stale

- `Stale/` — the tile-era design set (the old architecture and standards docs live in `../Trash/`). Kept for reference only; the tile machine proved too complicated to get right (see `design-target.md`). Do not implement from these without an explicit developer request.

## Code baseline

The live rust code is release **v0.0.9** (commit e6a0560), restored in commit 3bbba12. Tile-era code is preserved in git history (pre-revert commits, tip 3f877f3) and in git stash "tile-era code WIP before v0.0.9 revert".
