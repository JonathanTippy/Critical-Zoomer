# New unit: Settings & coloring script

No authoritative unit file. Non-authoritative.

## Settings struct (UD-SET-1) — inferred + decisions

Must include at least:

- Coloring script (ordered layer list)
- Bailout radius (animable continuous; domain at least [2, 255] per requirements, may allow wider UI range)
- Memory limit (slider; L means L CPU + L VRAM)
- Selection / id bookkeeping for the settings UI

No computation settings (requirements Hoarding).

## Layer model (UD-SET-LAY-1) — D-COLOR-2, D-COLOR-3, D-COLOR-4

Each layer:

| Field | Role |
|-------|------|
| Source | Which answer field or highlight kind |
| Normalization | none / log / reciprocal (requirements) |
| Colorizer | sin / modulo |
| Base color | RGB |
| Inside opacity | 0…max |
| Outside opacity | 0…max |

Composite: **alpha-over**, script order, later on top.

Highlight kinds (in-filament, out-filament, nodes, and any STE highlight) are **layers in the same list**.

## Default script (UD-SET-DEF-1) — D-COLOR-1

Exactly:

1. Escape-time layer (visible browsing default).
2. In-filaments as black.
3. Out-filaments colored like outside with ∞ escape time.

Nothing else in the default. (Live code may currently ship extra layers — treat as code drift vs this closing decision until aligned.)

## Bailout (UD-SET-BAIL-1) — D-BAIL-1

Changing bailout recolors from stored `escape_z` only. Does not change membership. Does not invalidate hoarded work.

## Memory UI (UD-SET-MEM-1) — D-MEM-2

On bump from tile manager: slider value moves to the bumped limit.
