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

r[cz.fast.settings-100ms+1]

**Normative summary.** Settings changes visible within 100ms (hoard recolor / shade path).

**Acceptance criteria.**
- [x] Release hard-assert shade/recolor ≤100ms for ≥3 setting deltas

r[cz.fast.cosmetic-17ms-1080p+1]

**Normative summary.** Continuous cosmetics animable within 17ms at 1080p.

**Acceptance criteria.**
- [x] Release hard-assert shade path ≤17ms @1080p for ≥3 continuous params

r[cz.fast.scroll-10-in-300ms+1]

**Normative summary.** Sustain 10 zoom bumps within 300ms (applied ticks, not harness wall alone).

**Acceptance criteria.**
- [x] Unit: 10 debt thresholds ⇒ Δpot ±10 within 300ms accounting
- [x] ≥3 verifies; e2e must not waive product 300ms via 2000ms harness bound alone

r[cz.fast.shift-space-5bps+1]

**Normative summary.** Shift/Space zoom hold rate about 5 bumps per second (center origin).

**Acceptance criteria.**
- [x] Hold-repeat ~5 bumps/s; ≥3 clocked verifies

r[cz.fast.no-tick-backlog+1]

**Normative summary.** Fast spinning neither skips nor backlogs scroll ticks (debt gaps).

**Acceptance criteria.**
- [x] N thresholds ⇒ N zooms; reverse-sign clears; no deferred burst (≥3)

r[cz.fast.input-next-frame-17ms+1]

**Normative summary.** Movements/zooms visible this or next frame (≤17ms at 60Hz).

**Acceptance criteria.**
- [x] Same-turn transform/input apply; ≥3 verifies (headed latency hard-assert when measurable)

r[cz.system.memory-default-1gb+1]

**Normative summary.** Default memory limit is 1GB CPU + 1GB VRAM class (1gb settings default).

**Acceptance criteria.**
- [x] Default const/settings == 1_000_000_000 class; ≥3 verifies

r[cz.cosmetic.bailout-range-2-255+1]

**Normative summary.** Bailout radius accepts at least [2, 255].

**Acceptance criteria.**
- [x] Accept/clamp verifies at 2, mid, 255 (≥3)

r[cz.deep.min-zoom-pot-capacity+1]

**Normative summary.** Types/gears can represent magnification factor ≥ 2^3600000 (pot magnitude).

**Acceptance criteria.**
- [x] Capacity/property verifies — not a 100-hour zoom run (≥3)

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
