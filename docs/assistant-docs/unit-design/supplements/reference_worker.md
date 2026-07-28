# Supplement: reference_worker.md

Pairs with authoritative `docs/design/reference_worker.md`. Non-authoritative.

## Choose / build / bind (UD-REF-1) — inferred + D-REF-1

- **Choose:** upper-left of screen = stencil homothety location.
- **Build:** rug float with precision = discrimination requirement **+ 20 bits** (D-REF-1).
- **Bind:** each stencil has one “latest” reference; if new not ready, keep using old. Work started with old must retain that orbit.

## Update trigger (UD-REF-2) — inferred

Recompute when stencil **magnification** changes. Pan alone does not trigger.

## Contents (UD-REF-3) — inferred

Store all stack types needed by workers in the produced reference, including the series-approximation term (series is in scope: D-SERIES-1).

## Retirement (UD-REF-4) — D-REF-2, A-REF-MAX-N

Drop a reference when:

1. no in-flight work still holds it, **or**
2. live reference count would exceed **N = 3** (**assumed**): evict oldest with zero users first; never evict a still-referenced orbit.

## Delivery (UD-REF-5) — inferred

References are delivered to the tile worker. Glitch handling stays inside the tile worker and does **not** notify the reference worker (auth tile_worker).
