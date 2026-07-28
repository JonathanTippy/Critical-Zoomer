# Supplement: workgroup.md

Pairs with authoritative `docs/design/workgroup.md`. Non-authoritative.

## Responsibility (UD-WG-1) — inferred

Complete work in best order/speed; publish continuous high-quality GPU answer tiles; pause WIP that left the viewport; never give up on started visible work; mix old+new for continuity; manage perturbation & references independently.

## Sub-actors (UD-WG-2) — inferred

As auth diagram: tile scheduler → tile worker ⇄ intratile scheduler; reference worker → tile worker; tile worker → gpu uploader → tile publisher → headgroup; publisher also consults hoard/proximate.

Detailed channels/rates: `new/actor_graph.md`.

## Honesty boundary (UD-WG-3) — inferred

Actors before publisher: calibrated & honest. Publisher: best-effort answers for headgroup (no agnostic seats).

## Hoard keying (UD-WG-4) — D-WORK-1

Key by tile address only. Stencil expresses desire (which addresses are urgent), not a storage key.
