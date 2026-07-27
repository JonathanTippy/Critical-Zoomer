# Product Tracey rules (assistant-owned mapping of requirements)

Derived from `docs/requirements.md` Central Differentiators / Display. Not authoritative text.

r[cz.seamless.perturbation-always-on+1]

**Normative summary.** Perturbation must always be on; there is no user toggle.

**Acceptance criteria.**
- [x] Live work path uses perturbation (or equivalent always-on deep iteration), not a user-facing off switch.
  `TileSession` drives batches through `PerturbationGpuWorker` (always perturbation math;
  GPU preferred when an adapter exists, else CPU). There is no naive/perturbation branch
  or user-facing perturbation/GPU toggle.

r[cz.seamless.gpu-preferred+1]

**Normative summary.** GPU acceleration must always be on (no user toggle); when a device
exists it is preferred over CPU for tile work.

**Acceptance criteria.**
- [x] Preference selects GPU when a probe reports an adapter; falls back to CPU when not;
  both paths remain perturbation-on (`perturbation_gpu_worker.rs`).

r[cz.seamless.reference-background+1]

**Normative summary.** Reference orbits are computed in the background without a progress bar or blocking the UI.

**Acceptance criteria.**
- [x] `ReferenceWorker` seeds/binds without UI; mag change keeps the previous bound orbit
  until poll binds the new one; in-flight work on an old orbit blocks discard
  (`reference_worker.rs`, wired from `TileSession`).

r[cz.seamless.foveated-mag-velocity+1]

**Normative summary.** Tile order follows magnification velocity: zoom-in and stationary prefer foveated begin-near-attention; zoom-out prefers scredge/low-res fill. Foveated spiral is from the mouse. Lookahead is a depth-first mag column (up to 8 bumps) then spiral.

**Acceptance criteria.**
- [x] Same-mag scheduler: mag_velocity ≥ 0 runs the DFS attention column (base+1..+8) before
  screen spiral; mag_velocity < 0 takes scredge first (`tile_scheduler.rs` verifies).
- [x] `TileSession` begins off-stencil lookahead work at the attention-containing tile for each
  column bump and publishes with that deeper location (`tile_session.rs`).

r[cz.tenacious.nores-not-flat-black+1]

**Normative summary.** Unfinished pixels without proximate work must not be painted as set-black; use NORES / dynamic res stack.

**Acceptance criteria.**
- [x] `NORES_ANSWER` and publisher/headgroup fallback paths treat unknown as Outside-inf, not Inside.
  Constants + GPU pack + `guess_biased` verifies (`constants.rs`, `gpu_display`, `structs/mod.rs`).

r[cz.fast.natural-zoom-2x+1]

**Normative summary.** One mouse wheel bump zooms by 2× magnification (one POT step).

**Acceptance criteria.**
- [x] Scroll handling changes zoom_pot by ±1 per discrete bump under natural scroll
  (`inputs.rs` scroll_zoom_tests; e2e_zoom_smoke exercises headed scroll).

r[cz.hoarding.one-answer-per-point+1]

**Normative summary.** There is one answer per point; cosmetic settings recolor from hoarded work.

**Acceptance criteria.**
- [x] Answer tiles persist across pans via absolute dyadic keys; sparser replacements rejected
  (`sampling.rs` hoard_tests). Cosmetic-only recolor from hoard is headgroup shade responsibility.

r[cz.system.tile-manager-protect-current-lookahead+1]

**Normative summary.** Tile manager never prunes on-screen or lookahead tiles for memory; if those alone exceed the limit, bump the limit.

**Acceptance criteria.**
- [x] `plan_prunes` skips CurrentStencil/Lookahead; `required_limit_bump` returns protected sum when over budget
  (`tile_manager.rs` verifies, including protected-only never pruned).

r[cz.system.max-homotheties+1]

**Normative summary.** At most ~8 magnifications/homotheties remain in play under the shared tile manager.

**Acceptance criteria.**
- [x] Prune plan reduces unprotected mags when more than 8 are present
  (`tile_manager.rs` + live `SamplingContext::prune_distant_tiles` wires the shared manager).
