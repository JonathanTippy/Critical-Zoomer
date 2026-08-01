# Unit-design decisions (developer Q&A)

Recorded from the unit-design closing pass. These override assistant guesses in this tree. They do **not** override authoritative root/`docs/design` content; where they contradict authoritative text, flag and ask.

## Coloring & settings

| Id | Decision |
|----|----------|
| D-COLOR-1 | Default script = requirements list only: escape-time layer; in-filaments black; out-filaments colored as outside with ∞ escape time; nothing else. |
| D-COLOR-2 | Layer fields: source field, normalization, colorizer, base color, inside opacity, outside opacity. |
| D-COLOR-3 | Layers combine by alpha-over in script order; later layers paint on top. |
| D-COLOR-4 | Filament and node highlights are layers in the same ordered list (participate in painting order). |
| D-BAIL-1 | Changing bailout radius recolors from the stored escaping z; never changes inside/outside membership; no rework. |

## Shading

| Id | Decision |
|----|----------|
| D-SHADE-1 | In-filament “hard inversion” threshold: constant; assistant picks by experiment and marks **assumed**. |
| D-SHADE-2 | Node/minibrot smallness threshold: constant; assistant picks by experiment and marks **assumed**. |
| D-SHADE-3 | Out-filament = any period change between neighbors; paint **only the higher-period side** of the edge. |

## Memory / tile manager

| Id | Decision |
|----|----------|
| D-MEM-1 | Bump size = exactly what screen + lookahead requires; no headroom. |
| D-MEM-2 | Bump is visible: settings slider moves to the new value. |
| D-MEM-3 | Tile cost = packed answer bytes from the encoding. |
| D-MEM-4 | Hoard equality = same tile-manager function + same inputs (stencil + limit) ⇒ same keep-set on both sides. |

## Scheduling

| Id | Decision |
|----|----------|
| D-SCH-1 | Lookahead column = tile containing the mouse, at each successive magnification, depth-first, down to 8 bumps. |
| D-SCH-2 | Magnification velocity = EWMA of bumps per second over recent input. |
| D-SCH-3 | Higher-preference intratile phase may interrupt a lower one **immediately** (suspend mid-job). |

## Period / worker / reference

| Id | Decision |
|----|----------|
| D-PER-1 | Twin-test iteration count N: assistant picks by experiment, **assumed**. |
| D-PER-2 | Twin equality = relative epsilon scaled to the active gear’s precision. |
| D-PER-3 | Loop detector = power-of-two iteration-count snapshots (GPU-friendly). |
| D-GEAR-1 | Mid-tile gear escalation is **not a design path**: a gear sufficient to discriminate screen resolution is sufficient for iteration. |
| D-SERIES-1 | Series approximation is **in scope now** (design + implement). |
| D-CANCEL-1 | Tile cancelled because it left the screen: keep partial calibrated work in the hoard (resume if it returns). |
| D-REF-1 | Reference precision = requisite precision for point discrimination **plus 20 bits**, as written. |
| D-REF-2 | Superseded reference dropped when last user finishes/cancels, **or** when live reference count exceeds N (N **assumed** until set). |

## Publisher / stencil / UI

| Id | Decision |
|----|----------|
| D-PUB-1 | Publisher cadence is a flat **1000/s** ceiling while incomplete; idle when complete. No minimum floor. GPU shader path is required. |
| D-PUB-2 | Calibrated→answer bias: clamp the proximate value into the proven range, field by field. All answer fields treated as numeric (no non-numeric special case). |
| D-STEN-1 | Stencil carries, beyond homothety + resolution: mouse position, magnification velocity, and a sequence number. |
| D-WORK-1 | Workgroup keys hoarded work by **tile address only**; stencil expresses current desire, not a work key. |
| D-UI-1 | Apply stays enabled even when the coordinate field already equals the current viewport location. |

## Assumed numeric placeholders (pending experiment)

| Id | Value | Notes |
|----|-------|-------|
| A-SHADE-INFIL | Neighbor escape-time slope-angle delta > π/2 counts as hard inversion | Replace after visual experiment |
| A-SHADE-NODE | `min_magnitude` below tile point-spacing (one pixel in complex space at that mag) counts as node seed | Replace after visual experiment |
| A-PER-TWIN-N | 20 twin-test iterations | Code constant `PERIOD_CONFIRMATION_ITERATIONS`; was listed 16 before experiment locked 20 |
| A-REF-MAX-N | Max live references = 3 (current + up to 2 retained) | Replace if PO sets N |
