# Supplement: tile_manager.md

Pairs with authoritative `docs/design/tile_manager.md`. Non-authoritative.

## Keep-set (UD-TM-1) — inferred + D-MEM-4

Pure function of: current stencil (incl. mouse for preference), memory limit, and candidate tile set.

Same function + same inputs on headgroup and workgroup ⇒ same keep-set (D-MEM-4). No sync channel.

## Preference order (UD-TM-2) — inferred

As auth: (1) current stencil members (2) lookahead (deeper less preferred) (3) hoarded near mouse (4) unrelated. Never prune on-screen or lookahead for memory; bump instead.

## Cost (UD-TM-3) — D-MEM-3

Cost(tile) = packed answer bytes from the encoding (`new/tile_and_answer.md`), not allocator overhead.

## Bump (UD-TM-4) — D-MEM-1, D-MEM-2

If screen + lookahead exceed limit: set limit to **exactly** that requirement (no headroom). Workgroup publisher path sends bump to headgroup; slider moves to the new value.

## Homothety cap (UD-TM-5) — inferred

Enforce ≤8 distinct magnifications in play.
