# Shade Tracey rules (assistant-owned mapping of docs/design/shaders.md)

Not authoritative text. Normative shading phases live in design; these tags exist
for Tracey linkage of shade verifies (`shade_tests.rs` / `shade_oracle`).

r[cz.shade.in-filament-slope-inversion+1]

**Normative summary.** In-filaments are detected via hard escape-time slope angle
inversions.

**Acceptance criteria.**
- [x] ≥3 shade verifies: inversion loci paint as in-filament; non-inversion do not
  (`shade_tests.rs`)

r[cz.shade.out-filament-period-step+1]

**Normative summary.** Out-filaments are period edges (period / small-time comparison
on annotated edges).

**Acceptance criteria.**
- [x] ≥3 shade verifies: lower/zero/highlight period-step cases
  (`shade_tests.rs`)

r[cz.shade.node-smallness-minimum+1]

**Normative summary.** Minibrots / nodes are points where smallness approaches zero
(also via hard angle inversion).

**Acceptance criteria.**
- [x] ≥3 shade verifies for node/smallness minimum detection (`shade_tests.rs`)

r[cz.shade.small-time-edge-nonzero+1]

**Normative summary.** Small-time edge annotation is nonzero where the edge rule
applies.

**Acceptance criteria.**
- [x] ≥3 shade verifies for small-time edge nonzero cases (`shade_tests.rs`)

r[cz.shade.escape-continues-to-bailout+1]

**Normative summary.** Escape coloring continues out to the bailout radius.

**Acceptance criteria.**
- [x] ≥3 shade verifies escape continues to bailout (`shade_tests.rs`)

r[cz.shade.layers-in-script-order+1]

**Normative summary.** Layered coloring paints in script list order.

**Acceptance criteria.**
- [x] ≥3 shade verifies including GPU↔oracle pixel agree on layer order
  (`shade_tests.rs`)
