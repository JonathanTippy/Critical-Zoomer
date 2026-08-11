# Shade Tracey rules (assistant-owned mapping of the shaders design)

Not authoritative text. Normative shading phases live in design — the source file moved to
`docs/assistant/design/Stale/shaders.md` in the 2026-08-06 revert cleanup (it was written against the
tile-era pipeline, but its shading *behavior* descriptions match what v0.0.9's shadergroup
actually paints). These tags exist for Tracey linkage of shade verifies.

> **2026-08-06 revert note.** Codebase is now v0.0.9 (e6a0560). The cited `shade_tests.rs` /
> `shade_oracle` harness was tile-era and is gone; the shading *behavior* lives in the restored
> `shadergroup/` (`escaper.rs`, `colorer/`), and the restored `settings.rs` carries the full
> layer/highlight instruction set the tile era had dropped. Every normative summary below
> remains product-true — v0.0.9 shipped these cosmetics. All checkboxes cleared; each rule
> needs fresh verifies written against the restored shadergroup (oracle can be the CPU colorer
> path itself).

r[cz.shade.in-filament-slope-inversion+1]

**Normative summary.** In-filaments are detected via hard escape-time slope angle
inversions.

**Acceptance criteria.**
- [ ] Verifies against restored shadergroup: inversion loci paint as in-filament; non-inversion
  do not.

r[cz.shade.out-filament-period-step+1]

**Normative summary.** Out-filaments are period edges (period / small-time comparison
on annotated edges).

**Acceptance criteria.**
- [ ] Verifies: lower/zero/highlight period-step cases on the restored colorer.
- Note: period data quality depends on the period-determination path, which is a known weak
  stage at v0.0.9 (virtues doc §12) — shade verifies should use synthetic answers, not live
  periods, until the derivative-based period theory lands.

r[cz.shade.node-smallness-minimum+1]

**Normative summary.** Minibrots / nodes are points where smallness approaches zero
(also via hard angle inversion).

**Acceptance criteria.**
- [ ] Verifies for node/smallness minimum detection on the restored colorer.

r[cz.shade.small-time-edge-nonzero+1]

**Normative summary.** Small-time edge annotation is nonzero where the edge rule
applies.

**Acceptance criteria.**
- [ ] Verifies for small-time edge nonzero cases on the restored colorer.
- Reminder (collected-wisdom): points outside r=2 legitimately have small_time 0 — that 0 is
  not a discontinuity and must not be special-cased into a seam.

r[cz.shade.escape-continues-to-bailout+1]

**Normative summary.** Escape coloring continues out to the bailout radius.

**Acceptance criteria.**
- [ ] Verifies escape continues to bailout on the restored escaper.

r[cz.shade.layers-in-script-order+1]

**Normative summary.** Layered coloring paints in script list order.

**Acceptance criteria.**
- [x] Verifies layer order on the restored colorer, including agreement between the GPU display
  path and a CPU oracle on the same answers.

**Test.** `gpu_matches_og_color32_default_script`, `gpu_matches_og_per_layer_scripts`,
`gpu_matches_og_home_escape_frame` (colorer/gpu).
