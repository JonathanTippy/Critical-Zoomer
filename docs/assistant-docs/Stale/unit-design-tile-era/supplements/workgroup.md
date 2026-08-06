# Supplement: workgroup.md

Pairs with authoritative `docs/design/workgroup.md`. Non-authoritative.

## Responsibility (UD-WG-1) — inferred

Complete work in best order/speed; publish continuous high-quality GPU answer tiles; pause WIP that left the viewport; never give up on started visible work; mix old+new for continuity; manage perturbation & references independently.

## Sub-actors (UD-WG-2) — inferred

As auth diagram, with residency made explicit:

- tile scheduler → tile worker ⇄ intratile scheduler
- reference worker → tile worker
- **GPU-native:** tile worker → tile publisher → headgroup (uploader bypass)
- **CPU work:** tile worker → gpu uploader → tile publisher → headgroup
- publisher consults hoard/proximate on both paths

Auth diagram draws only the uploader path; bypass is also auth (`gpu_uploader.md`, architecture). Detailed channels: `new/actor_graph.md`.

## Honesty boundary (UD-WG-3) — inferred

Actors before publisher: calibrated & honest. Publisher: best-effort answers for headgroup (no unproven membership left as agnostic seats).

## Hoard keying (UD-WG-4) — D-WORK-1

Key by tile address only. Stencil expresses desire (which addresses are urgent), not a storage key.
