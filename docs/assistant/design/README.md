# Design (non-authoritative index)

Assistant-owned index for this folder. The developer's files speak for themselves.

## Live

- `design-target.md` — the developer's current target: **build v0.0.9 but on GPU** (views, not tiles; full remap of old work).
- `naive-gpu-design.md` — Naive GPU port design lock (2026-08-08): wgpu, F32 + optional F64, control-plane queues / parallel bouts, FLOP-ratio IPS bar. **First pass implemented** (live `mode:naive-gpu`); FLOP→IPS tuning still open.
- `shadergroup-virtues.md` — single shade path (escaper + colorer); animated bailout is the same path with different numbers (2026-08-11 interview).
- `workgroup-virtues.md` — enshrined study of the v0.0.9 workgroup (commit e6a0560): the mechanisms and invariants that kept it from getting behind pivots, confusing work storage, stalling, or publishing stale work. **Read before changing anything in the workgroup.** Every later regression traced to breaking one of its seven invariants; they are not to be re-broken.
- `depth-design.md` — perturbation with background reference worker; partially implemented (see file status). CGenerator admission + PPS-selected kernel dispatch in progress.
- `gearbox.md` — compute-gear policy + test-only FloatExp Oracle (`src/gearbox/`).

## Quality

- `../quality-doctrine.md` — no ignore / soft-skip / soft floor; FIX NOW on Criterion.
- `../quality-slip-review.md` — how the bar softened after v0.0.9 and how to stop.
- `../headgroup-charter.md` — what may change outside the workgroup.
- `../coverage.md` — living llvm-cov baseline path.
- `../fuzz.md` — cargo-fuzz targets (coords + admission).

## Trashed tile-era design

Tile-era design prose: `../Trash/design-Stale-tile-era/`. Do not implement from it
without an explicit developer request.

## Code baseline

The live rust code is release **v0.0.9** (commit e6a0560), restored in commit 3bbba12. Tile-era code is preserved in git history (pre-revert commits, tip 3f877f3) and in git stash "tile-era code WIP before v0.0.9 revert".
